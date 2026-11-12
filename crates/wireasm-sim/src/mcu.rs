//! Virtual microcontroller: runs the WireASM graph itself with circuit time.
//!
//! Proteus loads a `.hex` into a chip model. WireASM already owns the program
//! as a graph, so for 0.4.0 the first co-simulation target runs the graph
//! directly: pin writes become Thevenin sources in the analog solver, delays
//! schedule the next event, UART output goes to a terminal, and `McuAdcRead`
//! samples the node connected to the pin. Executing a real `.elf`/`.hex` is a
//! separate backend and can share this interface later.

use std::collections::BTreeMap;

use wireasm_core::json::Json;
use wireasm_core::model::{Graph, Node, Target};

#[derive(Clone, Debug)]
pub struct VirtualMcu {
    graph: Graph,
    target: Target,
    pc: String,
    vars: BTreeMap<String, u64>,
    regs: BTreeMap<String, u64>,
    pins: BTreeMap<String, bool>,
    inputs: BTreeMap<String, bool>,
    analog: BTreeMap<String, f64>,
    uart: String,
    next_event: Option<f64>,
    now: f64,
    finished: bool,
    steps: u64,
    max_steps: u64,
}

#[derive(Clone, Debug)]
enum Flow {
    Next(Option<String>),
    Delay(f64),
    Done,
}

impl VirtualMcu {
    pub fn new(graph: Graph) -> Result<VirtualMcu, String> {
        let target = Target::parse(&graph.target)
            .ok_or_else(|| format!("unknown target `{}`", graph.target))?;
        if !target.is_mcu() {
            return Err(format!(
                "{} is not an MCU target; co-simulation needs avr/stm32/esp",
                target.label()
            ));
        }
        let entry = graph
            .entry()
            .ok_or_else(|| "graph has no Entry node".to_string())?
            .id
            .clone();
        let mut vars = BTreeMap::new();
        for v in &graph.variables {
            vars.insert(v.name.clone(), v.init);
        }
        Ok(VirtualMcu {
            graph,
            target,
            pc: entry,
            vars,
            regs: BTreeMap::new(),
            pins: BTreeMap::new(),
            inputs: BTreeMap::new(),
            analog: BTreeMap::new(),
            uart: String::new(),
            next_event: Some(0.0),
            now: 0.0,
            finished: false,
            steps: 0,
            max_steps: 5_000_000,
        })
    }

    pub fn from_json(src: &str) -> Result<VirtualMcu, String> {
        let v = Json::parse(src)?;
        let g = Graph::from_json_value(&v)?;
        VirtualMcu::new(g)
    }

    pub fn target(&self) -> Target {
        self.target
    }

    pub fn graph(&self) -> &Graph {
        &self.graph
    }

    /// Time of the next scheduled delay, in seconds.
    pub fn next_event(&self) -> Option<f64> {
        self.next_event
    }

    pub fn is_finished(&self) -> bool {
        self.finished
    }

    pub fn pins(&self) -> &BTreeMap<String, bool> {
        &self.pins
    }

    pub fn pin_high(&self, pin: &str, _vdd: f64) -> bool {
        self.pins.get(pin).copied().unwrap_or(false)
    }

    pub fn pin_voltage(&self, pin: &str, vdd: f64) -> f64 {
        if self.pin_high(pin, vdd) {
            vdd
        } else {
            0.0
        }
    }

    pub fn uart(&self) -> &str {
        &self.uart
    }

    pub fn take_uart(&mut self) -> String {
        std::mem::take(&mut self.uart)
    }

    pub fn set_analog(&mut self, pin: &str, volts: f64) {
        self.analog.insert(pin.to_string(), volts);
    }

    pub fn set_input(&mut self, pin: &str, high: bool) {
        self.inputs.insert(pin.to_string(), high);
    }

    pub fn vars(&self) -> &BTreeMap<String, u64> {
        &self.vars
    }

