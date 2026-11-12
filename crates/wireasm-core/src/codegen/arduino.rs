//! Arduino sketch backend for ESP32 and ESP8266.
//!
//! These chips are not covered by a hand-written assembler backend, so the
//! graph is translated to an Arduino C++ sketch (`setup()` / `loop()`). It is
//! built with `arduino-cli` and the matching core. MCU nodes map to the
//! standard Arduino API: pins, delays, UART (Serial), ADC (analogRead) and raw
//! register access.

use super::*;
use crate::model::*;
use crate::validate::Diagnostic;
use std::collections::HashSet;

pub fn generate(g: &Graph, _t: Target, diags: &mut Vec<Diagnostic>) -> Result<String, String> {
    let mut cg = Cg {
        g,
        lines: Vec::new(),
        visited: HashSet::new(),
        _diags: diags,
    };
    cg.run()
}

struct Cg<'a> {
    g: &'a Graph,
    lines: Vec<String>,
    visited: HashSet<String>,
    _diags: &'a mut Vec<Diagnostic>,
}

impl<'a> Cg<'a> {
    fn run(&mut self) -> Result<String, String> {
        let entry = self
            .g
            .entry()
            .ok_or_else(|| "no Entry node".to_string())?
            .id
            .clone();

        for line in header(self.g).lines() {
            self.lines.push(line.to_string());
        }
        self.lines.push("#include <Arduino.h>".to_string());
        self.lines.push(String::new());

        for name in self.g.all_variable_names() {
            self.lines.push(format!(
                "static uint32_t {} = {};",
                var_name(&name),
                self.g.init_of(&name) as u32
            ));
        }
        if !self.g.all_variable_names().is_empty() {
            self.lines.push(String::new());
        }

        self.lines.push("void setup() {".to_string());
        if self.uses_serial() {
            let baud = self
                .g
                .nodes
                .iter()
                .find(|n| n.kind == "McuUartInit" && n.prop_u64("baud") > 0)
                .map(|n| n.prop_u64("baud"))
                .unwrap_or(9600);
            self.emit(1, &format!("Serial.begin({baud});"));
        }
        self.walk_next(&entry, 1)?;
        self.lines.push("}".to_string());
        self.lines.push(String::new());
        self.lines.push("void loop() {".to_string());
        self.lines
            .push("  delay(1);  // the graph runs in setup()".to_string());
        self.lines.push("}".to_string());

        let mut s = self.lines.join("\n");
        s.push('\n');
        Ok(s)
    }

    fn emit(&mut self, indent: usize, line: &str) {
        self.lines.push(format!("{}{}", "  ".repeat(indent), line));
    }

    fn walk_next(&mut self, id: &str, indent: usize) -> Result<(), String> {
        match self.g.exec_next(id) {
            Some(next) => self.walk(&next, indent),
            None => {
                self.emit(indent, "return;");
                Ok(())
            }
        }
    }

