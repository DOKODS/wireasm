//! STM32F411 (Cortex-M4) backend: bare-metal Thumb-2 assembly.
//!
//! The node language works with u64 values, but this port keeps them in a
//! 32-bit register (`r0`). Values above 0xFFFF_FFFF are truncated; the
//! validator warns about it. Variables and expression temporaries live in
//! SRAM, so nested expressions cannot clobber each other.
//!
//! The generated file is a complete image: vector table, reset handler, `main`
//! and runtime helpers. It is linked by `arm-none-eabi-gcc -nostdlib` with the
//! linker script produced in `toolchain.rs`, so no libc or startup files are
//! needed. The chip runs from the 16 MHz HSI clock.

use super::*;
use crate::model::*;
use crate::validate::Diagnostic;
use std::collections::HashSet;

const RCC_AHB1ENR: u32 = 0x4002_3830;
const RCC_APB1ENR: u32 = 0x4002_3840;
const RCC_APB2ENR: u32 = 0x4002_3844;
const GPIO_BASE: u32 = 0x4002_0000;
const USART2_BASE: u32 = 0x4000_4400;
const ADC1_BASE: u32 = 0x4001_2000;
const SYSTICK_BASE: u32 = 0xE000_E010;

pub fn generate(g: &Graph, _t: Target, diags: &mut Vec<Diagnostic>) -> Result<String, String> {
    let mut cg = Cg {
        g,
        strings: collect_strings(g).1,
        em: Emitter::with_comment("@"),
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
        self.em.push(".syntax unified");
        self.em.push(".cpu cortex-m4");
        self.em.push(".fpu fpv4-sp-d16");
        self.em.push(".thumb");
        self.em.blank();
        self.em.push(".section .isr_vector,\"a\"");
        self.em.ins(".word _estack");
        self.em.ins(".word Reset_Handler");
        for _ in 0..14 {
            self.em.ins(".word Default_Handler");
        }
        self.em.blank();
        self.em.push(".text");
        self.em.push(".global Reset_Handler");
        self.em.label("Reset_Handler");
        self.em.ins(".thumb_func");
        self.em.ins("bl main");
        self.em.label("1");
        self.em.ins("b 1b");
        self.em.label("Default_Handler");
        self.em.ins(".thumb_func");
        self.em.ins("b Default_Handler");

        self.em.blank();
        self.em.push(".global main");
        self.em.label("main");
        self.em.ins(".thumb_func");
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
            let init = self.g.init_of(&name) as u32;
            self.em.comment(format!("variable {name} = {init}"));
            self.load_addr("r3", &var_label(&name));
            self.ldi32("r0", init as u64);
            self.em.ins("str r0, [r3]");
        }

        self.walk_next(&entry)?;

        self.em.blank();
        self.em.label("Ldefault_ret");
        self.em
            .comment("implicit return: back to the reset handler loop");
        self.em.ins("bx lr");

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
                self.em.ins("b Ldefault_ret");
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
            self.em.ins(format!("b {label}"));
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
                self.load_addr("r3", &var_label(node.prop("name").trim()));
                self.em.ins("str r0, [r3]");
            }
            "RegSet" => {
                self.eval_input(&node, "value", 0)?;
                let reg = stm_reg(node.prop("reg"));
                self.em.ins(format!("mov {reg}, r0"));
            }
            "PrintU64" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("bl uart_print_u32");
            }
            "PrintStr" | "McuUartWriteStr" => {
                let label = self
                    .strings
                    .get(&node.id)
                    .cloned()
                    .unwrap_or_else(|| "Lstr_missing".to_string());
                self.load_addr("r0", &label);
                self.em.ins("bl uart_write_str");
            }
            "McuPinMode" => {
                let pin = pin_of(&node)?;
                self.gpio_clock(pin.0);
                match node.prop("mode").trim() {
                    "out" => {
                        self.gpio_moder(pin, 1);
                    }
                    "pullup" => {
                        self.gpio_moder(pin, 0);
                        self.gpio_pupdr(pin, 1);
                    }
                    _ => {
                        self.gpio_moder(pin, 0);
                        self.gpio_pupdr(pin, 0);
                    }
                }
            }
            "McuDigitalWrite" => {
                let pin = pin_of(&node)?;
                self.gpio_clock(pin.0);
                self.eval_input(&node, "value", 0)?;
                self.em.ins("cmp r0, #0");
                self.em.ins("beq 1f");
                self.gpio_bsrr(pin, true);
                self.em.ins("b 2f");
                self.em.label("1");
                self.gpio_bsrr(pin, false);
                self.em.label("2");
            }
            "McuDigitalRead" => {
                let pin = pin_of(&node)?;
                self.em.ins(format!(
                    "ldr r1, =0x{:08X}",
                    GPIO_BASE + 0x400 * pin.0 as u32 + 0x10
                ));
                self.em.ins("ldr r0, [r1]");
                self.em.ins(format!("tst r0, #{}", 1u32 << pin.1));
                self.em.ins("beq 1f");
                self.em.ins("movs r0, #1");
                self.em.ins("b 2f");
                self.em.label("1");
                self.em.ins("movs r0, #0");
                self.em.label("2");
            }
            "McuDelayMs" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("bl delay_ms");
            }
            "McuDelayUs" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("bl delay_us");
            }
            "McuUartInit" => {
                let baud = node.prop_u64("baud").max(1);
                self.uart_init(baud);
            }
            "McuUartWrite" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("bl uart_write_byte");
            }
            "McuAdcRead" => {
                let ch = node.prop_u64("channel").min(15) as u8;
                self.adc_read(ch);
            }
            "McuRegisterWrite" => {
                let addr = node.prop_u64("address") as u32;
                self.eval_input(&node, "value", 0)?;
                self.em.ins(format!("ldr r1, =0x{addr:08X}"));
                self.em.ins("str r0, [r1]");
            }
            "McuRegisterRead" => {
                let addr = node.prop_u64("address") as u32;
                self.em.ins(format!("ldr r1, =0x{addr:08X}"));
                self.em.ins("ldr r0, [r1]");
            }
            "RawAsm" => {
                self.eval_input(&node, "a", 0)?;
                self.load_addr("r3", "Warg0");
                self.em.ins("str r0, [r3]");
                self.eval_input(&node, "b", 0)?;
                self.em.ins("mov r1, r0");
                self.load_addr("r3", "Warg0");
                self.em.ins("ldr r0, [r3]");
                let text = node
                    .prop("text")
                    .replace("{a}", "r0")
                    .replace("{b}", "r1")
                    .replace("{out}", "r0")
                    .replace("//", "@");
                self.em.comment("raw assembly begin");
                emit_snippet(&mut self.em, &text);
                self.em.comment("raw assembly end");
            }
            "Return" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("bx lr");
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
                self.em.ins("cmp r0, #0");
                self.em.ins("beq 1f");
                match &t_target {
                    Some(t) => self.em.ins(format!("b {}", sanitize(t))),
                    None => self.em.ins("b Ldefault_ret"),
                }
                self.em.label("1");
                match &f_target {
                    Some(f) => self.em.ins(format!("b {}", sanitize(f))),
                    None => self.em.ins("b Ldefault_ret"),
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
                    "node {} ({}) is not available on the STM32 target",
                    node.id, node.kind
                ));
            }
            other if self.g.custom(other).is_some() => {
                return Err(format!(
                    "custom node '{}' cannot be compiled for STM32 (no Cortex-M template)",
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
                self.em.ins("movs r0, #0");
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
                let v = node.prop_u64("value") as u32;
                self.ldi32("r0", v as u64);
            }
            "VarGet" => {
                self.load_addr("r3", &var_label(node.prop("name").trim()));
                self.em.ins("ldr r0, [r3]");
            }
            "BinOp" => {
                let a = self.input_of(&node, "a")?;
                self.eval(&a, depth)?;
                self.store_temp(depth);
                let b = self.input_of(&node, "b")?;
                self.eval(&b, depth + 1)?;
                self.em.ins("mov r1, r0");
                self.load_temp(depth);
                let op = node.prop("op");
                match op {
                    "add" => self.em.ins("add r0, r1"),
                    "sub" => self.em.ins("sub r0, r1"),
                    "and" => self.em.ins("and r0, r1"),
                    "or" => self.em.ins("orr r0, r1"),
                    "xor" => self.em.ins("eor r0, r1"),
                    "mul" => self.em.ins("mul r0, r1, r0"),
                    "div" => self.em.ins("bl udiv32"),
                    "shl" => {
                        self.em.ins("cmp r1, #32");
                        self.em.ins("bhs 1f");
                        self.em.ins("lsl r0, r0, r1");
                        self.em.ins("b 2f");
                        self.em.label("1");
                        self.em.ins("movs r0, #0");
                        self.em.label("2");
                    }
                    "shr" => {
                        self.em.ins("cmp r1, #32");
                        self.em.ins("bhs 1f");
                        self.em.ins("lsr r0, r0, r1");
                        self.em.ins("b 2f");
                        self.em.label("1");
                        self.em.ins("movs r0, #0");
                        self.em.label("2");
                    }
                    _ => return Err(format!("bad binop {op}")),
                }
            }
            "Cmp" => {
                let a = self.input_of(&node, "a")?;
                self.eval(&a, depth)?;
                self.store_temp(depth);
                let b = self.input_of(&node, "b")?;
                self.eval(&b, depth + 1)?;
                self.em.ins("mov r1, r0");
                self.load_temp(depth);
                self.em.ins("cmp r0, r1");
                self.em.ins("movs r0, #0");
                match node.prop("op") {
                    "eq" => {
                        self.em.ins("bne 1f");
                        self.em.ins("movs r0, #1");
                        self.em.label("1");
                    }
                    "ne" => {
                        self.em.ins("beq 1f");
                        self.em.ins("movs r0, #1");
                        self.em.label("1");
                    }
                    "lt" => {
                        self.em.ins("bhs 1f");
                        self.em.ins("movs r0, #1");
                        self.em.label("1");
                    }
                    "le" => {
                        self.em.ins("bhi 1f");
                        self.em.ins("movs r0, #1");
                        self.em.label("1");
                    }
                    "gt" => {
                        self.em.ins("bls 1f");
                        self.em.ins("movs r0, #1");
                        self.em.label("1");
                    }
                    "ge" => {
                        self.em.ins("blo 1f");
                        self.em.ins("movs r0, #1");
                        self.em.label("1");
                    }
                    _ => return Err("bad cmp".to_string()),
                }
            }
            "RegGet" => {
                let reg = stm_reg(node.prop("reg"));
                self.em.ins(format!("mov r0, {reg}"));
            }
            "McuDigitalRead" => {
                let pin = pin_of(&node)?;
                self.em.ins(format!(
                    "ldr r1, =0x{:08X}",
                    GPIO_BASE + 0x400 * pin.0 as u32 + 0x10
                ));
                self.em.ins("ldr r0, [r1]");
                self.em.ins(format!("tst r0, #{}", 1u32 << pin.1));
                self.em.ins("beq 1f");
                self.em.ins("movs r0, #1");
                self.em.ins("b 2f");
                self.em.label("1");
                self.em.ins("movs r0, #0");
                self.em.label("2");
            }
            "McuAdcRead" => {
                let ch = node.prop_u64("channel").min(15) as u8;
                self.adc_read(ch);
            }
            "McuRegisterRead" => {
                let addr = node.prop_u64("address") as u32;
                self.em.ins(format!("ldr r1, =0x{addr:08X}"));
                self.em.ins("ldr r0, [r1]");
            }
            other if self.g.custom(other).is_some() => {
                return Err(format!(
                    "custom node '{}' cannot be compiled for STM32 (no Cortex-M template)",
                    other
                ));
            }
            other => return Err(format!("node {id} ({other}) has no STM32 data output")),
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

    fn ldi32(&mut self, reg: &str, v: u64) {
        if v <= 0xFFFF {
            self.em.ins(format!("ldr {reg}, =0x{:04X}", v & 0xFFFF));
        } else {
            self.em.ins(format!("ldr {reg}, =0x{:08X}", v as u32));
        }
    }

    fn load_addr(&mut self, reg: &str, label: &str) {
        self.em.ins(format!("ldr {reg}, ={label}"));
    }

    fn store_temp(&mut self, depth: usize) {
        self.load_addr("r3", &format!("Wtmp{depth}"));
        self.em.ins("str r0, [r3]");
    }

    fn load_temp(&mut self, depth: usize) {
        self.load_addr("r3", &format!("Wtmp{depth}"));
        self.em.ins("ldr r0, [r3]");
    }

    fn uses_uart(&self) -> bool {
        self.g.nodes.iter().any(|n| {
            matches!(
                n.kind.as_str(),
                "PrintU64" | "PrintStr" | "McuUartInit" | "McuUartWrite" | "McuUartWriteStr"
            )
        })
    }

    fn gpio_clock(&mut self, port: u8) {
        self.em.ins(format!("ldr r1, =0x{RCC_AHB1ENR:08X}"));
        self.em.ins("ldr r0, [r1]");
        self.em.ins(format!("orr r0, r0, #{}", 1u32 << port));
        self.em.ins("str r0, [r1]");
    }

    fn gpio_moder(&mut self, pin: (u8, u8), mode: u32) {
        let base = GPIO_BASE + 0x400 * pin.0 as u32;
        self.em.ins(format!("ldr r1, =0x{base:08X}"));
        self.em.ins("ldr r0, [r1]");
        self.em.ins(format!("bic r0, r0, #{}", 3u32 << (2 * pin.1)));
        if mode != 0 {
            self.em.ins(format!("orr r0, r0, #{}", mode << (2 * pin.1)));
        }
        self.em.ins("str r0, [r1]");
    }

    fn gpio_pupdr(&mut self, pin: (u8, u8), mode: u32) {
        let base = GPIO_BASE + 0x400 * pin.0 as u32 + 0x0C;
        self.em.ins(format!("ldr r1, =0x{base:08X}"));
        self.em.ins("ldr r0, [r1]");
        self.em.ins(format!("bic r0, r0, #{}", 3u32 << (2 * pin.1)));
        if mode != 0 {
            self.em.ins(format!("orr r0, r0, #{}", mode << (2 * pin.1)));
        }
        self.em.ins("str r0, [r1]");
    }

    fn gpio_bsrr(&mut self, pin: (u8, u8), high: bool) {
        let base = GPIO_BASE + 0x400 * pin.0 as u32 + 0x18;
        let value = if high {
            1u32 << pin.1
        } else {
            1u32 << (pin.1 + 16)
        };
        self.em.ins(format!("ldr r1, =0x{base:08X}"));
        self.ldi32("r0", value as u64);
        self.em.ins("str r0, [r1]");
    }

    fn uart_init(&mut self, baud: u64) {
        let baud = baud.max(1);
        // 16 MHz / baud, oversampling by 16: BRR = mantissa << 4 | fraction.
        let div = 16_000_000f64 / baud as f64;
        let mant = (div / 16.0).floor() as u32;
        let frac = ((div - mant as f64 * 16.0).round() as u32) & 0xF;
        let brr = (mant << 4) | frac;
        self.em.comment(format!(
            "USART2: {} baud, 8N1, TX PA2 / RX PA3, BRR = {brr} (16 MHz HSI)",
            baud
        ));
        self.em.ins(format!("ldr r1, =0x{RCC_AHB1ENR:08X}"));
        self.em.ins("ldr r0, [r1]");
        self.em.ins("orr r0, r0, #1");
        self.em.ins("str r0, [r1]");
        self.em.ins(format!("ldr r1, =0x{RCC_APB1ENR:08X}"));
        self.em.ins("ldr r0, [r1]");
        self.em.ins("ldr r2, =0x00020000");
        self.em.ins("orr r0, r0, r2");
        self.em.ins("str r0, [r1]");
        // PA2/PA3 alternate function mode.
        self.em.ins("ldr r1, =0x40020000");
        self.em.ins("ldr r0, [r1]");
        self.em.ins("bic r0, r0, #0xF0");
        self.em.ins("orr r0, r0, #0xA0");
        self.em.ins("str r0, [r1]");
        // AF7 in AFR[0] for pins 2 and 3.
        self.em.ins("ldr r1, =0x40020020");
        self.em.ins("ldr r0, [r1]");
        self.em.ins("bic r0, r0, #0x0000FF00");
        self.em.ins("orr r0, r0, #0x00007700");
        self.em.ins("str r0, [r1]");
        self.em.ins(format!("ldr r1, =0x{USART2_BASE:08X}"));
        self.ldi32("r0", brr as u64);
        self.em.ins("str r0, [r1, #8]");
        self.em.ins("movs r0, #0");
        self.em.ins("str r0, [r1, #12]");
        self.em.ins("ldr r0, =0x200C");
        self.em.ins("str r0, [r1, #12]");
    }

    fn adc_read(&mut self, ch: u8) {
        let (port, bit) = adc_pin(ch);
        self.gpio_clock(port);
        self.gpio_moder((port, bit), 3);
        self.em
            .comment(format!("ADC1 channel {ch} ({}P{})", port_name(port), bit));
        self.em.ins(format!("ldr r1, =0x{RCC_APB2ENR:08X}"));
        self.em.ins("ldr r0, [r1]");
        self.em.ins("orr r0, r0, #0x100");
        self.em.ins("str r0, [r1]");
        self.em.ins(format!("ldr r1, =0x{ADC1_BASE:08X}"));
        self.em.ins("movs r0, #0");
        self.em.ins("str r0, [r1, #4]");
        self.em.ins("ldr r0, =0x00000023");
        self.em.ins("str r0, [r1, #0]");
        self.em.ins("ldr r3, =0x40012304");
        self.em.ins("movs r0, #0");
        self.em.ins("str r0, [r3]");
        self.em.ins("ldr r0, [r1, #8]");
        self.em.ins("orr r0, r0, #1");
        self.em.ins("str r0, [r1, #8]");
        // Channel selection and a long sample time.
        self.em.ins(format!("movs r0, #{ch}"));
        self.em.ins("str r0, [r1, #0x34]");
        self.em.ins("str r0, [r1, #0x2C]");
        if ch < 10 {
            self.em
                .ins(format!("ldr r0, =0x{:08X}", 7u32 << (3 * ch as u32)));
            self.em.ins("str r0, [r1, #0x10]");
        } else {
            self.em
                .ins(format!("ldr r0, =0x{:08X}", 7u32 << (3 * (ch as u32 - 10))));
            self.em.ins("str r0, [r1, #0x0C]");
        }
        self.em.ins("ldr r0, [r1, #8]");
        self.em.ins("orr r0, r0, #0x40000000");
        self.em.ins("str r0, [r1, #8]");
        self.em.label("1");
        self.em.ins("ldr r0, [r1, #0]");
        self.em.ins("tst r0, #2");
        self.em.ins("beq 1b");
        self.em.ins("ldr r0, [r1, #0x4C]");
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
        // `uart_print_u32` divides by ten, so it needs the helper too.
        let divide = number
            || self
                .g
                .nodes
                .iter()
                .any(|n| n.kind == "BinOp" && n.prop("op") == "div");

        if byte {
            self.em.blank();
            self.em.comment("USART2: send the byte in r0, wait for TXE");
            self.em.label("uart_write_byte");
            self.em.ins(".thumb_func");
            self.em.ins(format!("ldr r1, =0x{USART2_BASE:08X}"));
            self.em.label("1");
            self.em.ins("ldr r2, [r1, #0]");
            self.em.ins("tst r2, #0x80");
            self.em.ins("beq 1b");
            self.em.ins("str r0, [r1, #4]");
            self.em.ins("bx lr");
        }
        if text {
            self.em.blank();
            self.em
                .comment("USART2: send a zero-terminated string from r0");
            self.em.label("uart_write_str");
            self.em.ins(".thumb_func");
            self.em.ins("push {r4, lr}");
            self.em.ins("mov r4, r0");
            self.em.label("1");
            self.em.ins("ldrb r0, [r4], #1");
            self.em.ins("cmp r0, #0");
            self.em.ins("beq 2f");
            self.em.ins("bl uart_write_byte");
            self.em.ins("b 1b");
            self.em.label("2");
            self.em.ins("pop {r4, pc}");
        }
        if number {
            self.em.blank();
            self.em
                .comment("USART2: print the unsigned value in r0 as decimal + newline");
            self.em.label("uart_print_u32");
            self.em.ins(".thumb_func");
            self.em.ins("push {r4, r5, lr}");
            self.em.ins("mov r4, r0");
            self.em.ins("movs r5, #0");
            self.em.label("1");
            self.em.ins("mov r0, r4");
            self.em.ins("movs r1, #10");
            self.em.ins("bl udiv32");
            self.em.ins("add r1, #48");
            self.em.ins("push {r1}");
            self.em.ins("adds r5, #1");
            self.em.ins("mov r4, r0");
            self.em.ins("cmp r4, #0");
            self.em.ins("bne 1b");
            self.em.label("2");
            self.em.ins("pop {r0}");
            self.em.ins("bl uart_write_byte");
            self.em.ins("subs r5, #1");
            self.em.ins("bne 2b");
            self.em.ins("movs r0, #10");
            self.em.ins("bl uart_write_byte");
            self.em.ins("pop {r4, r5, pc}");
        }
        if delay_ms || delay_us {
            self.em.blank();
            self.em
                .comment("SysTick at 16 MHz: one tick per microsecond");
            self.em.ins(format!("ldr r1, =0x{SYSTICK_BASE:08X}"));
            self.em.ins("ldr r0, =15999");
            self.em.ins("str r0, [r1, #4]");
        }
        if delay_ms {
            self.em.blank();
            self.em
                .comment("busy-wait delay: r0 milliseconds via SysTick");
            self.em.label("delay_ms");
            self.em.ins(".thumb_func");
            self.em.ins("push {r4, r5, lr}");
            self.em.ins("mov r4, r0");
            self.em.ins("ldr r5, =0xE000E010");
            self.em.label("1");
            self.em.ins("movs r0, #0");
            self.em.ins("str r0, [r5, #8]");
            self.em.ins("str r0, [r5, #0]");
            self.em.ins("ldr r0, =5");
            self.em.ins("str r0, [r5, #0]");
            self.em.label("2");
            self.em.ins("ldr r0, [r5, #0]");
            self.em.ins("tst r0, #0x10000");
            self.em.ins("beq 2b");
            self.em.ins("subs r4, #1");
            self.em.ins("bne 1b");
            self.em.ins("pop {r4, r5, pc}");
        }
        if delay_us {
            self.em.blank();
            self.em
                .comment("busy-wait delay: r0 microseconds (16 MHz, ~16 cycles/us)");
            self.em.label("delay_us");
            self.em.ins(".thumb_func");
            self.em.ins("cmp r0, #0");
            self.em.ins("beq 9f");
            self.em.label("1");
            for _ in 0..12 {
                self.em.ins("nop");
            }
            self.em.ins("subs r0, #1");
            self.em.ins("bne 1b");
            self.em.label("9");
            self.em.ins("bx lr");
        }
        if divide {
            self.em.blank();
            self.em
                .comment("unsigned 32-bit division: r0 / r1 -> quotient r0, remainder r1");
            self.em.label("udiv32");
            self.em.ins(".thumb_func");
            self.em.ins("cmp r1, #0");
            self.em.ins("beq 9f");
            self.em.ins("push {r2, r3, r4, lr}");
            self.em.ins("movs r2, #0");
            self.em.ins("movs r3, #0");
            self.em.ins("movs r4, #32");
            self.em.label("1");
            self.em.ins("lsls r2, r2, #1");
            self.em.ins("lsls r0, r0, #1");
            self.em.ins("adc r3, r3");
            self.em.ins("cmp r3, r1");
            self.em.ins("blo 2f");
            self.em.ins("sub r3, r1");
            self.em.ins("orr r2, r2, #1");
            self.em.label("2");
            self.em.ins("subs r4, #1");
            self.em.ins("bne 1b");
            self.em.ins("mov r0, r2");
            self.em.ins("mov r1, r3");
            self.em.ins("pop {r2, r3, r4, pc}");
            self.em.label("9");
            self.em.ins("movs r0, #0");
            self.em.ins("movs r1, #0");
            self.em.ins("bx lr");
        }
    }

    fn emit_data(&mut self) {
        let lits = collect_strings(self.g).0;
        if lits.is_empty() {
            return;
        }
        self.em.blank();
        self.em.push(".section .rodata");
        for lit in lits {
            emit_bytes(&mut self.em, &lit.label, &lit.bytes);
        }
    }

    fn emit_bss(&mut self) {
        self.em.blank();
        self.em.push(".section .bss");
        for name in self.g.all_variable_names() {
            self.em.label(&var_label(&name));
            self.em.ins(".zero 4");
        }
        self.em.comment("expression temporaries");
        for i in 0..64 {
            self.em.label(&format!("Wtmp{i}"));
            self.em.ins(".zero 4");
        }
        self.em.label("Warg0");
        self.em.ins(".zero 4");
    }
}

fn pin_of(node: &Node) -> Result<(u8, u8), String> {
    let port = port_index(node.prop("port")).ok_or_else(|| {
        format!(
            "invalid STM32 port '{}' (use A, B, C, D, E or H)",
            node.prop("port")
        )
    })?;
    let bit = node.prop_u64("pin");
    if bit > 15 {
        return Err(format!("STM32 pin {bit} is out of range (0..15)"));
    }
    Ok((port, bit as u8))
}

fn port_index(p: &str) -> Option<u8> {
    match p.trim().to_ascii_uppercase().as_str() {
        "A" | "" => Some(0),
        "B" => Some(1),
        "C" => Some(2),
        "D" => Some(3),
        "E" => Some(4),
        "F" => Some(5),
        "G" => Some(6),
        "H" => Some(7),
        _ => None,
    }
}

fn port_name(port: u8) -> &'static str {
    match port {
        0 => "PA",
        1 => "PB",
        2 => "PC",
        3 => "PD",
        4 => "PE",
        5 => "PF",
        6 => "PG",
        _ => "PH",
    }
}

/// STM32F411 ADC1 channel to GPIO pin: A0..A7, B0..B1, C0..C5.
fn adc_pin(ch: u8) -> (u8, u8) {
    match ch {
        0..=7 => (0, ch),
        8..=9 => (1, ch - 8),
        _ => (2, ch - 10),
    }
}

fn var_label(name: &str) -> String {
    format!("V{}_{:08X}", sanitize(name), fnv1a(name))
}

fn fnv1a(s: &str) -> u32 {
    let mut h: u32 = 0x811c_9dc5;
    for b in s.bytes() {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

fn stm_reg(reg: &str) -> String {
    let r = reg.trim().trim_start_matches('%');
    match r {
        "sp" => "sp".to_string(),
        "lr" => "lr".to_string(),
        "" => "r0".to_string(),
        _ => r.to_string(),
    }
}
