//! The built-in component catalogue: one function per family.
//!
//! The data follows the Multisim component toolbar: basic parts, discrete
//! semiconductors, analog blocks, TTL, TTL Schottky, CMOS and ECL.
//!
//! Packages are described by their real pin maps. A part picks the package
//! that fits its units, and the signal names come from the unit list, so the
//! pin numbers and the logic stay in step.

use crate::library::{Entry, Family, Func, Library, Power, Unit};
use crate::netlist::InstrumentKind;

/// Build an entry from a list of units and a DIP pin map.
fn part(
    key: &str,
    title: &str,
    family: Family,
    prefix: &str,
    units: Vec<Unit>,
    dip: Vec<(u8, &'static str)>,
    power: Power,
) -> Entry {
    Entry {
        key: key.to_string(),
        title: title.to_string(),
        family,
        prefix: prefix.to_string(),
        units,
        dip: dip.into_iter().map(|(n, s)| (n, s.to_string())).collect(),
        power,
        ma: None,
        device: None,
    }
}

/// A part that is one device rather than a set of logic units.
fn device_part(
    key: &str,
    title: &str,
    family: Family,
    prefix: &str,
    device: crate::library::Device,
) -> Entry {
    let mut entry = part(key, title, family, prefix, vec![], vec![], Power::default());
    entry.device = Some(device);
    entry
}

/// Input names of one unit of a gate: `1A`, `1B`, … for the first input and
/// so on. Gates with more than two inputs keep the `A B C D` convention.
fn unit_ins(index: usize, count: usize) -> Vec<String> {
    let letters = ["A", "B", "C", "D", "E"];
    (0..count)
        .map(|i| format!("{index}{}", letters[i]))
        .collect()
}

/// `count` identical units, numbered from 1.
fn repeated(count: usize, func: Func, ins: usize) -> Vec<Unit> {
    (1..=count)
        .map(|n| {
            let inputs = unit_ins(n, ins);
            Unit {
                func,
                ins: inputs,
                outs: vec![format!("{n}Y")],
                sel_bits: None,
                has_enable: false,
            }
        })
        .collect()
}

/// Six inverters or buffers, numbered from 1.
fn simple(count: usize, func: Func) -> Vec<Unit> {
    (1..=count)
        .map(|n| Unit {
            func,
            ins: vec![format!("{n}A")],
            outs: vec![format!("{n}Y")],
            sel_bits: None,
            has_enable: false,
        })
        .collect()
}

// ---------------------------------------------------------------- packages

/// 14-pin DIP with four 2-input gates: the 7400 family.
fn dip14_quad2() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1A"),
        (2, "1B"),
        (3, "1Y"),
        (4, "2A"),
        (5, "2B"),
        (6, "2Y"),
        (7, "GND"),
        (8, "3Y"),
        (9, "3A"),
        (10, "3B"),
        (11, "4Y"),
        (12, "4A"),
        (13, "4B"),
        (14, "VCC"),
    ]
}

/// 14-pin DIP with three 3-input gates: the 7410 family.
fn dip14_triple3() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1A"),
        (2, "1B"),
        (3, "1C"),
        (4, "1Y"),
        (5, "2C"),
        (6, "2B"),
        (7, "2A"),
        (8, "GND"),
        (9, "3A"),
        (10, "3B"),
        (11, "3C"),
        (12, "3Y"),
        (13, "2Y"),
        (14, "VCC"),
    ]
}

/// 14-pin DIP with two 4-input gates: the 7420 family.
fn dip14_dual4() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1A"),
        (2, "1B"),
        (3, "1C"),
        (4, "1D"),
        (5, "1Y"),
        (6, "2A"),
        (7, "2B"),
        (8, "2C"),
        (9, "2D"),
        (10, "2Y"),
        (11, "3Y"),
        (12, "4Y"),
        (13, "GND"),
        (14, "VCC"),
    ]
}

/// 14-pin DIP with six inverters: the 7404 family.
fn dip14_hex1() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1A"),
        (2, "1Y"),
        (3, "2A"),
        (4, "2Y"),
        (5, "3A"),
        (6, "3Y"),
        (7, "GND"),
        (8, "6Y"),
        (9, "6A"),
        (10, "5Y"),
        (11, "5A"),
        (12, "4Y"),
        (13, "4A"),
        (14, "VCC"),
    ]
}

/// 16-pin DIP with two JK flip-flops: the 7476 family.
fn dip16_dual_jk() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1CLK"),
        (2, "1K"),
        (3, "1J"),
        (4, "1Q̄"),
        (5, "1Q"),
        (6, "2Q"),
        (7, "2Q̄"),
        (8, "GND"),
        (9, "2J"),
        (10, "2K"),
        (11, "2CLK"),
        (12, "VCC"),
    ]
}

/// 16-pin DIP with two D flip-flops: the 7474 family.
fn dip16_dual_dff() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1CLR"),
        (2, "1D"),
        (3, "1CLK"),
        (4, "1PRE"),
        (5, "1Q"),
        (6, "1Q̄"),
        (7, "GND"),
        (8, "2Q̄"),
        (9, "2Q"),
        (10, "2PRE"),
        (11, "2CLK"),
        (12, "2D"),
        (13, "2CLR"),
        (14, "VCC"),
    ]
}

/// 16-pin DIP with a 4-bit counter: the 74161 family.
fn dip16_counter4() -> Vec<(u8, &'static str)> {
    vec![
        (1, "CLR"),
        (2, "CLK"),
        (3, "LD"),
        (4, "A"),
        (5, "B"),
        (6, "C"),
        (7, "D"),
        (8, "GND"),
        (9, "Q3"),
        (10, "Q2"),
        (11, "Q1"),
        (12, "Q0"),
        (13, "ENP"),
        (14, "ENT"),
        (15, "RCO"),
        (16, "VCC"),
    ]
}

/// 14-pin DIP with an 8-bit shift register: the 74164 family.
fn dip14_shift8() -> Vec<(u8, &'static str)> {
    vec![
        (1, "CLR"),
        (2, "CLK"),
        (3, "SI"),
        (4, "Q0"),
        (5, "Q1"),
        (6, "Q2"),
        (7, "Q3"),
        (8, "GND"),
        (9, "Q4"),
        (10, "Q5"),
        (11, "Q6"),
        (12, "Q7"),
        (13, "Q8"),
        (14, "VCC"),
    ]
}

/// 16-pin DIP with three JK flip-flops: the 4027 family.
fn dip16_triple_jk() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1J"),
        (2, "1CLK"),
        (3, "1K"),
        (4, "1RST"),
        (5, "1Q"),
        (6, "1Q̄"),
        (7, "1SET"),
        (8, "GND"),
        (9, "3Q̄"),
        (10, "3Q"),
        (11, "3SET"),
        (12, "3K"),
        (13, "3CLK"),
        (14, "3J"),
        (15, "3RST"),
        (16, "VCC"),
    ]
}