    fn walk(&mut self, id: &str, indent: usize) -> Result<(), String> {
        let node = match self.g.node(id) {
            Some(n) => n.clone(),
            None => return Err(format!("missing node {id}")),
        };
        if !self.visited.insert(id.to_string()) {
            self.emit(indent, &format!("goto {};", label(&node.id)));
            return Ok(());
        }
        self.lines.push(String::new());
        self.emit(indent.saturating_sub(1), &format!("{}:", label(&node.id)));
        self.emit(
            indent,
            &format!("// {} ({})", self.g.title_of(&node.kind), node.id),
        );

        match node.kind.as_str() {
            "Entry" | "Anchor" | "Comment" => {}
            "VarSet" => {
                let value = self.expr_input(&node, "value")?;
                self.emit(
                    indent,
                    &format!("{} = {};", var_name(node.prop("name").trim()), value),
                );
            }
            "PrintU64" => {
                let value = self.expr_input(&node, "value")?;
                self.emit(indent, &format!("Serial.println({value});"));
            }
            "PrintStr" | "McuUartWriteStr" => {
                self.emit(
                    indent,
                    &format!("Serial.print(\"{}\");", c_string(node.prop("text"))),
                );
            }
            "McuPinMode" => {
                let pin = node.prop_u64("pin");
                let mode = match node.prop("mode").trim() {
                    "out" => "OUTPUT",
                    "pullup" => "INPUT_PULLUP",
                    _ => "INPUT",
                };
                self.emit(indent, &format!("pinMode({pin}, {mode});"));
            }
            "McuDigitalWrite" => {
                let pin = node.prop_u64("pin");
                let value = self.expr_input(&node, "value")?;
                self.emit(
                    indent,
                    &format!("digitalWrite({pin}, ({value}) ? HIGH : LOW);"),
                );
            }
            "McuDigitalRead" => {
                let pin = node.prop_u64("pin");
                self.emit(indent, &format!("(void)digitalRead({pin});"));
            }
            "McuDelayMs" => {
                let value = self.expr_input(&node, "value")?;
                self.emit(indent, &format!("delay({value});"));
            }
            "McuDelayUs" => {
                let value = self.expr_input(&node, "value")?;
                self.emit(indent, &format!("delayMicroseconds({value});"));
            }
            "McuUartInit" => {
                let baud = node.prop_u64("baud").max(1);
                self.emit(indent, &format!("Serial.begin({baud});"));
            }
            "McuUartWrite" => {
                let value = self.expr_input(&node, "value")?;
                self.emit(indent, &format!("Serial.write((uint8_t)({value}));"));
            }
            "McuAdcRead" => {
                let ch = node.prop_u64("channel");
                self.emit(indent, &format!("(void)analogRead({ch});"));
            }
            "McuRegisterWrite" => {
                let addr = node.prop_u64("address") as u32;
                let value = self.expr_input(&node, "value")?;
                self.emit(
                    indent,
                    &format!("*(volatile uint32_t *)0x{addr:08X}u = (uint32_t)({value});"),
                );
            }
            "McuRegisterRead" => {
                let addr = node.prop_u64("address") as u32;
                self.emit(
                    indent,
                    &format!("(void)*(volatile uint32_t *)0x{addr:08X}u;"),
                );
            }
            "RawAsm" => {
                let text = node.prop("text");
                for line in text.lines() {
                    self.emit(indent, line);
                }
            }
            "Return" => {
                self.emit(indent, "return;");
                return Ok(());
            }
            "Branch" => {
                let cond = self.expr_input(&node, "cond")?;
                let t_target = self
                    .g
                    .outgoing(&node.id, "true")
                    .next()
                    .map(|e| e.to_node.clone());
                let f_target = self
                    .g
                    .outgoing(&node.id, "false")
                    .next()
                    .map(|e| e.to_node.clone());
                match (&t_target, &f_target) {
                    (Some(t), Some(f)) => self.emit(
                        indent,
                        &format!("if ({cond}) goto {}; else goto {};", label(t), label(f)),
                    ),
                    (Some(t), None) => {
                        self.emit(indent, &format!("if ({cond}) goto {};", label(t)));
                        self.emit(indent, "return;");
                    }
                    (None, Some(f)) => {
                        self.emit(indent, &format!("if (!({cond})) goto {};", label(f)));
                        self.emit(indent, "return;");
                    }
                    (None, None) => self.emit(indent, "return;"),
                }
                if let Some(t) = &t_target {
                    self.walk(t, indent)?;
                }
                if let Some(f) = &f_target {
                    self.walk(f, indent)?;
                }
                return Ok(());
            }
            "RegGet" | "RegSet" => {
                return Err(
                    "CPU registers are not addressable on ESP targets; use McuRegisterWrite/Read"
                        .to_string(),
                );
            }
            "Call" | "Syscall" => {
                return Err(format!(
                    "node {} ({}) is not available on ESP targets",
                    node.id, node.kind
                ));
            }
            other if self.g.custom(other).is_some() => {
                return Err(format!(
                    "custom node '{}' has no source template for ESP targets",
                    other
                ));
            }
            other => {
                return Err(format!("node {} has unsupported kind {other}", node.id));
            }
        }
        self.walk_next(id, indent)
    }

