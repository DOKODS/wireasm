//! Logic units of an integrated circuit: what a gate or a flip-flop does.
//!
//! The solver evaluates a package unit by unit. Combinational units read
//! their inputs and drive their outputs; sequential units keep a state and
//! change it on a clock edge.

use crate::library::Func;

/// The state a sequential unit carries between clock edges.
#[derive(Clone, Copy, Debug, Default)]
pub struct State {
    /// The bits of the unit: a counter value, a flip-flop output.
    pub bits: u64,
    /// The previous level of the clock input.
    pub clk_prev: bool,
}

impl State {
    pub fn new() -> State {
        State::default()
    }
}

/// Evaluate a combinational unit. The inputs are the logic levels of its
/// input pins; the result are the levels of its output pins.
pub fn combinational(func: Func, ins: &[bool], sel_bits: usize, has_enable: bool) -> Vec<bool> {
    let bit = |i: usize| ins.get(i).copied().unwrap_or(false);
    match func {
        Func::And2 | Func::And3 | Func::And4 => {
            vec![ins.iter().all(|v| *v)]
        }
        Func::Or2 | Func::Or3 | Func::Or4 => {
            vec![ins.iter().any(|v| *v)]
        }
        Func::Nand2 | Func::Nand3 | Func::Nand4 => {
            vec![!ins.iter().all(|v| *v)]
        }
        Func::Nor2 | Func::Nor3 | Func::Nor4 => {
            vec![!ins.iter().any(|v| *v)]
        }
        Func::Xor2 => {
            let n = ins.iter().filter(|v| **v).count();
            vec![n % 2 == 1]
        }
        Func::Xnor2 => {
            let n = ins.iter().filter(|v| **v).count();
            vec![n % 2 == 0]
        }
        Func::Not1 => vec![!bit(0)],
        Func::Buf1 => vec![bit(0)],
        Func::Xor2Enable => {
            let n = ins.iter().filter(|v| **v).count();
            vec![bit(ins.len().saturating_sub(1)) && n % 2 == 1]
        }
        Func::Mux2 => {
            // The enable comes last, the lines before it carry the address,
            // the rest are the data inputs.
            let en_index = ins.len().saturating_sub(1);
            let enabled = if has_enable {
                ins.get(en_index).copied().unwrap_or(true)
            } else {
                true
            };
            // The address lines sit just before the enable, or at the end
            // when the unit has no enable.
            let sel_start = if has_enable {
                en_index.saturating_sub(sel_bits)
            } else {
                ins.len().saturating_sub(sel_bits)
            };
            let mut index = 0usize;
            for bit_index in 0..sel_bits {
                if ins.get(sel_start + bit_index).copied().unwrap_or(false) {
                    index |= 1 << bit_index;
                }
            }
            let value = ins.get(index).copied().unwrap_or(false);
            vec![enabled && value, enabled && !value]
        }
        Func::Decoder3to8 => {
            // Three address inputs, eight decoded outputs.
            let address = (0..3)
                .map(|i| if bit(i) { 1usize } else { 0 })
                .fold(0usize, |acc, v| (acc << 1) | v);
            let enabled = bit(3);
            (0..8)
                .map(|i| enabled && i == address)
                .collect()
        }
        Func::Encoder8to3 => {
            // The highest input that is high wins.
            let mut address = 0usize;
            let mut found = false;
            for i in (0..8).rev() {
                if bit(i) {
                    address = i;
                    found = true;
                    break;
                }
            }
            vec![
                found && (address & 1) != 0,
                found && (address & 2) != 0,
                found && (address & 4) != 0,
            ]
        }
        Func::Decoder4to16 => {
            // Four address inputs, four decoded outputs.
            let address = (0..4)
                .map(|i| if bit(i) { 1usize } else { 0 })
                .fold(0usize, |acc, v| (acc << 1) | v);
            (0..4)
                .map(|i| {
                    let group = address / (1 << (4 - i - 1));
                    group % 2 == 0
                })
                .collect()
        }
        _ => vec![false],
    }
}

