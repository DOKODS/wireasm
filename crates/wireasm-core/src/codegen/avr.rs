//! AVR backend: ATmega328P (Arduino Uno / Nano) at 16 MHz.
//!
//! The node language works with u64 values, but the AVR port keeps them in a
//! 16-bit accumulator (`r24:r25`, the avr-gcc return-value pair). Values above
//! 0xFFFF are truncated; the validator warns about it. All variables and
//! temporaries live in SRAM, so expression evaluation cannot clobber them.

use super::*;
use crate::model::*;
use crate::validate::Diagnostic;
use std::collections::HashSet;

pub fn generate(g: &Graph, _t: Target, diags: &mut Vec<Diagnostic>) -> Result<String, String> {
    let mut cg = Cg {
        g,
        strings: collect_strings(g).1,
        em: Emitter::with_comment(";"),
        visited: HashSet::new(),
        _diags: diags,
    };
    cg.run()
}

struct Cg<'a> {
    g: &'a Graph,
    strings: std::collections::BTreeMap<String, String>,
    em: Emitter,
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

        self.em.push(header(self.g));
        self.em.blank();
        self.em.push(".text");
        self.em.push(".global main");
        self.em
            .comment("crt0 calls main after clearing .bss and setting the stack pointer");
        self.em.label("main");
        self.em.label(&sanitize(&entry));
        self.em.comment(format!("Entry — {}", self.g.name));

        if self.uses_uart() {
            let baud = self
                .g
                .nodes
                .iter()
                .find(|n| n.kind == "McuUartInit")
                .map(|n| n.prop_u64("baud"))
                .filter(|b| *b > 0)
                .unwrap_or(9600);
            self.uart_init(baud);
        }

        for name in self.g.all_variable_names() {
            let init = self.g.init_of(&name) & 0xffff;
            if init == 0 {
                continue; // .bss is zeroed by crt0
            }
            self.em.comment(format!("variable {name} = {init}"));
            self.ldi16("r24", init);
            let label = var_label(&name);
            self.em.ins(format!("sts {label}, r24"));
            self.em.ins(format!("sts {label}+1, r25"));
        }

        self.walk_next(&entry)?;

        self.em.blank();
        self.em.label("Ldefault_ret");
        self.em
            .comment("implicit return: crt0 disables interrupts and halts");
        self.em.ins("ret");

        self.emit_runtime();
        self.emit_data();
        self.emit_bss();

        let mut s = self.em.lines.join("\n");
        s.push('\n');
        Ok(s)
    }

    fn walk_next(&mut self, id: &str) -> Result<(), String> {
        match self.g.exec_next(id) {
            Some(next) => self.walk(&next),
            None => {
                self.em.ins("jmp Ldefault_ret");
                Ok(())
            }
        }
    }

    fn walk(&mut self, id: &str) -> Result<(), String> {
        let node = match self.g.node(id) {
            Some(n) => n.clone(),
            None => return Err(format!("missing node {id}")),
        };
        let label = sanitize(id);
        if !self.visited.insert(id.to_string()) {
            self.em.ins(format!("jmp {label}"));
            return Ok(());
        }
        self.em.blank();
        self.em.label(&label);
        self.em.comment(format!(
            "{} — {}",
            self.g.title_of(&node.kind),
            node.prop("name")
        ));
        self.em.comment(format!("node {}", node.id));

        match node.kind.as_str() {
            "Entry" | "Anchor" | "Comment" => {}
            "VarSet" => {
                self.eval_input(&node, "value", 0)?;
                let name = node.prop("name").trim().to_string();
                let label = var_label(&name);
                self.em.ins(format!("sts {label}, r24"));
                self.em.ins(format!("sts {label}+1, r25"));
            }
            "RegSet" => {
                self.eval_input(&node, "value", 0)?;
                let reg = avr_reg(node.prop("reg"));
                self.em.ins(format!("mov {reg}, r24"));
            }
            "PrintU64" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("call uart_print_u16");
            }
            "PrintStr" | "McuUartWriteStr" => {
                let label = self
                    .strings
                    .get(&node.id)
                    .cloned()
                    .unwrap_or_else(|| "Lstr_missing".to_string());
                self.em.ins(format!("ldi r30, lo8({label})"));
                self.em.ins(format!("ldi r31, hi8({label})"));
                self.em.ins("call uart_write_str");
            }
            "McuPinMode" => {
                let pin = node.prop_u64("pin");
                let map = pin_map(pin).ok_or_else(|| format!("invalid pin {pin}"))?;
                let mode = node.prop("mode").trim();
                match mode {
                    "out" => {
                        self.em
                            .ins(format!("sbi 0x{:02X}, {}", map.ddr - 0x20, map.bit));
                    }
                    "pullup" => {
                        self.em
                            .ins(format!("cbi 0x{:02X}, {}", map.ddr - 0x20, map.bit));
                        self.em
                            .ins(format!("sbi 0x{:02X}, {}", map.port - 0x20, map.bit));
                    }
                    _ => {
                        self.em
                            .ins(format!("cbi 0x{:02X}, {}", map.ddr - 0x20, map.bit));
                        self.em
                            .ins(format!("cbi 0x{:02X}, {}", map.port - 0x20, map.bit));
                    }
                }
            }
            "McuDigitalWrite" => {
                let pin = node.prop_u64("pin");
                let map = pin_map(pin).ok_or_else(|| format!("invalid pin {pin}"))?;
                self.eval_input(&node, "value", 0)?;
                self.em.ins("mov r18, r24");
                self.em.ins("or r18, r25");
                self.em.ins("breq 1f");
                self.em
                    .ins(format!("sbi 0x{:02X}, {}", map.port - 0x20, map.bit));
                self.em.ins("jmp 2f");
                self.em.label("1");
                self.em
                    .ins(format!("cbi 0x{:02X}, {}", map.port - 0x20, map.bit));
                self.em.label("2");
            }
            "McuDigitalRead" => {
                let pin = node.prop_u64("pin");
                let map = pin_map(pin).ok_or_else(|| format!("invalid pin {pin}"))?;
                self.em.ins(format!("lds r24, 0x{:02X}", map.pin));
                self.em.ins(format!("andi r24, 0x{:02X}", 1u8 << map.bit));
                self.em.ins("breq 1f");
                self.em.ins("ldi r24, 1");
                self.em.label("1");
                self.em.ins("clr r25");
            }
            "McuDelayMs" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("call delay_ms");
            }
            "McuDelayUs" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("call delay_us");
            }
            "McuUartInit" => {
                let baud = node.prop_u64("baud").max(1);
                self.uart_init(baud);
            }
            "McuUartWrite" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("call uart_write_byte");
            }
            "McuAdcRead" => {
                let ch = node.prop_u64("channel").min(7) as u8;
                self.em
                    .comment(format!("ADC channel {ch}, AVcc reference, /128 prescaler"));
                self.em.ins(format!("ldi r24, 0x{:02X}", 0x40 | ch));
                self.em.ins("sts 0x7C, r24");
                self.em.ins("ldi r24, 0xC7");
                self.em.ins("sts 0x7A, r24");
                self.em.label("1");
                self.em.ins("lds r24, 0x7A");
                self.em.ins("sbrc r24, 6");
                self.em.ins("rjmp 1b");
                self.em.ins("lds r24, 0x78");
                self.em.ins("lds r25, 0x79");
            }
            "McuRegisterWrite" => {
                let addr = node.prop_u64("address");
                self.eval_input(&node, "value", 0)?;
                self.em.ins(format!("sts {}, r24", addr as u16));
            }
            "McuRegisterRead" => {
                let addr = node.prop_u64("address");
                self.em.ins(format!("lds r24, {}", addr as u16));
                self.em.ins("clr r25");
            }
            "RawAsm" => {
                self.eval_input(&node, "a", 0)?;
                self.em.ins("sts Warg0, r24");
                self.em.ins("sts Warg0+1, r25");
                self.eval_input(&node, "b", 0)?;
                self.em.ins("movw r22, r24");
                self.em.ins("lds r24, Warg0");
                self.em.ins("lds r25, Warg0+1");
                let text = node
                    .prop("text")
                    .replace("{a}", "r24")
                    .replace("{b}", "r22")
                    .replace("{out}", "r24")
                    .replace("//", ";");
                self.em.comment("raw assembly begin");
                emit_snippet(&mut self.em, &text);
                self.em.comment("raw assembly end");
            }
            "Return" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("ret");
                return Ok(());
            }
            "Branch" => {
                self.eval_input(&node, "cond", 0)?;
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
                self.em.comment("Branch");
                self.em.ins("mov r18, r24");
                self.em.ins("or r18, r25");
                self.em.ins("breq 1f");
                match &t_target {
                    Some(t) => self.em.ins(format!("jmp {}", sanitize(t))),
                    None => self.em.ins("jmp Ldefault_ret"),
                }
                self.em.label("1");
                match &f_target {
                    Some(f) => self.em.ins(format!("jmp {}", sanitize(f))),
                    None => self.em.ins("jmp Ldefault_ret"),
                }
                if let Some(t) = &t_target {
                    self.walk(t)?;
                }
                if let Some(f) = &f_target {
                    self.walk(f)?;
                }
                return Ok(());
            }
            "Call" | "Syscall" => {
                return Err(format!(
                    "node {} ({}) is not available on the AVR target",
                    node.id, node.kind
                ));
            }
            other if self.g.custom(other).is_some() => {
                return Err(format!(
                    "custom node '{}' cannot be compiled for AVR (no AVR template)",
                    other
                ));
            }
            other => {
                return Err(format!("node {} has unsupported kind {other}", node.id));
            }
        }
        self.walk_next(id)
    }

    // ------------------------------------------------------------ expressions

    fn eval_input(&mut self, node: &Node, pin: &str, depth: usize) -> Result<(), String> {
        match self.g.incoming(&node.id, pin) {
            Some(e) => {
                let src = e.from_node.clone();
                self.eval(&src, depth)
            }
            None => {
                self.ldi16("r24", 0);
                Ok(())
            }
        }
    }

    fn eval(&mut self, id: &str, depth: usize) -> Result<(), String> {
        if depth >= 60 {
            return Err(format!("expression too deep at {id}"));
        }
        let node = self
            .g
            .node(id)
            .ok_or_else(|| format!("missing data node {id}"))?
            .clone();
        match node.kind.as_str() {
            "Const" => {
                let v = node.prop_u64("value") & 0xffff;
                self.ldi16("r24", v);
            }
            "VarGet" => {
                let name = node.prop("name").trim().to_string();
                let label = var_label(&name);
                self.em.ins(format!("lds r24, {label}"));
                self.em.ins(format!("lds r25, {label}+1"));
            }
            "BinOp" => {
                let a = self.input_of(&node, "a")?;
                self.eval(&a, depth)?;
                let tmp = format!("Wtmp{depth}");
                self.em.ins(format!("sts {tmp}, r24"));
                self.em.ins(format!("sts {tmp}+1, r25"));
                let b = self.input_of(&node, "b")?;
                self.eval(&b, depth + 1)?;
                self.em.ins("movw r22, r24");
                self.em.ins(format!("lds r24, {tmp}"));
                self.em.ins(format!("lds r25, {tmp}+1"));
                let op = node.prop("op");
                match op {
                    "add" => self.add16(),
                    "sub" => self.sub16(),
                    "and" => {
                        self.em.ins("and r24, r22");
                        self.em.ins("and r25, r23");
                    }
                    "or" => {
                        self.em.ins("or r24, r22");
                        self.em.ins("or r25, r23");
                    }
                    "xor" => {
                        self.em.ins("eor r24, r22");
                        self.em.ins("eor r25, r23");
                    }
                    "mul" => {
                        self.em.ins("mul r24, r22");
                        self.em.ins("mov r18, r0");
                        self.em.ins("mov r19, r1");
                        self.em.ins("mul r24, r23");
                        self.em.ins("add r19, r0");
                        self.em.ins("mul r25, r22");
                        self.em.ins("add r19, r0");
                        self.em.ins("clr r1");
                        self.em.ins("mov r24, r18");
                        self.em.ins("mov r25, r19");
                    }
                    "div" => self.em.ins("call udiv16"),
                    "shl" => {
                        self.em.ins("cpi r22, 16");
                        self.em.ins("brlo 1f");
                        self.ldi16("r24", 0);
                        self.em.ins("jmp 2f");
                        self.em.label("1");
                        self.em.ins("tst r22");
                        self.em.ins("breq 3f");
                        self.em.label("4");
                        self.em.ins("lsl r24");
                        self.em.ins("rol r25");
                        self.em.ins("dec r22");
                        self.em.ins("brne 4b");
                        self.em.label("3");
                        self.em.label("2");
                    }
                    "shr" => {
                        self.em.ins("cpi r22, 16");
                        self.em.ins("brlo 1f");
                        self.ldi16("r24", 0);
                        self.em.ins("jmp 2f");
                        self.em.label("1");
                        self.em.ins("tst r22");
                        self.em.ins("breq 3f");
                        self.em.label("4");
                        self.em.ins("lsr r25");
                        self.em.ins("ror r24");
                        self.em.ins("dec r22");
                        self.em.ins("brne 4b");
                        self.em.label("3");
                        self.em.label("2");
                    }
                    _ => return Err(format!("bad binop {op}")),
                }
            }
            "Cmp" => {
                let a = self.input_of(&node, "a")?;
                self.eval(&a, depth)?;
                let tmp = format!("Wtmp{depth}");
                self.em.ins(format!("sts {tmp}, r24"));
                self.em.ins(format!("sts {tmp}+1, r25"));
                let b = self.input_of(&node, "b")?;
                self.eval(&b, depth + 1)?;
                self.em.ins("movw r22, r24");
                self.em.ins(format!("lds r24, {tmp}"));
                self.em.ins(format!("lds r25, {tmp}+1"));
                self.em.ins("cp r24, r22");
                self.em.ins("cpc r25, r23");
                self.ldi16("r24", 0);
                match node.prop("op") {
                    "eq" => {
                        self.em.ins("brne 1f");
                        self.em.ins("ldi r24, 1");
                        self.em.label("1");
                    }
                    "ne" => {
                        self.em.ins("breq 1f");
                        self.em.ins("ldi r24, 1");
                        self.em.label("1");
                    }
                    "lt" => {
                        self.em.ins("brsh 1f");
                        self.em.ins("ldi r24, 1");
                        self.em.label("1");
                    }
                    "le" => {
                        self.em.ins("brhi 1f");
                        self.em.ins("ldi r24, 1");
                        self.em.label("1");
                    }
                    "gt" => {
                        self.em.ins("brlo 1f");
                        self.em.ins("breq 1f");
                        self.em.ins("ldi r24, 1");
                        self.em.label("1");
                    }
                    "ge" => {
                        self.em.ins("brlo 1f");
                        self.em.ins("ldi r24, 1");
                        self.em.label("1");
                    }
                    _ => return Err("bad cmp".to_string()),
                }
            }
            "RegGet" => {
                let reg = avr_reg(node.prop("reg"));
                self.em.ins(format!("mov r18, {reg}"));
                self.em.ins("mov r24, r18");
                self.em.ins("clr r25");
            }
            "McuDigitalRead" => {
                let pin = node.prop_u64("pin");
                let map = pin_map(pin).ok_or_else(|| format!("invalid pin {pin}"))?;
                self.em.ins(format!("lds r24, 0x{:02X}", map.pin));
                self.em.ins(format!("andi r24, 0x{:02X}", 1u8 << map.bit));
                self.em.ins("breq 1f");
                self.em.ins("ldi r24, 1");
                self.em.label("1");
                self.em.ins("clr r25");
            }
            "McuAdcRead" => {
                let ch = node.prop_u64("channel").min(7) as u8;
                self.em.ins(format!("ldi r24, 0x{:02X}", 0x40 | ch));
                self.em.ins("sts 0x7C, r24");
                self.em.ins("ldi r24, 0xC7");
                self.em.ins("sts 0x7A, r24");
                self.em.label("1");
                self.em.ins("lds r24, 0x7A");
                self.em.ins("sbrc r24, 6");
                self.em.ins("rjmp 1b");
                self.em.ins("lds r24, 0x78");
                self.em.ins("lds r25, 0x79");
            }
            "McuRegisterRead" => {
                let addr = node.prop_u64("address");
                self.em.ins(format!("lds r24, {}", addr as u16));
                self.em.ins("clr r25");
            }
            other if self.g.custom(other).is_some() => {
                return Err(format!(
                    "custom node '{}' cannot be compiled for AVR (no AVR template)",
                    other
                ));
            }
            other => return Err(format!("node {id} ({other}) has no AVR data output")),
        }
        Ok(())
    }

    fn input_of(&self, node: &Node, pin: &str) -> Result<String, String> {
        self.g
            .incoming(&node.id, pin)
            .map(|e| e.from_node.clone())
            .ok_or_else(|| format!("input {pin} of {} is not connected", node.id))
    }

    // --------------------------------------------------------------- helpers

    fn ldi16(&mut self, pair: &str, v: u64) {
        let lo = (v & 0xff) as u8;
        let hi = ((v >> 8) & 0xff) as u8;
        self.em.ins(format!("ldi {pair}, {lo}"));
        let hi_reg = match pair {
            "r24" => "r25",
            "r22" => "r23",
            other => other,
        };
        self.em.ins(format!("ldi {hi_reg}, {hi}"));
    }

    fn add16(&mut self) {
        self.em.ins("add r24, r22");
        self.em.ins("adc r25, r23");
    }

    fn sub16(&mut self) {
        self.em.ins("sub r24, r22");
        self.em.ins("sbc r25, r23");
    }

    fn uses_uart(&self) -> bool {
        self.g.nodes.iter().any(|n| {
            matches!(
                n.kind.as_str(),
                "PrintU64" | "PrintStr" | "McuUartInit" | "McuUartWrite" | "McuUartWriteStr"
            )
        })
    }

    fn uart_init(&mut self, baud: u64) {
        let baud = baud.max(1);
        let ubrr = (16_000_000u64 / (16 * baud)).saturating_sub(1).min(4095);
        self.em.comment(format!(
            "UART0: {} baud, 8N1, UBRR = {ubrr} (16 MHz, normal speed)",
            baud
        ));
        self.ldi16("r24", ubrr);
        self.em.ins("sts 0xC4, r24"); // UBRR0L
        self.em.ins("sts 0xC5, r25"); // UBRR0H
        self.em.ins("ldi r24, 0x06"); // 8 data bits, no parity, 1 stop bit
        self.em.ins("sts 0xC2, r24"); // UCSR0C
        self.em.ins("ldi r24, 0x18"); // RXEN0 | TXEN0
        self.em.ins("sts 0xC1, r24"); // UCSR0B
    }

    fn emit_runtime(&mut self) {
        let number = self.g.nodes.iter().any(|n| n.kind == "PrintU64");
        let text = self
            .g
            .nodes
            .iter()
            .any(|n| n.kind == "PrintStr" || n.kind == "McuUartWriteStr");
        let byte = number || text || self.g.nodes.iter().any(|n| n.kind == "McuUartWrite");
        let delay_ms = self.g.nodes.iter().any(|n| n.kind == "McuDelayMs");
        let delay_us = self.g.nodes.iter().any(|n| n.kind == "McuDelayUs");
        let divide = self
            .g
            .nodes
            .iter()
            .any(|n| n.kind == "BinOp" && n.prop("op") == "div");

        if byte {
            self.em.blank();
            self.em
                .comment("UART: send the byte in r24, wait for UDRE0");
            self.em.label("uart_write_byte");
            self.em.label("1");
            self.em.ins("lds r23, 0xC0"); // UCSR0A
            self.em.ins("sbrs r23, 5"); // UDRE0
            self.em.ins("rjmp 1b");
            self.em.ins("sts 0xC6, r24"); // UDR0
            self.em.ins("ret");
        }
        if text {
            self.em.blank();
            self.em
                .comment("UART: send a zero-terminated string from data space (Z)");
            self.em.label("uart_write_str");
            self.em.label("1");
            self.em.ins("ld r24, Z+");
            self.em.ins("tst r24");
            self.em.ins("breq 2f");
            self.em.ins("call uart_write_byte");
            self.em.ins("rjmp 1b");
            self.em.label("2");
            self.em.ins("ret");
        }
        if number {
            self.em.blank();
            self.em
                .comment("UART: print the 16-bit value in r24:r25 as decimal + newline");
            self.em.label("uart_print_u16");
            self.em.ins("mov r16, r24");
            self.em.ins("mov r17, r25");
            self.em.ins("clr r21"); // started flag
            for div in [10000u64, 1000, 100, 10, 1] {
                self.em.ins(format!("ldi r18, lo8({div})"));
                self.em.ins(format!("ldi r19, hi8({div})"));
                self.em.ins("call pu_digit");
            }
            self.em.ins("tst r21");
            self.em.ins("brne 1f");
            self.em.ins("ldi r24, 48");
            self.em.ins("call uart_write_byte");
            self.em.label("1");
            self.em.ins("ldi r24, 10");
            self.em.ins("call uart_write_byte");
            self.em.ins("ret");
            self.em.blank();
            self.em
                .comment("one decimal digit: r16:r17 value, r18:r19 divisor");
            self.em.label("pu_digit");
            self.em.ins("clr r20");
            self.em.label("1");
            self.em.ins("cp r16, r18");
            self.em.ins("cpc r17, r19");
            self.em.ins("brlo 2f");
            self.em.ins("sub r16, r18");
            self.em.ins("sbc r17, r19");
            self.em.ins("inc r20");
            self.em.ins("rjmp 1b");
            self.em.label("2");
            self.em.ins("tst r20");
            self.em.ins("brne 3f");
            self.em.ins("tst r21");
            self.em.ins("breq 4f");
            self.em.label("3");
            self.em.ins("ldi r21, 1");
            self.em.ins("subi r20, -48"); // + '0'
            self.em.ins("mov r24, r20");
            self.em.ins("call uart_write_byte");
            self.em.label("4");
            self.em.ins("ret");
        }
        if delay_ms {
            self.em.blank();
            self.em
                .comment("busy-wait delay: r24:r25 milliseconds (16 MHz, ~1 ms)");
            self.em.label("delay_ms");
            self.em.ins("sbiw r24, 0");
            self.em.ins("breq 9f");
            self.em.label("1");
            self.em.ins("ldi r18, 9");
            self.em.label("2");
            self.em.ins("ldi r19, 254");
            self.em.label("3");
            for _ in 0..4 {
                self.em.ins("nop");
            }
            self.em.ins("dec r19");
            self.em.ins("brne 3b");
            self.em.ins("dec r18");
            self.em.ins("brne 2b");
            self.em.ins("sbiw r24, 1");
            self.em.ins("brne 1b");
            self.em.label("9");
            self.em.ins("ret");
        }
        if delay_us {
            self.em.blank();
            self.em
                .comment("busy-wait delay: r24:r25 microseconds (16 MHz, ~16 cycles/us)");
            self.em.label("delay_us");
            self.em.ins("sbiw r24, 0");
            self.em.ins("breq 9f");
            self.em.label("1");
            for _ in 0..12 {
                self.em.ins("nop");
            }
            self.em.ins("sbiw r24, 1");
            self.em.ins("brne 1b");
            self.em.label("9");
            self.em.ins("ret");
        }
        if divide {
            self.em.blank();
            self.em
                .comment("unsigned 16-bit division: r24:r25 / r22:r23 -> quotient r24:r25");
            self.em.label("udiv16");
            self.em.ins("mov r18, r22");
            self.em.ins("or r18, r23");
            self.em.ins("breq 9f"); // divisor 0 -> result 0
            self.em.ins("ldi r18, 16");
            self.em.ins("clr r20");
            self.em.ins("clr r21");
            self.em.label("1");
            self.em.ins("lsl r24");
            self.em.ins("rol r25");
            self.em.ins("rol r20");
            self.em.ins("rol r21");
            self.em.ins("cp r20, r22");
            self.em.ins("cpc r21, r23");
            self.em.ins("brlo 2f");
            self.em.ins("sub r20, r22");
            self.em.ins("sbc r21, r23");
            self.em.ins("ori r24, 1");
            self.em.label("2");
            self.em.ins("dec r18");
            self.em.ins("brne 1b");
            self.em.ins("ret");
            self.em.label("9");
            self.em.ins("clr r24");
            self.em.ins("clr r25");
            self.em.ins("ret");
        }
    }

    fn emit_data(&mut self) {
        let lits = collect_strings(self.g).0;
        if lits.is_empty() {
            return;
        }
        self.em.blank();
        self.em.push(".section .data");
        for lit in lits {
            emit_bytes(&mut self.em, &lit.label, &lit.bytes);
        }
    }

    fn emit_bss(&mut self) {
        self.em.blank();
        self.em.push(".section .bss");
        for name in self.g.all_variable_names() {
            self.em.label(&var_label(&name));
            self.em.ins(".zero 2");
        }
        self.em.comment("expression temporaries");
        for i in 0..64 {
            self.em.label(&format!("Wtmp{i}"));
            self.em.ins(".zero 2");
        }
        for i in 0..2 {
            self.em.label(&format!("Warg{i}"));
            self.em.ins(".zero 2");
        }
    }
}

struct PinMap {
    port: u8,
    ddr: u8,
    pin: u8,
    bit: u8,
}

/// Arduino pin number -> ATmega328P port registers (data-space addresses).
fn pin_map(pin: u64) -> Option<PinMap> {
    let (port, ddr, pin_reg, bit) = match pin {
        0..=7 => (0x2B, 0x2A, 0x29, pin as u8),          // PD0..PD7
        8..=13 => (0x25, 0x24, 0x23, (pin - 8) as u8),   // PB0..PB5
        14..=19 => (0x28, 0x27, 0x26, (pin - 14) as u8), // PC0..PC5
        _ => return None,
    };
    Some(PinMap {
        port,
        ddr,
        pin: pin_reg,
        bit,
    })
}

fn var_label(name: &str) -> String {
    format!("V{}_{:04X}", sanitize(name), fnv1a(name))
}

fn fnv1a(s: &str) -> u16 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    (h ^ (h >> 16)) as u16
}

fn avr_reg(reg: &str) -> String {
    let r = reg.trim().trim_start_matches('%');
    if r.is_empty() {
        "r24".to_string()
    } else {
        r.to_string()
    }
}