    // ------------------------------------------------------------ expressions

    fn expr_input(&self, node: &Node, pin: &str) -> Result<String, String> {
        match self.g.incoming(&node.id, pin) {
            Some(e) => self.expr(&e.from_node, 0),
            None => Ok("0u".to_string()),
        }
    }

    fn expr(&self, id: &str, depth: usize) -> Result<String, String> {
        if depth >= 60 {
            return Err(format!("expression too deep at {id}"));
        }
        let node = self
            .g
            .node(id)
            .ok_or_else(|| format!("missing data node {id}"))?;
        let bin = |pin: &str| -> Result<String, String> {
            match self.g.incoming(&node.id, pin) {
                Some(e) => self.expr(&e.from_node, depth + 1),
                None => Ok("0u".to_string()),
            }
        };
        Ok(match node.kind.as_str() {
            "Const" => format!("{}u", node.prop_u64("value") as u32),
            "VarGet" => var_name(node.prop("name").trim()),
            "BinOp" => {
                let a = bin("a")?;
                let b = bin("b")?;
                match node.prop("op") {
                    "add" => format!("({a} + {b})"),
                    "sub" => format!("({a} - {b})"),
                    "mul" => format!("((uint32_t)({a}) * (uint32_t)({b}))"),
                    "div" => format!("(({b}) ? (({a}) / ({b})) : 0u)"),
                    "and" => format!("({a} & {b})"),
                    "or" => format!("({a} | {b})"),
                    "xor" => format!("({a} ^ {b})"),
                    "shl" => format!("(({b}) < 32u ? (({a}) << ({b})) : 0u)"),
                    "shr" => format!("(({b}) < 32u ? (({a}) >> ({b})) : 0u)"),
                    op => return Err(format!("bad binop {op}")),
                }
            }
            "Cmp" => {
                let a = bin("a")?;
                let b = bin("b")?;
                let op = match node.prop("op") {
                    "eq" => "==",
                    "ne" => "!=",
                    "lt" => "<",
                    "le" => "<=",
                    "gt" => ">",
                    "ge" => ">=",
                    op => return Err(format!("bad cmp {op}")),
                };
                format!("(({a}) {op} ({b}) ? 1u : 0u)")
            }
            "McuDigitalRead" => format!("(digitalRead({}) ? 1u : 0u)", node.prop_u64("pin")),
            "McuAdcRead" => format!("(uint32_t)analogRead({})", node.prop_u64("channel")),
            "McuRegisterRead" => format!(
                "(*(volatile uint32_t *)0x{:08X}u)",
                node.prop_u64("address") as u32
            ),
            other if self.g.custom(other).is_some() => {
                return Err(format!(
                    "custom node '{}' has no source template for ESP targets",
                    other
                ));
            }
            other => return Err(format!("node {id} ({other}) has no ESP data output")),
        })
    }

    fn uses_serial(&self) -> bool {
        self.g.nodes.iter().any(|n| {
            matches!(
                n.kind.as_str(),
                "PrintU64" | "PrintStr" | "McuUartInit" | "McuUartWrite" | "McuUartWriteStr"
            )
        })
    }
}

fn label(id: &str) -> String {
    format!("L_{}", sanitize(id))
}

fn var_name(name: &str) -> String {
    format!("V_{}_{:08X}", sanitize(name), fnv1a(name))
}

fn fnv1a(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn c_string(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\x{:02X}", c as u32)),
            c => out.push(c),
        }
    }
    out
}
