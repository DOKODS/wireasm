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
        self.em.push(".intel_syntax noprefix");
        self.em.push(".text");
        self.em.push(format!(".global {}", self.t.entry_symbol()));
        self.em.blank();

        let entry_owned = self.t.entry_symbol().to_string();
        self.em.label(&entry_owned);
        self.em.ins("push rbp");
        self.em.ins("mov rbp, rsp");
        self.em.ins(format!("sub rsp, {}", self.layout.frame));
        self.em.ins("and rsp, -16");

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
            self.mov_imm("rax", self.g.init_of(&name));
            self.em.ins(format!("mov [rbp-{off}], rax"));
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
        self.em.ins("xor eax, eax");
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
                let off = self
                    .layout
                    .var(&name)
                    .ok_or_else(|| format!("unknown variable '{name}'"))?;
                self.em.ins(format!("mov [rbp-{off}], rax"));
            }
            "RegSet" => {
                self.eval_input(&node, "value", 0)?;
                let reg = x86_reg(node.prop("reg"));
                if reg == "rsp" || reg == "rbp" {
                    self.em
                        .ins(format!("mov {reg}, rax // warning: clobbers frame pointer"));
                } else {
                    self.em.ins(format!("mov {reg}, rax"));
                }
            }
            "PrintU64" => {
                self.eval_input(&node, "value", 0)?;
                self.em.ins("mov rdi, rax");
                self.em.ins("call print_u64");
            }
            "PrintStr" => {
                let text = node.prop("text");
                let len = text.len();
                let label = self
                    .strings
                    .get(&node.id)
                    .cloned()
                    .unwrap_or_else(|| "S_missing".to_string());
                self.em.ins(format!("lea rsi, [rip+{label}]"));
                self.em.ins("mov edi, 1");
                self.mov_imm("rdx", len as u64);
                if self.t.is_macos() {
                    self.em.ins("mov eax, 0x2000004");
                } else {
                    self.em.ins("mov eax, 1");
                }
                self.em.ins("syscall");
            }
            "Call" => {
                for i in 0..4 {
                    self.eval_arg(&node, i)?;
                }
                for (i, reg) in ["rdi", "rsi", "rdx", "rcx"].iter().enumerate() {
                    self.em
                        .ins(format!("mov {reg}, [rbp-{}]", self.layout.arg(i)));
                }
                let sym = c_symbol(node.prop("func").trim(), self.t);
                self.em.ins(format!("call {sym}"));
                self.em
                    .ins(format!("mov [rbp-{}], rax", self.layout.callret()));
            }
            "Syscall" => {
                for i in 0..4 {
                    self.eval_arg(&node, i)?;
                }
                for (i, reg) in ["rdi", "rsi", "rdx", "r10"].iter().enumerate() {
                    self.em
                        .ins(format!("mov {reg}, [rbp-{}]", self.layout.arg(i)));
                }
                self.mov_imm("rax", node.prop_u64("number"));
                self.em.ins("syscall");
                self.em
                    .ins(format!("mov [rbp-{}], rax", self.layout.callret()));
            }
            "RawAsm" => {
                self.eval_input(&node, "a", 0)?;
                self.em.ins(format!("mov [rbp-{}], rax", self.layout.arg(0)));
                match self.g.incoming(&node.id, "b") {
                    Some(e) => {
                        let src = e.from_node.clone();
                        self.eval(&src, 0)?;
                        self.em.ins("mov r10, rax");
                    }
                    None => self.em.ins("xor r10d, r10d"),
                }
                self.em
                    .ins(format!("mov rax, [rbp-{}]", self.layout.arg(0)));
                let text = node
                    .prop("text")
                    .replace("{a}", "rax")
                    .replace("{b}", "r10");
                self.em.comment("raw assembly begin");
                emit_snippet(&mut self.em, &text);
                self.em.comment("raw assembly end");
                self.em
                    .ins(format!("mov [rbp-{}], rax", self.layout.rawret()));
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
                self.em.ins("test rax, rax");
                match &t_target {
                    Some(t) => self.em.ins(format!("jne {}", sanitize(t))),
                    None => self.em.ins("jne Ldefault_ret"),
                }
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
                    self.em.ins(format!(
                        "mov [rbp-{}], rax",
                        self.layout.arg(i)
                    ));
                }
                let contract = ["rdi", "rsi", "rdx", "rcx", "r8", "r9", "r10", "r11"];
                for (i, reg) in contract.iter().enumerate().take(inputs.len()) {
                    self.em.ins(format!(
                        "mov {reg}, [rbp-{}]",
                        self.layout.arg(i)
                    ));
                }
                let regs: Vec<String> = contract[..inputs.len()]
                    .iter()
                    .map(|s| s.to_string())
                    .collect();
                let seq = self.next_inline();
                let label = format!("Lcustom_{}_{seq}", sanitize(&node.id));
                let text = expand_custom_template(&def.asm_x86_64, &node, &regs, "rax", &label);
                self.em.comment("custom node begin");
                emit_snippet(&mut self.em, &text);
                self.em.comment("custom node end");
                if def.u64_output().is_some() {
                    let off = self
                        .layout
                        .custom_slot(&node.id)
                        .ok_or_else(|| format!("no result slot for {}", node.id))?;
                    self.em.ins(format!("mov [rbp-{off}], rax"));
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
                self.mov_imm("rax", 0);
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
            None => self.mov_imm("rax", 0),
        }
        self.em
            .ins(format!("mov [rbp-{}], rax", self.layout.arg(i)));
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
                self.mov_imm("rax", node.prop_u64("value"));
            }
            "VarGet" => {
                let name = node.prop("name").trim().to_string();
                let off = self
                    .layout
                    .var(&name)
                    .ok_or_else(|| format!("unknown variable '{name}'"))?;
                self.em.ins(format!("mov rax, [rbp-{off}]"));
            }
            "BinOp" => {
                let a = self.input_of(&node, "a")?;
                self.eval(&a, depth)?;
                self.em
                    .ins(format!("mov [rbp-{}], rax", self.layout.temp(depth)));
                let b = self.input_of(&node, "b")?;
                self.eval(&b, depth + 1)?;
                self.em.ins("mov r10, rax");
                self.em
                    .ins(format!("mov rax, [rbp-{}]", self.layout.temp(depth)));
                let op = node.prop("op");
                match op {
                    "add" => self.em.ins("add rax, r10"),
                    "sub" => self.em.ins("sub rax, r10"),
                    "mul" => self.em.ins("imul rax, r10"),
                    "div" => {
                        self.em.ins("xor edx, edx");
                        self.em.ins("div r10");
                    }
                    "and" => self.em.ins("and rax, r10"),
                    "or" => self.em.ins("or rax, r10"),
                    "xor" => self.em.ins("xor rax, r10"),
                    "shl" => {
                        self.em.ins("mov rcx, r10");
                        self.em.ins("shl rax, cl");
                    }
                    "shr" => {
                        self.em.ins("mov rcx, r10");
                        self.em.ins("shr rax, cl");
                    }
                    _ => return Err(format!("bad binop {op}")),
                }
            }
            "Cmp" => {
                let a = self.input_of(&node, "a")?;
                self.eval(&a, depth)?;
                self.em
                    .ins(format!("mov [rbp-{}], rax", self.layout.temp(depth)));
                let b = self.input_of(&node, "b")?;
                self.eval(&b, depth + 1)?;
                self.em.ins("mov r10, rax");
                self.em
                    .ins(format!("mov rax, [rbp-{}]", self.layout.temp(depth)));
                self.em.ins("cmp rax, r10");
                let cc = match node.prop("op") {
                    "eq" => "sete",
                    "ne" => "setne",
                    "lt" => "setb",
                    "le" => "setbe",
                    "gt" => "seta",
                    "ge" => "setae",
                    _ => return Err("bad cmp".to_string()),
                };
                self.em.ins(format!("{cc} al"));
                self.em.ins("movzx rax, al");
            }
            "RegGet" => {
                let reg = x86_reg(node.prop("reg"));
                self.em.ins(format!("mov rax, {reg}"));
            }
            "Call" | "Syscall" => {
                self.em
                    .ins(format!("mov rax, [rbp-{}]", self.layout.callret()));
            }
            "RawAsm" => {
                self.em
                    .ins(format!("mov rax, [rbp-{}]", self.layout.rawret()));
            }
            other if self.g.custom(other).is_some() => {
                let def = self.g.custom(other).cloned().unwrap_or_default();
                if def.has_exec_input() {
                    let off = self.layout.custom_slot(&node.id).ok_or_else(|| {
                        format!("custom node {} has no u64 output", node.id)
                    })?;
                    self.em.ins(format!("mov rax, [rbp-{off}]"));
                } else {
                    let inputs = def.u64_inputs();
                    for (i, p) in inputs.iter().enumerate() {
                        match self.g.incoming(&node.id, &p.name) {
                            Some(e) => {
                                let src = e.from_node.clone();
                                self.eval(&src, depth + 1)?;
                            }
                            None => self.mov_imm("rax", 0),
                        }
                        self.em.ins(format!(
                            "mov [rbp-{}], rax",
                            self.layout.custom_temp(depth, i)
                        ));
                    }
                    let contract = ["rdi", "rsi", "rdx", "rcx", "r8", "r9", "r10", "r11"];
                    for (i, reg) in contract.iter().enumerate().take(inputs.len()) {
                        self.em.ins(format!(
                            "mov {reg}, [rbp-{}]",
                            self.layout.custom_temp(depth, i)
                        ));
                    }
                    let regs: Vec<String> = contract[..inputs.len()]
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    let seq = self.next_inline();
                    let label = format!("Lcustom_{}_{seq}", sanitize(&node.id));
                    let text =
                        expand_custom_template(&def.asm_x86_64, &node, &regs, "rax", &label);
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
        if v <= i32::MAX as u64 {
            self.em.ins(format!("mov {reg}, {v}"));
        } else {
            self.em.ins(format!("movabs {reg}, 0x{v:x}"));
        }
    }

    fn emit_epilogue(&mut self) {
        if self.t.is_start() {
            self.em.ins("mov rdi, rax");
            if self.t.is_macos() {
                self.em.ins("mov eax, 0x2000001");
            } else {
                self.em.ins("mov eax, 60");
            }
            self.em.ins("syscall");
        } else {
            self.em.ins("mov rsp, rbp");
            self.em.ins("pop rbp");
            self.em.ins("ret");
        }
    }

    fn emit_runtime(&mut self) {
        if uses_print_u64(self.g) {
            self.em.blank();
            self.em.comment("runtime: print unsigned 64-bit decimal from rdi");
            self.em.label("print_u64");
            self.em.ins("mov rax, rdi");
            self.em.ins("sub rsp, 80");
            self.em.ins("lea rsi, [rsp+79]");
            self.em.ins("xor ecx, ecx");
            self.em.ins("mov r8, 10");
            self.em.label(".Lpu_loop");
            self.em.ins("xor edx, edx");
            self.em.ins("div r8");
            self.em.ins("add dl, 48");
            self.em.ins("dec rsi");
            self.em.ins("mov [rsi], dl");
            self.em.ins("inc ecx");
            self.em.ins("test rax, rax");
            self.em.ins("jnz .Lpu_loop");
            self.em.ins("mov byte ptr [rsp+79], 10");
            self.em.ins("inc ecx");
            self.em.ins("mov rdx, rcx");
            self.em.ins("mov edi, 1");
            if self.t.is_macos() {
                self.em.ins("mov eax, 0x2000004");
            } else {
                self.em.ins("mov eax, 1");
            }
            self.em.ins("syscall");
            self.em.ins("add rsp, 80");
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

fn x86_reg(reg: &str) -> String {
    let r = reg.trim().trim_start_matches('%');
    match r {
        "fp" => "rbp".to_string(),
        "sp" => "rsp".to_string(),
        "" => "rax".to_string(),
        _ => r.to_string(),
    }
}
