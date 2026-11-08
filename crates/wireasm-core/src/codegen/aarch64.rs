use super::*;
use crate::model::*;
use crate::validate::Diagnostic;

pub fn generate(
    g: &Graph,
    t: Target,
    diags: &mut Vec<Diagnostic>,
) -> Result<String, String> {
    let mut cg = Cg {
        g,
        t,
        layout: Layout::new(g),
        strings: collect_strings(g).1,
        em: Emitter::new(),
        visited: std::collections::HashSet::new(),
        inline_seq: 0,
        _diags: diags,
    };
    cg.run()
}

struct Cg<'a> {
    g: &'a Graph,
    t: Target,
    layout: Layout,
    strings: std::collections::BTreeMap<String, String>,
    em: Emitter,
    visited: std::collections::HashSet<String>,
    inline_seq: usize,
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
        self.em.push(format!(".global {}", self.t.entry_symbol()));
        self.em.push(".p2align 2");
        self.em.blank();
        self.em.label(self.t.entry_symbol());
        self.em.ins("stp x29, x30, [sp, #-16]!");
        self.em.ins("mov x29, sp");
        if self.layout.frame <= 4095 {
            self.em
                .ins(format!("sub sp, sp, #{}", self.layout.frame));
        } else {
            let frame = self.layout.frame as u64;
            self.mov_imm("x9", frame);
            self.em.ins("sub sp, sp, x9");
        }

        let vars: Vec<(String, usize)> = self
            .layout
            .var_off
            .iter()
            .map(|(k, v)| (k.clone(), *v))
            .collect();
        if !vars.is_empty() {
            self.em.comment("graph variables");
        }
        for (name, off) in vars {
            self.mov_imm("x0", self.g.init_of(&name));
            self.em.ins(format!("str x0, {}", a_mem(off)));
        }

        self.em.blank();
        self.em.comment("---- program ----");
        let label = sanitize(&entry);
        self.em.label(&label);
        self.em.comment("Entry");
        self.walk_next(&entry)?;

        self.em.blank();
        self.em.label("Ldefault_ret");
        self.em.comment("implicit return 0");
        self.mov_imm("x0", 0);
        self.emit_epilogue();

        self.emit_runtime();
        self.emit_data();
        {
            let mut s = self.em.lines.join("\n");
            s.push('\n');
            Ok(s)
        }
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
                let name = node.prop("name").trim().to_string();
                let off = self
                    .layout
                    .var(&name)
                    .ok_or_else(|| format!("unknown variable '{name}'"))?;
                self.em.ins(format!("str x0, {}", a_mem(off)));
            }
            "RegSet" => {
                self.eval_input(&node, "value", 0)?;
                let reg = a64_reg(node.prop("reg"));
                self.em.ins(format!("mov {reg}, x0"));
            }
            "PrintU64" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("bl print_u64");
            }
            "PrintStr" => {
                let text = node.prop("text");
                let len = text.len();
                let label = self
                    .strings
                    .get(&node.id)
                    .cloned()
                    .unwrap_or_else(|| "S_missing".to_string());
                if self.t.is_macos() {
                    self.em.ins(format!("adrp x1, {label}@PAGE"));
                    self.em.ins(format!("add x1, x1, {label}@PAGEOFF"));
                } else {
                    self.em.ins(format!("adrp x1, {label}"));
                    self.em.ins(format!("add x1, x1, :lo12:{label}"));
                }
                self.em.ins("mov x0, #1");
                self.mov_imm("x2", len as u64);
                if self.t.is_macos() {
                    self.em.ins("mov x16, #4");
                    self.em.ins("svc #0x80");
                } else {
                    self.em.ins("mov x8, #64");
                    self.em.ins("svc #0");
                }
            }
            "Call" => {
                for i in 0..4 {
                    self.eval_arg(&node, i)?;
                }
                for i in 0..4 {
                    self.em.ins(format!("ldr x{i}, {}", a_mem(self.layout.arg(i))));
                }
                let sym = c_symbol(node.prop("func").trim(), self.t);
                self.em.ins(format!("bl {sym}"));
                self.em
                    .ins(format!("str x0, {}", a_mem(self.layout.callret())));
            }
            "Syscall" => {
                for i in 0..4 {
                    self.eval_arg(&node, i)?;
                }
                for i in 0..4 {
                    self.em.ins(format!("ldr x{i}, {}", a_mem(self.layout.arg(i))));
                }
                self.mov_imm("x9", node.prop_u64("number"));
                if self.t.is_macos() {
                    self.em.ins("mov x16, x9");
                    self.em.ins("svc #0x80");
                } else {
                    self.em.ins("mov x8, x9");
                    self.em.ins("svc #0");
                }
                self.em
                    .ins(format!("str x0, {}", a_mem(self.layout.callret())));
            }
            "RawAsm" => {
                self.eval_input(&node, "a", 0)?;
                self.em
                    .ins(format!("str x0, {}", a_mem(self.layout.arg(0))));
                self.eval_input(&node, "b", 0)?;
                self.em.ins("mov x1, x0");
                self.em
                    .ins(format!("ldr x0, {}", a_mem(self.layout.arg(0))));
                let text = node.prop("text").replace("{a}", "x0").replace("{b}", "x1");
                self.em.comment("raw assembly begin");
                emit_snippet(&mut self.em, &text);
                self.em.comment("raw assembly end");
                self.em
                    .ins(format!("str x0, {}", a_mem(self.layout.rawret())));
            }
            "Return" => {
                self.eval_input(&node, "value", 0)?;
                self.emit_epilogue();
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
                match &t_target {
                    Some(t) => self.em.ins(format!("cbnz x0, {}", sanitize(t))),
                    None => self.em.ins("cbnz x0, Ldefault_ret"),
                }
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
            other if self.g.custom(other).is_some() => {
                let def = self.g.custom(other).cloned().unwrap_or_default();
                if !def.has_exec_input() {
                    return Err(format!(
                        "node {} is a data-only custom node and cannot be on the exec wire",
                        node.id
                    ));
                }
                let inputs = def.u64_inputs();
                for (i, p) in inputs.iter().enumerate() {
                    self.eval_input(&node, &p.name, 0)?;
                    self.em
                        .ins(format!("str x0, {}", a_mem(self.layout.arg(i))));
                }
                for i in 0..inputs.len() {
                    self.em
                        .ins(format!("ldr x{i}, {}", a_mem(self.layout.arg(i))));
                }
                let regs: Vec<String> = (0..inputs.len()).map(|i| format!("x{i}")).collect();
                let seq = self.next_inline();
                let label = format!("Lcustom_{}_{seq}", sanitize(&node.id));
                let text = expand_custom_template(&def.asm_aarch64, &node, &regs, "x0", &label);
                self.em.comment("custom node begin");
                emit_snippet(&mut self.em, &text);
                self.em.comment("custom node end");
                if def.u64_output().is_some() {
                    let off = self
                        .layout
                        .custom_slot(&node.id)
                        .ok_or_else(|| format!("no result slot for {}", node.id))?;
                    self.em.ins(format!("str x0, {}", a_mem(off)));
                }
            }
            other => {
                return Err(format!("node {} has unsupported kind {other}", node.id));
            }
        }
        self.walk_next(id)
    }

    fn eval_input(&mut self, node: &Node, pin: &str, depth: usize) -> Result<(), String> {
        match self.g.incoming(&node.id, pin) {
            Some(e) => {
                let src = e.from_node.clone();
                self.eval(&src, depth)
            }
            None => {
                self.mov_imm("x0", 0);
                Ok(())
            }
        }
    }

    fn eval_arg(&mut self, node: &Node, i: usize) -> Result<(), String> {
        let pin = format!("a{i}");
        match self.g.incoming(&node.id, &pin) {
            Some(e) => {
                let src = e.from_node.clone();
                self.eval(&src, 0)?;
            }
            None => self.mov_imm("x0", 0),
        }
        self.em
            .ins(format!("str x0, {}", a_mem(self.layout.arg(i))));
        Ok(())
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
                self.mov_imm("x0", node.prop_u64("value"));
            }
            "VarGet" => {
                let name = node.prop("name").trim().to_string();
                let off = self
                    .layout
                    .var(&name)
                    .ok_or_else(|| format!("unknown variable '{name}'"))?;
                self.em.ins(format!("ldr x0, {}", a_mem(off)));
            }
            "BinOp" => {
                let a = self.input_of(&node, "a")?;
                self.eval(&a, depth)?;
                self.em
                    .ins(format!("str x0, {}", a_mem(self.layout.temp(depth))));
                let b = self.input_of(&node, "b")?;
                self.eval(&b, depth + 1)?;
                self.em.ins("mov x1, x0");
                self.em
                    .ins(format!("ldr x0, {}", a_mem(self.layout.temp(depth))));
                let op = node.prop("op");
                let mnem = match op {
                    "add" => "add",
                    "sub" => "sub",
                    "mul" => "mul",
                    "div" => "udiv",
                    "and" => "and",
                    "or" => "orr",
                    "xor" => "eor",
                    "shl" => "lsl",
                    "shr" => "lsr",
                    _ => return Err(format!("bad binop {op}")),
                };
                self.em.ins(format!("{mnem} x0, x0, x1"));
            }
            "Cmp" => {
                let a = self.input_of(&node, "a")?;
                self.eval(&a, depth)?;
                self.em
                    .ins(format!("str x0, {}", a_mem(self.layout.temp(depth))));
                let b = self.input_of(&node, "b")?;
                self.eval(&b, depth + 1)?;
                self.em.ins("mov x1, x0");
                self.em
                    .ins(format!("ldr x0, {}", a_mem(self.layout.temp(depth))));
                self.em.ins("cmp x0, x1");
                let cc = match node.prop("op") {
                    "eq" => "eq",
                    "ne" => "ne",
                    "lt" => "lo",
                    "le" => "ls",
                    "gt" => "hi",
                    "ge" => "hs",
                    _ => return Err("bad cmp".to_string()),
                };
                self.em.ins(format!("cset x0, {cc}"));
            }
            "RegGet" => {
                let reg = a64_reg(node.prop("reg"));
                self.em.ins(format!("mov x0, {reg}"));
            }
            "Call" | "Syscall" => {
                self.em
                    .ins(format!("ldr x0, {}", a_mem(self.layout.callret())));
            }
            "RawAsm" => {
                self.em
                    .ins(format!("ldr x0, {}", a_mem(self.layout.rawret())));
            }
            other if self.g.custom(other).is_some() => {
                let def = self.g.custom(other).cloned().unwrap_or_default();
                if def.has_exec_input() {
                    let off = self.layout.custom_slot(&node.id).ok_or_else(|| {
                        format!("custom node {} has no u64 output", node.id)
                    })?;
                    self.em.ins(format!("ldr x0, {}", a_mem(off)));
                } else {
                    let inputs = def.u64_inputs();
                    for (i, p) in inputs.iter().enumerate() {
                        match self.g.incoming(&node.id, &p.name) {
                            Some(e) => {
                                let src = e.from_node.clone();
                                self.eval(&src, depth + 1)?;
                            }
                            None => self.mov_imm("x0", 0),
                        }
                        self.em.ins(format!(
                            "str x0, {}",
                            a_mem(self.layout.custom_temp(depth, i))
                        ));
                    }
                    for i in 0..inputs.len() {
                        self.em.ins(format!(
                            "ldr x{i}, {}",
                            a_mem(self.layout.custom_temp(depth, i))
                        ));
                    }
                    let regs: Vec<String> =
                        (0..inputs.len()).map(|i| format!("x{i}")).collect();
                    let seq = self.next_inline();
                    let label = format!("Lcustom_{}_{seq}", sanitize(&node.id));
                    let text =
                        expand_custom_template(&def.asm_aarch64, &node, &regs, "x0", &label);
                    self.em.comment("custom node begin");
                    emit_snippet(&mut self.em, &text);
                    self.em.comment("custom node end");
                }
            }
            other => return Err(format!("node {id} ({other}) has no data output")),
        }
        Ok(())
    }

    fn input_of(&self, node: &Node, pin: &str) -> Result<String, String> {
        self.g
            .incoming(&node.id, pin)
            .map(|e| e.from_node.clone())
            .ok_or_else(|| format!("input {pin} of {} is not connected", node.id))
    }

    fn next_inline(&mut self) -> usize {
        let n = self.inline_seq;
        self.inline_seq += 1;
        n
    }

    fn mov_imm(&mut self, reg: &str, v: u64) {
        if v <= 0xffff {
            self.em.ins(format!("mov {reg}, #{v}"));
            return;
        }
        self.em.ins(format!("movz {reg}, #{}", v & 0xffff));
        for (i, shift) in [16u32, 32, 48].iter().enumerate() {
            let chunk = (v >> *shift) & 0xffff;
            if chunk != 0 {
                self.em
                    .ins(format!("movk {reg}, #{chunk}, lsl #{}", 16 * (i + 1)));
            }
        }
    }

    fn emit_epilogue(&mut self) {
        if self.t.is_start() {
            if self.t.is_macos() {
                self.em.ins("mov x16, #1");
                self.em.ins("svc #0x80");
            } else {
                self.em.ins("mov x8, #93");
                self.em.ins("svc #0");
            }
        } else {
            self.em.ins("mov sp, x29");
            self.em.ins("ldp x29, x30, [sp], #16");
            self.em.ins("ret");
        }
    }

    fn emit_runtime(&mut self) {
        if uses_print_u64(self.g) {
            self.em.blank();
            self.em.comment("runtime: print unsigned 64-bit decimal in x0");
            self.em.label("print_u64");
            self.em.ins("sub sp, sp, #80");
            self.em.ins("add x1, sp, #79");
            self.em.ins("mov x2, #0");
            self.em.ins("mov x3, #10");
            self.em.label(".Lpu_loop");
            self.em.ins("udiv x4, x0, x3");
            self.em.ins("msub x5, x4, x3, x0");
            self.em.ins("add w5, w5, #48");
            self.em.ins("strb w5, [x1, #-1]!");
            self.em.ins("add x2, x2, #1");
            self.em.ins("mov x0, x4");
            self.em.ins("cbnz x0, .Lpu_loop");
            self.em.comment("append newline (byte just past the highest digit)");
            self.em.ins("mov w5, #10");
            self.em.ins("strb w5, [sp, #79]");
            self.em.ins("add x2, x2, #1");
            self.em.ins("mov x0, #1");
            if self.t.is_macos() {
                self.em.ins("mov x16, #4");
                self.em.ins("svc #0x80");
            } else {
                self.em.ins("mov x8, #64");
                self.em.ins("svc #0");
            }
            self.em.ins("add sp, sp, #80");
            self.em.ins("ret");
        }
    }

    fn emit_data(&mut self) {
        let lits = collect_strings(self.g).0;
        if lits.is_empty() {
            return;
        }
        self.em.blank();
        self.em.push(data_section(self.t));
        self.em.push(".p2align 2");
        for lit in lits {
            emit_bytes(&mut self.em, &lit.label, &lit.bytes);
        }
    }
}

fn a64_reg(reg: &str) -> String {
    let r = reg.trim().trim_start_matches('%');
    match r {
        "fp" => "x29".to_string(),
        "lr" => "x30".to_string(),
        "xzr" | "wzr" => "xzr".to_string(),
        "" => "x0".to_string(),
        _ => r.to_string(),
    }
}

fn a_mem(off: usize) -> String {
    format!("[sp, #{}]", off.saturating_sub(8))
}