    /// Run the program until it hits a non-zero delay or finishes. A call at
    /// `t` earlier than the next event does nothing.
    pub fn run_until(&mut self, t: f64) -> Result<(), String> {
        if self.finished {
            return Ok(());
        }
        if self.next_event.is_some_and(|e| e > t + 1e-12) {
            return Ok(());
        }
        self.now = t.max(self.now);
        loop {
            if self.finished {
                self.next_event = None;
                return Ok(());
            }
            if self.pc.is_empty() {
                self.finished = true;
                self.next_event = None;
                return Ok(());
            }
            self.steps += 1;
            if self.steps > self.max_steps {
                return Err(format!(
                    "virtual MCU ran {} nodes without a delay; check the loop",
                    self.steps
                ));
            }
            let id = self.pc.clone();
            let node = self
                .graph
                .node(&id)
                .ok_or_else(|| format!("MCU jumped to unknown node `{id}`"))?
                .clone();
            match self.exec(&node)? {
                Flow::Next(next) => {
                    self.pc = next.unwrap_or_default();
                }
                Flow::Delay(seconds) => {
                    // The delay node is done; the next run resumes at the node
                    // its `exec` output points to, not by re-entering the delay.
                    self.pc = self.graph.exec_next(&node.id).unwrap_or_default();
                    if seconds <= 0.0 {
                        continue;
                    }
                    self.next_event = Some(self.now + seconds);
                    return Ok(());
                }
                Flow::Done => {
                    self.finished = true;
                    self.next_event = None;
                    return Ok(());
                }
            }
        }
    }

    fn exec(&mut self, node: &Node) -> Result<Flow, String> {
        match node.kind.as_str() {
            "Entry" | "Anchor" | "Comment" => Ok(self.flow_next(node)),
            "VarSet" => {
                let value = self.input_u64(node, "value")?;
                let name = node.prop("name").trim().to_string();
                self.vars.insert(name, value);
                Ok(self.flow_next(node))
            }
            "Branch" => {
                let cond = self.input_u64(node, "cond")? != 0;
                let pin = if cond { "true" } else { "false" };
                Ok(Flow::Next(
                    self.graph
                        .outgoing(&node.id, pin)
                        .next()
                        .map(|e| e.to_node.clone()),
                ))
            }
            "McuPinMode" => {
                let pin = self.pin_key(node);
                self.pins.entry(pin).or_insert(false);
                Ok(self.flow_next(node))
            }
            "McuDigitalWrite" => {
                let value = self.input_u64(node, "value")? != 0;
                let pin = self.pin_key(node);
                self.pins.insert(pin, value);
                Ok(self.flow_next(node))
            }
            "McuDigitalRead" => Ok(self.flow_next(node)),
            "McuDelayMs" => {
                let ms = self.input_u64(node, "value")?;
                Ok(Flow::Delay(ms as f64 / 1000.0))
            }
            "McuDelayUs" => {
                let us = self.input_u64(node, "value")?;
                Ok(Flow::Delay(us as f64 / 1_000_000.0))
            }
            "McuUartInit" => Ok(self.flow_next(node)),
            "McuUartWrite" => {
                let byte = self.input_u64(node, "value")? as u8;
                self.uart.push(byte as char);
                Ok(self.flow_next(node))
            }
            "McuUartWriteStr" => {
                self.uart.push_str(&unescape(node.prop("text")));
                Ok(self.flow_next(node))
            }
            "PrintU64" => {
                let value = self.input_u64(node, "value")?;
                self.uart.push_str(&value.to_string());
                Ok(self.flow_next(node))
            }
            "PrintStr" => {
                self.uart.push_str(&unescape(node.prop("text")));
                Ok(self.flow_next(node))
            }
            "McuAdcRead" | "McuRegisterWrite" | "RegSet" => Ok(self.flow_next(node)),
            "Return" => Ok(Flow::Done),
            other => Err(format!(
                "virtual MCU does not support node `{other}` (id `{}`)",
                node.id
            )),
        }
    }