/// 16-pin DIP with a dual five-bit comparator.
fn dip16_comparator() -> Vec<(u8, &'static str)> {
    vec![
        (1, "A0"),
        (2, "A1"),
        (3, "A2"),
        (4, "A3"),
        (5, "A4"),
        (6, "B0"),
        (7, "B1"),
        (8, "B2"),
        (9, "B3"),
        (10, "B4"),
        (11, "EQ"),
        (12, "GT"),
        (13, "LT"),
        (14, "GND"),
        (15, "VCC"),
    ]
}

/// 16-pin DIP with three JK flip-flops, each with its own clock.
fn dip16_triple_jk3() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1J"),
        (2, "1CLK"),
        (3, "1K"),
        (4, "1Q"),
        (5, "1Q̄"),
        (6, "2J"),
        (7, "2CLK"),
        (8, "2K"),
        (9, "2Q"),
        (10, "2Q̄"),
        (11, "3J"),
        (12, "3CLK"),
        (13, "3K"),
        (14, "3Q"),
        (15, "3Q̄"),
        (16, "GND"),
    ]
}

/// 16-pin DIP with an 8-to-1 multiplexer: the 74151 family.
fn dip16_mux8() -> Vec<(u8, &'static str)> {
    vec![
        (1, "I3"),
        (2, "I2"),
        (3, "I1"),
        (4, "I0"),
        (5, "Y"),
        (6, "Ȳ"),
        (7, "EN"),
        (8, "GND"),
        (9, "S2"),
        (10, "S1"),
        (11, "S0"),
        (12, "I7"),
        (13, "I6"),
        (14, "I5"),
        (15, "I4"),
        (16, "VCC"),
    ]
}

/// 16-pin DIP with a decade counter: the 4017 family.
fn dip16_decade() -> Vec<(u8, &'static str)> {
    vec![
        (1, "Q5"),
        (2, "Q4"),
        (3, "Q3"),
        (4, "Q2"),
        (5, "Q1"),
        (6, "Q0"),
        (7, "CLK"),
        (8, "GND"),
        (9, "RST"),
        (10, "Q9"),
        (11, "Q8"),
        (12, "Q7"),
        (13, "Q6"),
        (14, "VCC"),
    ]
}

/// 16-pin DIP with a 12-bit counter: the 4040 family.
fn dip16_counter12() -> Vec<(u8, &'static str)> {
    vec![
        (1, "CLK"),
        (2, "RST"),
        (3, "Q0"),
        (4, "Q1"),
        (4, "Q2"),
        (5, "Q3"),
        (6, "Q4"),
        (7, "GND"),
        (8, "Q5"),
        (9, "Q6"),
        (10, "Q7"),
        (11, "Q8"),
        (12, "Q9"),
        (13, "Q10"),
        (14, "Q11"),
        (15, "Q12"),
        (16, "VCC"),
    ]
}

/// 16-pin DIP with a 14-bit oscillator counter: the 4060 family.
fn dip16_osc14() -> Vec<(u8, &'static str)> {
    vec![
        (1, "OSC1"),
        (2, "OSC2"),
        (3, "RST"),
        (4, "CLK"),
        (5, "Q4"),
        (6, "GND"),
        (7, "Q5"),
        (8, "Q6"),
        (9, "Q7"),
        (10, "Q8"),
        (11, "Q9"),
        (12, "Q10"),
        (13, "Q11"),
        (14, "Q12"),
        (15, "Q13"),
        (16, "VCC"),
    ]
}

/// 16-pin DIP with a monostable: the 74121 family.
fn dip16_monostable() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1A"),
        (2, "1B"),
        (3, "1CLR"),
        (4, "1Q"),
        (5, "1Q̄"),
        (6, "2Q̄"),
        (7, "2Q"),
        (8, "2CLR"),
        (9, "2B"),
        (10, "2A"),
        (11, "GND"),
        (12, "VCC"),
    ]
}

/// How many inputs a gate unit takes.
fn ins_of(func: Func) -> usize {
    match func {
        Func::And2 | Func::Or2 | Func::Nand2 | Func::Nor2 | Func::Xor2 | Func::Xnor2 => 2,
        Func::And3 | Func::Or3 | Func::Nand3 | Func::Nor3 => 3,
        Func::And4 | Func::Or4 | Func::Nand4 | Func::Nor4 => 4,
        Func::Not1 | Func::Buf1 => 1,
        _ => 2,
    }
}

/// A gate part: pick the package that fits the number of units.
fn gate_part(
    key: &str,
    title: &str,
    family: Family,
    func: Func,
    units: usize,
    power: &Power,
) -> Entry {
    let ins = ins_of(func);
    let units_vec = repeated(units, func, ins);
    let dip = match (units, ins) {
        (4, 2) => dip14_quad2(),
        (3, 3) => dip14_triple3(),
        (2, 4) => dip14_dual4(),
        (6, 1) => dip14_hex1(),
        _ => dip14_quad2(),
    };
    part(key, title, family, "U", units_vec, dip, power.clone())
}

// ------------------------------------------------------------------ basic

/// Parts with no family history: the passives and the sources.
pub fn basic() -> Vec<Entry> {
    let plain = Power::default();
    vec![
        part(
            "resistor",
            "Resistor",
            Family::Basic,
            "R",
            vec![],
            vec![],
            plain.clone(),
        ),
        part(
            "capacitor",
            "Capacitor",
            Family::Basic,
            "C",
            vec![],
            vec![],
            plain.clone(),
        ),
        part(
            "inductor",
            "Inductor",
            Family::Basic,
            "L",
            vec![],
            vec![],
            plain.clone(),
        ),
        part(
            "vsource",
            "Voltage source",
            Family::Basic,
            "V",
            vec![],
            vec![],
            plain.clone(),
        ),
        part(
            "isource",
            "Current source",
            Family::Basic,
            "I",
            vec![],
            vec![],
            plain.clone(),
        ),
        part(
            "switch",
            "Switch",
            Family::Basic,
            "SW",
            vec![],
            vec![],
            plain.clone(),
        ),
        part(
            "ground",
            "Ground",
            Family::Basic,
            "GND",
            vec![],
            vec![],
            plain.clone(),
        ),
        part(
            "mcu_pin",
            "Microcontroller pin",
            Family::Basic,
            "P",
            vec![],
            vec![],
            plain.clone(),
        ),
        part(
            "probe",
            "Probe",
            Family::Basic,
            "X",
            vec![],
            vec![],
            plain.clone(),
        ),
    ]
}

/// Diodes and transistors.
pub fn discrete() -> Vec<Entry> {
    use crate::library::Device;
    vec![
        device_part("diode", "Diode", Family::Discrete, "D", Device::Diode),
        device_part(
            "led",
            "Light-emitting diode",
            Family::Discrete,
            "LED",
            Device::Led,
        ),
        device_part(
            "zener",
            "Zener diode",
            Family::Discrete,
            "D",
            Device::Zener,
        ),
        device_part(
            "npn",
            "NPN transistor",
            Family::Discrete,
            "Q",
            Device::Bjt { npn: true },
        ),
        device_part(
            "pnp",
            "PNP transistor",
            Family::Discrete,
            "Q",
            Device::Bjt { npn: false },
        ),
        device_part(
            "nmfet",
            "N-channel MOSFET",
            Family::Discrete,
            "Q",
            Device::Mosfet { nch: true },
        ),
        device_part(
            "pmfet",
            "P-channel MOSFET",
            Family::Discrete,
            "Q",
            Device::Mosfet { nch: false },
        ),
    ]
}