/// Evaluate a sequential unit. The inputs are the logic levels of its input
/// pins; the state is updated when the clock edge arrives.
pub fn sequential(func: Func, ins: &[bool], state: &mut State) -> Vec<bool> {
    let bit = |i: usize| ins.get(i).copied().unwrap_or(false);
    let clock = match func {
        Func::Counter4 | Func::Counter4Down | Func::BcdCounter | Func::Shift8 => bit(0),
        _ => bit(1),
    };
    // A negative-edge unit reacts to the falling edge, the rest to the
    // rising one.
    let edge = if is_rising(func) {
        clock && !state.clk_prev
    } else {
        !clock && state.clk_prev
    };
    state.clk_prev = clock;

    match func {
        Func::Dff | Func::DffNeg => {
            if edge {
                state.bits = u64::from(bit(0));
            }
            let q = state.bits & 1 == 1;
            vec![q, !q]
        }
        Func::Jk => {
            if edge {
                let j = bit(0);
                let k = bit(1);
                let q = state.bits & 1 == 1;
                let next = match (j, k) {
                    (true, true) => !q,
                    (true, false) => true,
                    (false, true) => false,
                    (false, false) => q,
                };
                state.bits = u64::from(next);
            }
            let q = state.bits & 1 == 1;
            vec![q, !q]
        }
        Func::Tff => {
            if edge {
                let t = bit(0);
                let q = state.bits & 1 == 1;
                state.bits = u64::from(q ^ t);
            }
            let q = state.bits & 1 == 1;
            vec![q, !q]
        }
        Func::Counter4 | Func::Counter4Down | Func::BcdCounter => {
            // The clear input of a TTL counter is active low.
            let reset = !bit(1);
            if reset {
                state.bits = 0;
            } else if edge {
                state.bits = match func {
                    Func::Counter4Down => state.bits.wrapping_sub(1) & 0xF,
                    Func::BcdCounter => (state.bits + 1) % 10,
                    _ => (state.bits + 1) & 0xF,
                };
            }
            (0..4).map(|i| (state.bits >> i) & 1 == 1).collect()
        }
        Func::Shift8 => {
            // The clear input of a shift register is active low too.
            let reset = !bit(2);
            if reset {
                state.bits = 0;
            } else if edge {
                let serial = u64::from(bit(1));
                state.bits = ((state.bits << 1) | serial) & 0xFF;
            }
            (0..8).map(|i| (state.bits >> i) & 1 == 1).collect()
        }
        _ => vec![false],
    }
}

/// How many outputs a unit of the given function drives.
///
/// The count comes from the unit itself: a counter can be four bits wide or
/// twelve, depending on the package.
pub fn output_count(func: Func) -> usize {
    match func {
        Func::Mux2 => 2,
        Func::Decoder4to16 => 4,
        Func::Dff | Func::DffNeg | Func::Jk | Func::Tff => 2,
        Func::Counter4 | Func::Counter4Down | Func::BcdCounter => 4,
        Func::Shift8 => 8,
        _ => 1,
    }
}

/// How many outputs a unit with the given signal names drives.
pub fn output_count_of(func: Func, outs: &[String]) -> usize {
    // A counter and a shift register name every bit, so their width follows
    // the package.
    match func {
        Func::Counter4 | Func::Counter4Down | Func::BcdCounter | Func::Shift8 => outs.len(),
        _ => output_count(func),
    }
}

/// Whether the unit reacts to a rising clock edge.
pub fn is_rising(func: Func) -> bool {
    !matches!(func, Func::DffNeg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nand_truth_table() {
        for a in [false, true] {
            for b in [false, true] {
                let out = combinational(Func::Nand2, &[a, b], 0, false);
                assert_eq!(out[0], !(a && b), "a={a} b={b}");
            }
        }
    }

    #[test]
    fn xor_counts_ones() {
        assert!(!combinational(Func::Xor2, &[false, false], 0, false)[0]);
        assert!(combinational(Func::Xor2, &[true, false], 0, false)[0]);
        assert!(!combinational(Func::Xor2, &[true, true], 0, false)[0]);
    }

    #[test]
    fn dff_holds_and_follows() {
        let mut st = State::new();
        // Clock low, data high: nothing happens.
        let out = sequential(Func::Dff, &[true, false], &mut st);
        assert!(!out[0]);
        // Clock goes high: the data is sampled.
        let out = sequential(Func::Dff, &[true, true], &mut st);
        assert!(out[0]);
        assert!(!out[1]);
        // Clock stays high: no new edge, the value is held.
        let out = sequential(Func::Dff, &[false, true], &mut st);
        assert!(out[0]);
    }

    #[test]
    fn counter_counts_up() {
        // The clear input of a TTL counter is active low, so it stays high.
        let mut st = State::new();
        let mut value = 0u64;
        for step in 0..6 {
            // Clock edge: low then high.
            sequential(Func::Counter4, &[false, true], &mut st);
            let out = sequential(Func::Counter4, &[true, true], &mut st);
            value = (value + 1) & 0xF;
            let got: u64 = out
                .iter()
                .enumerate()
                .fold(0, |acc, (i, b)| acc | (u64::from(*b) << i));
            assert_eq!(got, value, "step {step}");
        }
    }

    #[test]
    fn decade_counter_wraps_at_ten() {
        let mut st = State::new();
        // A clock edge is a low level followed by a high level.
        let edge = |st: &mut State| {
            sequential(Func::BcdCounter, &[false, true], st);
            let out = sequential(Func::BcdCounter, &[true, true], st);
            out.iter()
                .enumerate()
                .fold(0u64, |acc, (i, b)| acc | (u64::from(*b) << i))
        };
        for step in 1..=9 {
            assert_eq!(edge(&mut st), step, "step {step}");
        }
        // The tenth edge brings it back to zero: the counter wraps.
        assert_eq!(edge(&mut st), 0);
        assert_eq!(edge(&mut st), 1);
    }
}