    /// Follow the node's `exec` output.
    fn flow_next(&self, node: &Node) -> Flow {
        Flow::Next(self.graph.exec_next(&node.id))
    }

    fn input_u64(&mut self, node: &Node, pin: &str) -> Result<u64, String> {
        self.input_at(&node.id, pin, 0)
    }

    /// Follow `node_id.pin` back to the value it carries. The edge is cloned
    /// so recursive evaluation can borrow `self` mutably.
    fn input_at(&mut self, node_id: &str, pin: &str, depth: usize) -> Result<u64, String> {
        let from = self
            .graph
            .incoming(node_id, pin)
            .map(|e| e.from_node.clone())
            .ok_or_else(|| format!("`{node_id}` is missing input `{pin}`"))?;
        self.eval(&from, depth + 1)
    }

    fn eval(&mut self, id: &str, depth: usize) -> Result<u64, String> {
        if depth > 64 {
            return Err(format!("expression too deep at `{id}`"));
        }
        let node = self
            .graph
            .node(id)
            .ok_or_else(|| format!("expression refers to unknown node `{id}`"))?
            .clone();
        match node.kind.as_str() {
            "Const" => Ok(node.prop_u64("value")),
            "VarGet" => Ok(self
                .vars
                .get(node.prop("name").trim())
                .copied()
                .unwrap_or(0)),
            "VarSet" | "McuRegisterRead" => self.input_at(&node.id, "value", depth).or(Ok(0)),
            "RegGet" => Ok(self.regs.get(node.prop("reg").trim()).copied().unwrap_or(0)),
            "BinOp" => {
                let a = self.input_at(&node.id, "a", depth)?;
                let b = self.input_at(&node.id, "b", depth)?;
                Ok(match node.prop("op") {
                    "add" => a.wrapping_add(b),
                    "sub" => a.wrapping_sub(b),
                    "mul" => a.wrapping_mul(b),
                    "div" => a.checked_div(b).unwrap_or(0),
                    "and" => a & b,
                    "or" => a | b,
                    "xor" => a ^ b,
                    "shl" => a.checked_shl(b as u32).unwrap_or(0),
                    "shr" => a.checked_shr(b as u32).unwrap_or(0),
                    _ => return Err(format!("unknown BinOp `{}`", node.prop("op"))),
                })
            }
            "Cmp" => {
                let a = self.input_at(&node.id, "a", depth)?;
                let b = self.input_at(&node.id, "b", depth)?;
                let r = match node.prop("op") {
                    "eq" => a == b,
                    "ne" => a != b,
                    "lt" => a < b,
                    "le" => a <= b,
                    "gt" => a > b,
                    "ge" => a >= b,
                    _ => return Err(format!("unknown Cmp `{}`", node.prop("op"))),
                };
                Ok(if r { 1 } else { 0 })
            }
            "McuDigitalRead" => {
                let pin = self.pin_key(&node);
                let v = self
                    .inputs
                    .get(&pin)
                    .or_else(|| self.pins.get(&pin))
                    .copied()
                    .unwrap_or(false);
                Ok(if v { 1 } else { 0 })
            }
            "McuAdcRead" => {
                let pin = if self.target.is_stm32() {
                    format!("A{}", node.prop_u64("channel"))
                } else {
                    node.prop("pin").to_string()
                };
                let volts = self.analog.get(&pin).copied().unwrap_or(0.0);
                let full = if self.target.is_avr() { 1023.0 } else { 4095.0 };
                Ok(((volts / 5.0) * full).clamp(0.0, full) as u64)
            }
            "McuRegisterWrite" | "RegSet" => {
                let value = self.input_at(&node.id, "value", depth)?;
                self.regs.insert(node.prop("reg").trim().to_string(), value);
                Ok(value)
            }
            _ => Ok(0),
        }
    }

    /// STM32 names a pin by port and number (`C13`); AVR and ESP use the
    /// Arduino number (`13`).
    fn pin_key(&self, node: &Node) -> String {
        if self.target.is_stm32() {
            let port = if node.prop("port").is_empty() {
                "A"
            } else {
                node.prop("port")
            };
            format!("{}{}", port.to_ascii_uppercase(), node.prop("pin"))
        } else {
            node.prop("pin").to_string()
        }
    }
}

fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('\\') => out.push('\\'),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use wireasm_core::model::{Edge, Node};

    fn blink(target: &str, port: &str, pin: &str) -> Graph {
        let mut g = Graph {
            name: "blink".into(),
            target: target.into(),
            ..Graph::default()
        };
        let n = |id: &str, kind: &str| Node {
            id: id.into(),
            kind: kind.into(),
            x: 0.0,
            y: 0.0,
            props: Default::default(),
        };
        let entry = n("e", "Entry");
        let mut mode = n("m", "McuPinMode");
        mode.props.insert("pin".into(), pin.into());
        mode.props.insert("port".into(), port.into());
        mode.props.insert("mode".into(), "out".into());
        let mut hi = n("h", "McuDigitalWrite");
        hi.props.insert("pin".into(), pin.into());
        hi.props.insert("port".into(), port.into());
        let mut one = n("one", "Const");
        one.props.insert("value".into(), "1".into());
        let d1 = n("d1", "McuDelayMs");
        let mut ms = n("ms", "Const");
        ms.props.insert("value".into(), "500".into());
        let mut lo = n("l", "McuDigitalWrite");
        lo.props.insert("pin".into(), pin.into());
        lo.props.insert("port".into(), port.into());
        let mut zero = n("zero", "Const");
        zero.props.insert("value".into(), "0".into());
        let d2 = n("d2", "McuDelayMs");
        let anchor = n("loop", "Anchor");
        g.nodes = vec![entry, mode, hi, one, d1, ms, lo, zero, d2, anchor];
        let e = |a: &str, ap: &str, b: &str, bp: &str| Edge {
            from_node: a.into(),
            from_pin: ap.into(),
            to_node: b.into(),
            to_pin: bp.into(),
        };
        g.edges = vec![
            e("e", "exec", "m", "exec"),
            e("m", "exec", "h", "exec"),
            e("one", "value", "h", "value"),
            e("h", "exec", "d1", "exec"),
            e("ms", "value", "d1", "value"),
            e("d1", "exec", "l", "exec"),
            e("zero", "value", "l", "value"),
            e("l", "exec", "d2", "exec"),
            e("ms", "value", "d2", "value"),
            e("d2", "exec", "loop", "exec"),
            e("loop", "exec", "h", "exec"),
        ];
        g
    }

    #[test]
    fn avr_blink_toggles_pin() {
        let mut mcu = VirtualMcu::new(blink("avr-atmega328p", "", "13")).unwrap();
        assert!(mcu.next_event() == Some(0.0));
        mcu.run_until(0.0).unwrap();
        assert!(mcu.pin_high("13", 5.0));
        assert_eq!(mcu.next_event(), Some(0.5));
        mcu.run_until(0.5).unwrap();
        assert!(!mcu.pin_high("13", 5.0));
        assert_eq!(mcu.next_event(), Some(1.0));
    }

    #[test]
    fn stm32_uses_port_names() {
        let mut mcu = VirtualMcu::new(blink("stm32f411", "C", "13")).unwrap();
        mcu.run_until(0.0).unwrap();
        assert!(mcu.pin_high("C13", 3.3));
        assert_eq!(mcu.pin_voltage("C13", 3.3), 3.3);
    }

    #[test]
    fn uart_and_unescape() {
        let mut g = blink("esp32", "", "2");
        g.nodes.push(Node {
            id: "msg".into(),
            kind: "McuUartWriteStr".into(),
            x: 0.0,
            y: 0.0,
            props: [("text".to_string(), "hi\\n".to_string())].into(),
        });
        let mut mcu = VirtualMcu::new(g).unwrap();
        let msg = mcu.graph.node("msg").unwrap().clone();
        mcu.exec(&msg).unwrap();
        assert_eq!(mcu.uart(), "hi\n");
    }
}