/// Amplifiers, comparators and timers.
pub fn analog() -> Vec<Entry> {
    use crate::library::Device;
    vec![
        device_part(
            "opamp",
            "Operational amplifier",
            Family::Analog,
            "U",
            Device::OpAmp,
        ),
        device_part(
            "comparator",
            "Comparator",
            Family::Analog,
            "U",
            Device::Comparator,
        ),
        device_part(
            "timer555",
            "Timer 555",
            Family::Analog,
            "U",
            Device::Timer555,
        ),
    ]
}

// -------------------------------------------------------------------- TTL

/// The original 74xx bipolar TTL series.
pub fn ttl() -> Vec<Entry> {
    let p = Power::ttl();
    let mut out = Vec::new();

    // Quad 2-input gates.
    for (key, title, func) in [
        ("7400", "Quad 2-input NAND", Func::Nand2),
        ("7401", "Quad 2-input NAND, open collector", Func::Nand2),
        ("7402", "Quad 2-input NOR", Func::Nor2),
        ("7403", "Quad 2-input NAND, open collector", Func::Nand2),
        ("7408", "Quad 2-input AND", Func::And2),
        ("7426", "Quad 2-input NAND, 15 V", Func::Nand2),
        ("7429", "Quad 2-input NOR, open collector", Func::Nor2),
        ("7432", "Quad 2-input OR", Func::Or2),
        ("7433", "Quad 2-input NOR, open collector", Func::Nor2),
        ("7437", "Quad 2-input NAND buffer", Func::Nand2),
        ("7464", "Quad 2-input XOR, open collector", Func::Xor2),
        ("7465", "Quad 2-input NOR, open collector", Func::Nor2),
        ("7486", "Quad 2-input XOR", Func::Xor2),
        ("7498", "Quad 2-input XOR", Func::Xor2),
        ("7499", "Quad 2-input NOR buffer", Func::Nor2),
    ] {
        let units = match (key, func) {
            ("7412", _) => repeated(3, func, 3),
            _ => repeated(4, func, 2),
        };
        out.push(part(
            key,
            title,
            Family::Ttl,
            "U",
            units,
            dip14_quad2(),
            p.clone(),
        ));
    }

    // Triple 3-input gates.
    for (key, title, func) in [
        ("7410", "Triple 3-input NAND", Func::Nand3),
        ("7411", "Triple 3-input AND", Func::And3),
        ("7412", "Triple 3-input NAND, open collector", Func::Nand3),
        ("7415", "Triple 3-input AND, open collector", Func::And3),
        ("7427", "Triple 3-input NOR", Func::Nor3),
    ] {
        out.push(part(
            key,
            title,
            Family::Ttl,
            "U",
            repeated(3, func, 3),
            dip14_triple3(),
            p.clone(),
        ));
    }

    // Dual 2-input gates on the quad package.
    out.push(part(
        "7475",
        "Dual 2-input NOR, strobed",
        Family::Ttl,
        "U",
        repeated(2, Func::Nor2, 2),
        dip14_quad2(),
        p.clone(),
    ));

    // Dual 4-input gates.
    for (key, title, func) in [
        ("7413", "Dual 4-input Schmitt NAND", Func::Nand4),
        ("7420", "Dual 4-input NAND", Func::Nand4),
        ("7421", "Dual 4-input AND", Func::And4),
        ("7423", "Dual 4-input NOR", Func::Nor4),
        ("7425", "Dual 4-input NOR, strobed", Func::Nor4),
        ("7440", "Dual 4-input NAND buffer", Func::Nand4),
        ("7442", "Dual 4-input NOR", Func::Nor4),
        ("7450", "Dual 4-input NOR, strobed", Func::Nor4),
        ("7451", "Dual 4-input AND, strobed", Func::And4),
        ("7453", "Expandable 4-input AND", Func::And4),
        ("7455", "Dual 4-input NOR", Func::Nor4),
        ("7480", "Dual 4-input NAND buffer", Func::Nand4),
    ] {
        out.push(part(
            key,
            title,
            Family::Ttl,
            "U",
            repeated(2, func, 4),
            dip14_dual4(),
            p.clone(),
        ));
    }

    // Hex inverters and buffers.
    for (key, title, func) in [
        ("7404", "Hex inverter", Func::Not1),
        ("7405", "Hex inverter, open collector", Func::Not1),
        ("7406", "Hex inverter, open collector", Func::Not1),
        ("7407", "Hex buffer, open collector", Func::Buf1),
        ("7414", "Hex Schmitt inverter", Func::Not1),
        ("7416", "Hex inverter, open collector", Func::Not1),
        ("7417", "Hex buffer, open collector", Func::Buf1),
        ("7446", "Hex inverter buffer", Func::Not1),
    ] {
        out.push(part(
            key,
            title,
            Family::Ttl,
            "U",
            simple(6, func),
            dip14_hex1(),
            p.clone(),
        ));
    }

    // Flip-flops, counters and registers.
    out.push(part(
        "7474",
        "Dual D flip-flop, preset, clear",
        Family::Ttl,
        "U",
        vec![
            Unit::new(Func::Dff, &["1D", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Dff, &["2D", "2CLK"], &["2Q", "2Q̄"]),
        ],
        dip16_dual_dff(),
        p.clone(),
    ));
    out.push(part(
        "7476",
        "Dual JK flip-flop, preset, clear",
        Family::Ttl,
        "U",
        vec![
            Unit::new(Func::Jk, &["1J", "1K", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Jk, &["2J", "2K", "2CLK"], &["2Q", "2Q̄"]),
        ],
        dip16_dual_jk(),
        p.clone(),
    ));
    out.push(part(
        "7483",
        "4-bit binary counter",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::Counter4,
            &["CLK", "CLR"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_counter4(),
        p.clone(),
    ));
    out.push(part(
        "7490",
        "Decade counter",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::BcdCounter,
            &["CLK", "RST"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_decade(),
        p.clone(),
    ));
    out.push(part(
        "7493",
        "4-bit binary counter",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::Counter4,
            &["CLK", "CLR"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_counter4(),
        p.clone(),
    ));
    out.push(part(
        "74121",
        "Dual one-shot monostable",
        Family::Ttl,
        "U",
        vec![
            Unit::new(Func::DffNeg, &["1A", "1B"], &["1Q", "1Q̄"]),
            Unit::new(Func::DffNeg, &["2A", "2B"], &["2Q", "2Q̄"]),
        ],
        dip16_monostable(),
        p.clone(),
    ));
    out.push(part(
        "74151",
        "8-to-1 multiplexer",
        Family::Ttl,
        "U",
        vec![Unit::mux(
            Func::Mux2,
            &[
                "I0", "I1", "I2", "I3", "I4", "I5", "I6", "I7", "S0", "S1", "S2", "EN",
            ],
            &["Y", "Ȳ"],
            3,
            true,
        )],
        dip16_mux8(),
        p.clone(),
    ));
    out.push(part(
        "74161",
        "Synchronous 4-bit binary counter",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::Counter4,
            &["CLK", "CLR", "LD", "A", "B", "C", "D", "ENP", "ENT"],
            &["Q0", "Q1", "Q2", "Q3", "RCO"],
        )],
        dip16_counter4(),
        p.clone(),
    ));
    out.push(part(
        "74164",
        "8-bit serial-in parallel-out shift register",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::Shift8,
            &["CLK", "SI", "CLR"],
            &["Q0", "Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7"],
        )],
        dip14_shift8(),
        p.clone(),
    ));

    // Multiplexers, decoders and the rest of the counters.
    out.push(part(
        "74148",
        "8-to-3 priority encoder",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::Encoder8to3,
            &["1", "2", "3", "4", "5", "6", "7", "8"],
            &["A0", "A1", "A2"],
        )],
        dip16_encoder8(),
        p.clone(),
    ));
    out.push(part(
        "74153",
        "Dual 4-to-1 multiplexer",
        Family::Ttl,
        "U",
        vec![
            Unit::mux(
                Func::Mux2,
                &["1C0", "1C1", "1C2", "1C3", "1S0", "1S1"],
                &["1Y"],
                2,
                false,
            ),
            Unit::mux(
                Func::Mux2,
                &["2C0", "2C1", "2C2", "2C3", "2S0", "2S1"],
                &["2Y"],
                2,
                false,
            ),
        ],
        dip16_mux4(),
        p.clone(),
    ));
    out.push(part(
        "74157",
        "Quad 2-to-1 multiplexer",
        Family::Ttl,
        "U",
        vec![
            Unit::mux(Func::Mux2, &["1A", "1B", "1S"], &["1Y"], 1, false),
            Unit::mux(Func::Mux2, &["2A", "2B", "2S"], &["2Y"], 1, false),
            Unit::mux(Func::Mux2, &["3A", "3B", "3S"], &["3Y"], 1, false),
            Unit::mux(Func::Mux2, &["4A", "4B", "4S"], &["4Y"], 1, false),
        ],
        dip16_mux2(),
        p.clone(),
    ));
    out.push(part(
        "74138",
        "3-to-8 decoder",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::Decoder3to8,
            &["A", "B", "C", "EN"],
            &["Y0", "Y1", "Y2", "Y3", "Y4", "Y5", "Y6", "Y7"],
        )],
        dip16_decoder(),
        p.clone(),
    ));
    out.push(part(
        "74139",
        "Dual 2-to-4 decoder",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::Decoder4to16,
            &["A", "B", "EN", "E1"],
            &["Y0", "Y1", "Y2", "Y3"],
        )],
        dip16_decoder(),
        p.clone(),
    ));
    out.push(part(
        "74174",
        "Quad D flip-flop",
        Family::Ttl,
        "U",
        vec![
            Unit::new(Func::Dff, &["1D", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Dff, &["2D", "1CLK"], &["2Q", "2Q̄"]),
            Unit::new(Func::Dff, &["3D", "3CLK"], &["3Q", "3Q̄"]),
            Unit::new(Func::Dff, &["4D", "3CLK"], &["4Q", "4Q̄"]),
        ],
        dip16_quad_dff(),
        p.clone(),
    ));
    out.push(part(
        "74193",
        "4-bit binary up/down counter",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::Counter4Down,
            &["CLK", "CLR", "UP", "DOWN"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_counter_updown(),
        p.clone(),
    ));
    out.push(part(
        "74194",
        "4-bit bidirectional shift register",
        Family::Ttl,
        "U",
        vec![Unit::new(
            Func::Shift8,
            &["CLK", "SI", "SR", "SL", "S0", "S1"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_shift4(),
        p.clone(),
    ));
    out
}

/// TTL Schottky: the 74LS, 74S and 74HC families.
pub fn ttl_shottky() -> Vec<Entry> {
    let p = Power::ttl();
    let mut out = Vec::new();

    // 74LS series.
    for (key, title, func, units) in [
        ("74LS00", "Quad 2-input NAND", Func::Nand2, 4),
        ("74LS02", "Quad 2-input NOR", Func::Nor2, 4),
        ("74LS04", "Hex inverter", Func::Not1, 6),
        ("74LS08", "Quad 2-input AND", Func::And2, 4),
        ("74LS10", "Triple 3-input NAND", Func::Nand3, 3),
        ("74LS11", "Triple 3-input AND", Func::And3, 3),
        ("74LS14", "Hex Schmitt inverter", Func::Not1, 6),
        ("74LS20", "Dual 4-input NAND", Func::Nand4, 2),
        ("74LS21", "Dual 4-input AND", Func::And4, 2),
        ("74LS32", "Quad 2-input OR", Func::Or2, 4),
        ("74LS86", "Quad 2-input XOR", Func::Xor2, 4),
    ] {
        out.push(gate_part(key, title, Family::TtlSchottky, func, units, &p));
    }

    // 74S series.
    for (key, title, func, units) in [
        ("74S00", "Quad 2-input NAND", Func::Nand2, 4),
        ("74S02", "Quad 2-input NOR", Func::Nor2, 4),
        ("74S04", "Hex inverter", Func::Not1, 6),
        ("74S08", "Quad 2-input AND", Func::And2, 4),
        ("74S32", "Quad 2-input OR", Func::Or2, 4),
        ("74S86", "Quad 2-input XOR", Func::Xor2, 4),
    ] {
        out.push(gate_part(key, title, Family::TtlSchottky, func, units, &p));
    }

    // 74S flip-flops.
    out.push(part(
        "74S112",
        "Dual negative-edge JK flip-flop",
        Family::TtlSchottky,
        "U",
        vec![
            Unit::new(Func::Jk, &["1J", "1K", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Jk, &["2J", "2K", "2CLK"], &["2Q", "2Q̄"]),
        ],
        dip16_dual_jk(),
        p.clone(),
    ));
    out.push(part(
        "74S175",
        "Quad D flip-flop",
        Family::TtlSchottky,
        "U",
        vec![
            Unit::new(Func::Dff, &["1D", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Dff, &["2D", "2CLK"], &["2Q", "2Q̄"]),
        ],
        vec![
            (1, "1CLK"),
            (2, "1D"),
            (3, "1Q"),
            (4, "1Q̄"),
            (5, "1PRE"),
            (6, "1CLR"),
            (7, "GND"),
            (8, "2Q̄"),
            (9, "2Q"),
            (10, "2D"),
            (11, "2CLK"),
            (12, "2PRE"),
            (13, "2CLR"),
            (14, "VCC"),
        ],
        p.clone(),
    ));

    // The 74LS flip-flops, counters and registers.
    out.push(part(
        "74LS74",
        "Dual D flip-flop, preset, clear",
        Family::TtlSchottky,
        "U",
        vec![
            Unit::new(Func::Dff, &["1D", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Dff, &["2D", "2CLK"], &["2Q", "2Q̄"]),
        ],
        dip16_dual_dff(),
        p.clone(),
    ));
    out.push(part(
        "74LS76",
        "Dual JK flip-flop, preset, clear",
        Family::TtlSchottky,
        "U",
        vec![
            Unit::new(Func::Jk, &["1J", "1K", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Jk, &["2J", "2K", "2CLK"], &["2Q", "2Q̄"]),
        ],
        dip16_dual_jk(),
        p.clone(),
    ));
    out.push(part(
        "74LS90",
        "Decade counter",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::BcdCounter,
            &["CLK", "RST"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_decade(),
        p.clone(),
    ));
    out.push(part(
        "74LS93",
        "4-bit binary counter",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::Counter4,
            &["CLK", "CLR"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_counter4(),
        p.clone(),
    ));
    out.push(part(
        "74LS138",
        "3-to-8 decoder",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::Decoder3to8,
            &["A", "B", "C", "EN"],
            &["Y0", "Y1", "Y2", "Y3", "Y4", "Y5", "Y6", "Y7"],
        )],
        dip16_decoder(),
        p.clone(),
    ));
    out.push(part(
        "74LS139",
        "Dual 2-to-4 decoder",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::Decoder4to16,
            &["A", "B", "EN", "E1"],
            &["Y0", "Y1", "Y2", "Y3"],
        )],
        dip16_decoder(),
        p.clone(),
    ));
    out.push(part(
        "74LS151",
        "8-to-1 multiplexer",
        Family::TtlSchottky,
        "U",
        vec![Unit::mux(
            Func::Mux2,
            &[
                "I0", "I1", "I2", "I3", "I4", "I5", "I6", "I7", "S0", "S1", "S2", "EN",
            ],
            &["Y", "Ȳ"],
            3,
            true,
        )],
        dip16_mux8(),
        p.clone(),
    ));
    out.push(part(
        "74LS153",
        "Dual 4-to-1 multiplexer",
        Family::TtlSchottky,
        "U",
        vec![
            Unit::mux(
                Func::Mux2,
                &["1C0", "1C1", "1C2", "1C3", "1S0", "1S1"],
                &["1Y"],
                2,
                false,
            ),
            Unit::mux(
                Func::Mux2,
                &["2C0", "2C1", "2C2", "2C3", "2S0", "2S1"],
                &["2Y"],
                2,
                false,
            ),
        ],
        dip16_mux4(),
        p.clone(),
    ));
    out.push(part(
        "74LS163",
        "Synchronous 4-bit binary counter",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::Counter4,
            &["CLK", "CLR", "LD", "A", "B", "C", "D", "ENP", "ENT"],
            &["Q0", "Q1", "Q2", "Q3", "RCO"],
        )],
        dip16_counter4(),
        p.clone(),
    ));
    out.push(part(
        "74LS164",
        "8-bit serial-in parallel-out shift register",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::Shift8,
            &["CLK", "SI", "CLR"],
            &["Q0", "Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7"],
        )],
        dip14_shift8(),
        p.clone(),
    ));
    out.push(part(
        "74LS174",
        "Quad D flip-flop",
        Family::TtlSchottky,
        "U",
        vec![
            Unit::new(Func::Dff, &["1D", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Dff, &["2D", "1CLK"], &["2Q", "2Q̄"]),
            Unit::new(Func::Dff, &["3D", "3CLK"], &["3Q", "3Q̄"]),
            Unit::new(Func::Dff, &["4D", "3CLK"], &["4Q", "4Q̄"]),
        ],
        dip16_quad_dff(),
        p.clone(),
    ));
    out.push(part(
        "74LS192",
        "Synchronous decade counter, up/down",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::Counter4Down,
            &["CLK", "CLR", "UP", "DOWN"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_counter_updown(),
        p.clone(),
    ));
    out.push(part(
        "74LS193",
        "4-bit binary up/down counter",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::Counter4Down,
            &["CLK", "CLR", "UP", "DOWN"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_counter_updown(),
        p.clone(),
    ));
    out.push(part(
        "74LS194",
        "4-bit bidirectional shift register",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::Shift8,
            &["CLK", "SI", "SR", "SL", "S0", "S1"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_shift4(),
        p.clone(),
    ));

    // 74HC series: CMOS levels on the classic TTL pinouts.
    for (key, title, func, units) in [
        ("74HC00", "Quad 2-input NAND", Func::Nand2, 4),
        ("74HC02", "Quad 2-input NOR", Func::Nor2, 4),
        ("74HC04", "Hex inverter", Func::Not1, 6),
        ("74HC08", "Quad 2-input AND", Func::And2, 4),
        ("74HC14", "Hex Schmitt inverter", Func::Not1, 6),
        ("74HC32", "Quad 2-input OR", Func::Or2, 4),
        ("74HC86", "Quad 2-input XOR", Func::Xor2, 4),
    ] {
        out.push(gate_part(
            key,
            title,
            Family::TtlSchottky,
            func,
            units,
            &Power::cmos(),
        ));
    }
    for (key, title, dip) in [
        ("74HC74", "Dual D flip-flop, preset, clear", 0u8),
        ("74HC76", "Dual JK flip-flop, preset, clear", 1),
        ("74HC173", "Quad D flip-flop with enable", 2),
        ("74HC175", "Quad D flip-flop", 2),
    ] {
        let units = match (key, dip) {
            ("74HC74", _) => vec![
                Unit::new(Func::Dff, &["1D", "1CLK"], &["1Q", "1Q̄"]),
                Unit::new(Func::Dff, &["2D", "2CLK"], &["2Q", "2Q̄"]),
            ],
            ("74HC76", _) => vec![
                Unit::new(Func::Jk, &["1J", "1K", "1CLK"], &["1Q", "1Q̄"]),
                Unit::new(Func::Jk, &["2J", "2K", "2CLK"], &["2Q", "2Q̄"]),
            ],
            ("74HC173", _) => vec![
                Unit::new(Func::Dff, &["1D", "1CLK"], &["1Q", "1Q̄"]),
                Unit::new(Func::Dff, &["2D", "1CLK"], &["2Q", "2Q̄"]),
                Unit::new(Func::Dff, &["3D", "3CLK"], &["3Q", "3Q̄"]),
                Unit::new(Func::Dff, &["4D", "3CLK"], &["4Q", "4Q̄"]),
            ],
            _ => vec![
                Unit::new(Func::Dff, &["1D", "1CLK"], &["1Q", "1Q̄"]),
                Unit::new(Func::Dff, &["2D", "1CLK"], &["2Q", "2Q̄"]),
                Unit::new(Func::Dff, &["3D", "3CLK"], &["3Q", "3Q̄"]),
                Unit::new(Func::Dff, &["4D", "3CLK"], &["4Q", "4Q̄"]),
            ],
        };
        let dip_map = match dip {
            0 => dip16_dual_dff(),
            1 => dip16_dual_jk(),
            _ => dip16_quad_dff(),
        };
        out.push(part(
            key,
            title,
            Family::TtlSchottky,
            "U",
            units,
            dip_map,
            Power::cmos(),
        ));
        let _ = units;
    }
    for (key, title) in [
        ("74HC138", "3-to-8 decoder"),
        ("74HC139", "Dual 2-to-4 decoder"),
    ] {
        let units = if key == "74HC138" {
            vec![Unit::new(
                Func::Decoder3to8,
                &["A", "B", "C", "EN"],
                &["Y0", "Y1", "Y2", "Y3", "Y4", "Y5", "Y6", "Y7"],
            )]
        } else {
            vec![Unit::new(
                Func::Decoder4to16,
                &["A", "B", "EN", "E1"],
                &["Y0", "Y1", "Y2", "Y3"],
            )]
        };
        out.push(part(
            key,
            title,
            Family::TtlSchottky,
            "U",
            units,
            dip16_decoder(),
            Power::cmos(),
        ));
    }
    for (key, title) in [
        ("74HC151", "8-to-1 multiplexer"),
        ("74HC153", "Dual 4-to-1 multiplexer"),
    ] {
        let units = if key == "74HC151" {
            vec![Unit::mux(
                Func::Mux2,
                &[
                    "I0", "I1", "I2", "I3", "I4", "I5", "I6", "I7", "S0", "S1", "S2", "EN",
                ],
                &["Y", "Ȳ"],
                3,
                true,
            )]
        } else {
            vec![
                Unit::mux(
                    Func::Mux2,
                    &["1C0", "1C1", "1C2", "1C3", "1S0", "1S1"],
                    &["1Y"],
                    2,
                    false,
                ),
                Unit::mux(
                    Func::Mux2,
                    &["2C0", "2C1", "2C2", "2C3", "2S0", "2S1"],
                    &["2Y"],
                    2,
                    false,
                ),
            ]
        };
        let dip_map = if key == "74HC151" {
            dip16_mux8()
        } else {
            dip16_mux4()
        };
        out.push(part(
            key,
            title,
            Family::TtlSchottky,
            "U",
            units,
            dip_map,
            Power::cmos(),
        ));
    }
    for (key, title) in [
        ("74HC163", "Synchronous 4-bit binary counter"),
        ("74HC192", "Synchronous decade counter, up/down"),
        ("74HC193", "4-bit binary up/down counter"),
    ] {
        let units = if key == "74HC163" {
            vec![Unit::new(
                Func::Counter4,
                &["CLK", "CLR", "LD", "A", "B", "C", "D", "ENP", "ENT"],
                &["Q0", "Q1", "Q2", "Q3", "RCO"],
            )]
        } else {
            vec![Unit::new(
                Func::Counter4Down,
                &["CLK", "CLR", "UP", "DOWN"],
                &["Q0", "Q1", "Q2", "Q3"],
            )]
        };
        let dip_map = if key == "74HC163" {
            dip16_counter4()
        } else {
            dip16_counter_updown()
        };
        out.push(part(
            key,
            title,
            Family::TtlSchottky,
            "U",
            units,
            dip_map,
            Power::cmos(),
        ));
    }
    out.push(part(
        "74HC164",
        "8-bit serial-in parallel-out shift register",
        Family::TtlSchottky,
        "U",
        vec![Unit::new(
            Func::Shift8,
            &["CLK", "SI", "CLR"],
            &["Q0", "Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7"],
        )],
        dip14_shift8(),
        Power::cmos(),
    ));
    out
}

/// CMOS: the CD4000 series.
pub fn cmos() -> Vec<Entry> {
    let p = Power::cmos();
    let mut out = Vec::new();

    for (key, title, func, units) in [
        ("4001", "Quad 2-input NOR", Func::Nor2, 4),
        ("4002", "Quad 2-input NOR", Func::Nor2, 4),
        ("4011", "Quad 2-input NAND", Func::Nand2, 4),
        ("4012", "Quad 2-input NAND", Func::Nand2, 4),
        ("4023", "Triple 3-input NAND", Func::Nand3, 3),
        ("4025", "Triple 3-input NOR", Func::Nor3, 3),
        ("4030", "Quad 2-input XOR", Func::Xor2, 4),
        ("4070", "Quad 2-input XOR", Func::Xor2, 4),
        ("4071", "Quad 2-input OR", Func::Or2, 4),
        ("4075", "Triple 3-input OR", Func::Or3, 3),
        ("4081", "Quad 2-input AND", Func::And2, 4),
        ("4093", "Quad 2-input Schmitt NAND", Func::Nand2, 4),
    ] {
        out.push(gate_part(key, title, Family::Cmos, func, units, &p));
    }

    out.push(part(
        "4013",
        "Dual D flip-flop, preset, clear",
        Family::Cmos,
        "U",
        vec![
            Unit::new(Func::Dff, &["1D", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Dff, &["2D", "2CLK"], &["2Q", "2Q̄"]),
        ],
        dip16_dual_dff(),
        p.clone(),
    ));
    out.push(part(
        "4027",
        "Triple JK flip-flop, preset, clear",
        Family::Cmos,
        "U",
        vec![
            Unit::new(Func::Jk, &["1J", "1K", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Jk, &["3J", "3K", "3CLK"], &["3Q", "3Q̄"]),
        ],
        dip16_triple_jk(),
        p.clone(),
    ));
    out.push(part(
        "4017",
        "Decade counter with decoder",
        Family::Cmos,
        "U",
        vec![Unit::new(
            Func::BcdCounter,
            &["CLK", "RST"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_decade(),
        p.clone(),
    ));
    out.push(part(
        "4040",
        "12-bit binary counter",
        Family::Cmos,
        "U",
        vec![Unit::new(
            Func::Counter4,
            &["CLK", "RST"],
            &["Q0", "Q1", "Q2", "Q3", "Q4", "Q5", "Q6", "Q7"],
        )],
        dip16_counter12(),
        p.clone(),
    ));
    out.push(part(
        "4060",
        "14-bit ripple counter with oscillator",
        Family::Cmos,
        "U",
        vec![Unit::new(
            Func::Counter4,
            &["CLK", "RST"],
            &["Q4", "Q5", "Q6", "Q7", "Q8", "Q9", "Q10", "Q11"],
        )],
        dip16_osc14(),
        p.clone(),
    ));
    out.push(part(
        "4098",
        "Monostable multivibrator",
        Family::Cmos,
        "U",
        vec![Unit::new(Func::DffNeg, &["1A", "1B"], &["1Q", "1Q̄"])],
        dip16_monostable(),
        p.clone(),
    ));
    out.push(part(
        "4003",
        "Triple JK flip-flop",
        Family::Cmos,
        "U",
        vec![
            Unit::new(Func::Jk, &["1J", "1K", "1CLK"], &["1Q", "1Q̄"]),
            Unit::new(Func::Jk, &["2J", "2K", "2CLK"], &["2Q", "2Q̄"]),
            Unit::new(Func::Jk, &["3J", "3K", "3CLK"], &["3Q", "3Q̄"]),
        ],
        dip16_triple_jk3(),
        p.clone(),
    ));
    out.push(part(
        "4020",
        "14-bit ripple counter",
        Family::Cmos,
        "U",
        vec![Unit::new(
            Func::Counter4,
            &["CLK", "RST"],
            &["Q4", "Q5", "Q6", "Q7", "Q8", "Q9", "Q10", "Q11"],
        )],
        dip16_osc14(),
        p.clone(),
    ));
    out.push(part(
        "4042",
        "Quad D latch",
        Family::Cmos,
        "U",
        vec![
            Unit::new(Func::Dff, &["1D", "1EN"], &["1Q"]),
            Unit::new(Func::Dff, &["2D", "2EN"], &["2Q"]),
            Unit::new(Func::Dff, &["3D", "3EN"], &["3Q"]),
            Unit::new(Func::Dff, &["4D", "4EN"], &["4Q"]),
        ],
        dip16_quad_latch(),
        p.clone(),
    ));
    out.push(part(
        "4051",
        "8-to-1 analogue multiplexer",
        Family::Cmos,
        "U",
        vec![Unit::mux(
            Func::Mux2,
            &[
                "I0", "I1", "I2", "I3", "I4", "I5", "I6", "I7", "S0", "S1", "S2", "EN",
            ],
            &["Y"],
            3,
            true,
        )],
        dip16_mux8(),
        p.clone(),
    ));
    out.push(part(
        "4053",
        "Triple 2-to-1 multiplexer",
        Family::Cmos,
        "U",
        vec![
            Unit::mux(Func::Mux2, &["1A", "1B", "1S"], &["1Y"], 1, false),
            Unit::mux(Func::Mux2, &["2A", "2B", "2S"], &["2Y"], 1, false),
            Unit::mux(Func::Mux2, &["3A", "3B", "3S"], &["3Y"], 1, false),
        ],
        dip16_mux2(),
        p.clone(),
    ));
    out.push(part(
        "4511",
        "BCD-to-7-segment decoder and driver",
        Family::Cmos,
        "U",
        vec![Unit::new(
            Func::Decoder4to16,
            &["A", "B", "C", "D", "LT", "BI", "EI"],
            &["a", "b", "c", "d", "e", "f", "g"],
        )],
        dip16_decoder7(),
        p.clone(),
    ));
    out.push(part(
        "4518",
        "Dual precision monostable multivibrator",
        Family::Cmos,
        "U",
        vec![
            Unit::new(Func::DffNeg, &["1A", "1B"], &["1Q", "1Q̄"]),
            Unit::new(Func::DffNeg, &["2A", "2B"], &["2Q", "2Q̄"]),
        ],
        dip16_monostable(),
        p.clone(),
    ));
    out.push(part(
        "4520",
        "4-bit synchronous binary counter",
        Family::Cmos,
        "U",
        vec![Unit::new(
            Func::Counter4,
            &["CLK", "CLR"],
            &["Q0", "Q1", "Q2", "Q3"],
        )],
        dip16_counter4(),
        p.clone(),
    ));
    out.push(part(
        "4530",
        "Dual 5-bit comparator",
        Family::Cmos,
        "U",
        vec![Unit::new(
            Func::Mux2,
            &["A0", "A1", "A2", "A3", "A4", "B0", "B1", "B2", "B3", "B4"],
            &["EQ", "GT", "LT"],
        )],
        dip16_comparator(),
        p.clone(),
    ));
    out
}

/// ECL: the 10xxx family.
pub fn ecl() -> Vec<Entry> {
    let p = Power {
        vdd: -5.2,
        vth: -1.1,
        ..Power::default()
    };
    let mut out = Vec::new();
    for (key, title, func, units) in [
        ("10100", "Quad 2-input NOR", Func::Nor2, 4),
        ("10101", "Quad 2-input NOR", Func::Nor2, 4),
        ("10102", "Quad 2-input NOR", Func::Nor2, 4),
        ("10104", "Quad 2-input OR", Func::Or2, 4),
        ("10105", "Quad 2-input OR", Func::Or2, 4),
        ("10106", "Quad 2-input NOR", Func::Nor2, 4),
        ("10107", "Quad 2-input XOR", Func::Xor2, 4),
        ("10108", "Quad 2-input OR", Func::Or2, 4),
        ("10109", "Quad 2-input NOR", Func::Nor2, 4),
        ("10112", "Quad 2-input NOR", Func::Nor2, 4),
        ("10113", "Quad 2-input OR", Func::Or2, 4),
        ("10114", "Quad 2-input NOR", Func::Nor2, 4),
        ("10122", "Quad 2-input NOR", Func::Nor2, 4),
        ("10124", "Quad 2-input OR", Func::Or2, 4),
        ("10125", "Quad 2-input NOR", Func::Nor2, 4),
    ] {
        out.push(gate_part(key, title, Family::Ecl, func, units, &p));
    }
    for (key, title, func) in [
        ("10110", "Triple 3-input NOR", Func::Nor3),
        ("10111", "Triple 3-input OR", Func::Or3),
        ("10116", "Triple 3-input NOR", Func::Nor3),
        ("10117", "Triple 3-input OR", Func::Or3),
    ] {
        out.push(part(
            key,
            title,
            Family::Ecl,
            "U",
            repeated(3, func, 3),
            dip14_triple3(),
            p.clone(),
        ));
    }
    out.push(part(
        "10121",
        "Dual 4-input NOR",
        Family::Ecl,
        "U",
        repeated(2, Func::Nor4, 4),
        dip14_dual4(),
        p.clone(),
    ));
    out.push(part(
        "10115",
        "Triple 3-input NOR",
        Family::Ecl,
        "U",
        repeated(3, Func::Nor3, 3),
        dip14_triple3(),
        p.clone(),
    ));
    for (key, title, func, units) in [
        ("10119", "Quad 2-input NOR", Func::Nor2, 4),
        ("10123", "Quad 2-input NOR", Func::Nor2, 4),
        ("10129", "Quad 2-input NOR", Func::Nor2, 4),
        ("10216", "Quad 2-input OR", Func::Or2, 4),
        ("10222", "Quad 2-input NOR", Func::Nor2, 4),
        ("10224", "Quad 2-input NOR", Func::Nor2, 4),
        ("10225", "Quad 2-input NOR", Func::Nor2, 4),
        ("10226", "Quad 2-input NOR", Func::Nor2, 4),
    ] {
        out.push(gate_part(key, title, Family::Ecl, func, units, &p));
    }
    for (key, title) in [
        ("10217", "Hex driver"),
        ("10230", "Hex non-inverting driver"),
        ("10231", "50-ohm driver"),
        ("10232", "Dual OR and AND driver"),
    ] {
        out.push(gate_part(key, title, Family::Ecl, Func::Buf1, 6, &p));
    }
    out.push(part(
        "10221",
        "Dual 4-input NOR",
        Family::Ecl,
        "U",
        repeated(2, Func::Nor4, 4),
        dip14_dual4(),
        p.clone(),
    ));
    out
}

/// 16-pin DIP with an 8-to-3 priority encoder.
fn dip16_encoder8() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1"),
        (2, "2"),
        (3, "3"),
        (4, "4"),
        (5, "5"),
        (6, "6"),
        (7, "7"),
        (8, "8"),
        (9, "EI"),
        (10, "GS"),
        (11, "A0"),
        (12, "A1"),
        (13, "A2"),
        (14, "VCC"),
        (15, "GND"),
    ]
}

/// 16-pin DIP with two 4-to-1 multiplexers.
fn dip16_mux4() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1C0"),
        (2, "1C1"),
        (3, "1C2"),
        (4, "1C3"),
        (5, "1Y"),
        (6, "1S0"),
        (7, "1S1"),
        (8, "2Y"),
        (9, "GND"),
        (10, "2C0"),
        (11, "2C1"),
        (12, "2C2"),
        (13, "2C3"),
        (14, "2S0"),
        (15, "2S1"),
        (16, "VCC"),
    ]
}

/// 16-pin DIP with four 2-to-1 multiplexers.
fn dip16_mux2() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1A"),
        (2, "1B"),
        (3, "1Y"),
        (4, "1S"),
        (5, "2A"),
        (6, "2B"),
        (7, "2Y"),
        (8, "2S"),
        (9, "3A"),
        (10, "3B"),
        (11, "3Y"),
        (12, "3S"),
        (13, "4A"),
        (14, "4B"),
        (15, "4Y"),
        (16, "4S"),
    ]
}

/// 16-pin DIP with a 3-to-8 decoder.
fn dip16_decoder() -> Vec<(u8, &'static str)> {
    vec![
        (1, "A"),
        (2, "B"),
        (3, "C"),
        (4, "E1"),
        (5, "E2"),
        (6, "Y0"),
        (7, "Y1"),
        (8, "GND"),
        (9, "Y2"),
        (10, "Y3"),
        (11, "Y4"),
        (12, "Y5"),
        (13, "Y6"),
        (14, "Y7"),
        (15, "EN"),
        (16, "VCC"),
    ]
}

/// 16-pin DIP with four D flip-flops.
///
/// The first pair shares one clock and so does the second pair, the way
/// the 74174 is wired.
fn dip16_quad_dff() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1D"),
        (2, "1Q"),
        (3, "1Q̄"),
        (4, "2D"),
        (5, "2Q"),
        (6, "2Q̄"),
        (7, "3D"),
        (8, "3Q"),
        (9, "3Q̄"),
        (10, "4D"),
        (11, "4Q"),
        (12, "4Q̄"),
        (13, "1CLK"),
        (14, "3CLK"),
        (15, "GND"),
        (16, "VCC"),
    ]
}

/// 16-pin DIP with an up/down counter: the 74193 family.
fn dip16_counter_updown() -> Vec<(u8, &'static str)> {
    vec![
        (1, "CLK"),
        (2, "CLR"),
        (3, "UP"),
        (4, "DOWN"),
        (5, "LD"),
        (6, "A"),
        (7, "B"),
        (8, "C"),
        (9, "D"),
        (10, "GND"),
        (11, "Q0"),
        (12, "Q1"),
        (13, "Q2"),
        (14, "Q3"),
        (15, "TC"),
        (16, "VCC"),
    ]
}

/// 16-pin DIP with a 4-bit bidirectional shift register.
fn dip16_shift4() -> Vec<(u8, &'static str)> {
    vec![
        (1, "CLR"),
        (2, "SL"),
        (3, "SR"),
        (4, "S0"),
        (5, "S1"),
        (6, "Q0"),
        (7, "Q1"),
        (8, "GND"),
        (9, "Q2"),
        (10, "Q3"),
        (11, "CLK"),
        (12, "SI"),
        (13, "VCC"),
    ]
}

/// 16-pin DIP with four D latches.
fn dip16_quad_latch() -> Vec<(u8, &'static str)> {
    vec![
        (1, "1D"),
        (2, "1EN"),
        (3, "1Q"),
        (4, "2D"),
        (5, "2EN"),
        (6, "2Q"),
        (7, "3D"),
        (8, "3EN"),
        (9, "3Q"),
        (10, "4D"),
        (11, "4EN"),
        (12, "4Q"),
        (13, "GND"),
        (14, "LT"),
        (15, "BI"),
        (16, "VCC"),
    ]
}

/// 16-pin DIP with a BCD-to-seven-segment decoder.
fn dip16_decoder7() -> Vec<(u8, &'static str)> {
    vec![
        (1, "LT"),
        (2, "BI"),
        (3, "EI"),
        (4, "f"),
        (5, "g"),
        (6, "a"),
        (7, "b"),
        (8, "c"),
        (9, "d"),
        (10, "e"),
        (11, "GND"),
        (12, "D"),
        (13, "C"),
        (14, "B"),
        (15, "A"),
        (16, "VCC"),
    ]
}

/// The virtual instruments, the way Multisim puts them on the toolbar.
pub fn instruments() -> Vec<Entry> {
    let mut out = Vec::new();
    for kind in [
        InstrumentKind::Scope,
        InstrumentKind::Multimeter,
        InstrumentKind::FunctionGenerator,
        InstrumentKind::BodePlotter,
        InstrumentKind::LogicAnalyzer,
        InstrumentKind::Wattmeter,
        InstrumentKind::FrequencyCounter,
    ] {
        let channels = kind.channel_count();
        // The pins carry the channel names, the way Multisim labels them.
        let mut pins: Vec<String> = Vec::new();
        for i in 0..channels {
            pins.push(channel_label(kind, i));
        }
        let dip: Vec<(u8, String)> = pins
            .iter()
            .enumerate()
            .map(|(i, name)| (i as u8 + 1, name.clone()))
            .collect();
        let mut entry = part(
            kind.key(),
            instrument_title(kind),
            Family::Instrument,
            kind.prefix(),
            vec![],
            vec![],
            Power::default(),
        );
        entry.dip = dip;
        out.push(entry);
    }
    out
}

/// The name of one channel of an instrument.
pub fn channel_label(kind: InstrumentKind, index: usize) -> String {
    match kind {
        InstrumentKind::Scope => match index {
            0 => "A".to_string(),
            1 => "B".to_string(),
            2 => "C".to_string(),
            _ => "D".to_string(),
        },
        InstrumentKind::Multimeter => "+".to_string(),
        InstrumentKind::FunctionGenerator => "OUT".to_string(),
        InstrumentKind::BodePlotter => match index {
            0 => "IN".to_string(),
            _ => "OUT".to_string(),
        },
        InstrumentKind::LogicAnalyzer => format!("{index}"),
        InstrumentKind::Wattmeter => match index {
            0 => "V".to_string(),
            _ => "I".to_string(),
        },
        InstrumentKind::FrequencyCounter => "IN".to_string(),
    }
}

/// The title of an instrument in the palette.
pub fn instrument_title(kind: InstrumentKind) -> &'static str {
    match kind {
        InstrumentKind::Scope => "Oscilloscope",
        InstrumentKind::Multimeter => "Multimeter",
        InstrumentKind::FunctionGenerator => "Function generator",
        InstrumentKind::BodePlotter => "Bode plotter",
        InstrumentKind::LogicAnalyzer => "Logic analyzer",
        InstrumentKind::Wattmeter => "Wattmeter",
        InstrumentKind::FrequencyCounter => "Frequency counter",
    }
}

/// The catalogue as one library.
pub fn all() -> Library {
    let mut lib = Library::new("WireASM elements");
    lib.entries.extend(basic());
    lib.entries.extend(discrete());
    lib.entries.extend(analog());
    lib.entries.extend(ttl());
    lib.entries.extend(ttl_shottky());
    lib.entries.extend(cmos());
    lib.entries.extend(ecl());
    lib.entries.extend(instruments());
    lib
}
