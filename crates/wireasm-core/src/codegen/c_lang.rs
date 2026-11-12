//! Host source backend: C, C++ and C#.
//!
//! The graph becomes a single source file with a `main` function. Data nodes
//! are inlined as expressions, flow nodes as statements and `goto` labels, so
//! the result is readable code that can be built with the host compiler (or
//! `dotnet` for C#) and opened in the built-in code editor.

use super::*;
use crate::model::*;
use crate::validate::Diagnostic;
use std::collections::HashSet;

pub fn generate(g: &Graph, t: Target, diags: &mut Vec<Diagnostic>) -> Result<String, String> {
    let mut cg = Cg {
        g,
        lang: t,
        lines: Vec::new(),
        visited: HashSet::new(),
        _diags: diags,
    };
    cg.run()
}

struct Cg<'a> {
    g: &'a Graph,
    lang: Target,
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
        match self.lang {
            Target::CLang => {
                self.lines.push("#include <stdint.h>".to_string());
                self.lines.push("#include <stdio.h>".to_string());
            }
            Target::Cpp => {
                self.lines.push("#include <cstdint>".to_string());
                self.lines.push("#include <cstdio>".to_string());
                self.lines.push("using u64 = std::uint64_t;".to_string());
            }
            _ => self.lines.push("using System;".to_string()),
        }
        self.lines.push(String::new());

        if self.lang == Target::CSharp {
            self.lines.push("internal static class Program".to_string());
            self.lines.push("{".to_string());
        }

        let pad = if self.lang == Target::CSharp {
            "    "
        } else {
            ""
        };
        for name in self.g.all_variable_names() {
            self.lines.push(format!(
                "{pad}static {} {} = {};",
                self.u64_type(),
                var_name(&name),
                self.init_literal(self.g.init_of(&name))
            ));
        }
        if !self.g.all_variable_names().is_empty() {
            self.lines.push(String::new());
        }

        match self.lang {
            Target::CSharp => self.lines.push(format!("{pad}static int Main()")),
            Target::Cpp => self.lines.push("int main()".to_string()),
            _ => self.lines.push("int main(void)".to_string()),
        }
        let open = if self.lang == Target::CSharp {
            format!("{pad}{{")
        } else {
            "{".to_string()
        };
        self.lines.push(open);
        self.walk_next(&entry, 1)?;
        self.lines.push("}".to_string());
        if self.lang == Target::CSharp {
            self.lines.push("}".to_string());
        }

        let mut s = self.lines.join("\n");
        s.push('\n');
        Ok(s)
    }

    fn emit(&mut self, indent: usize, line: &str) {
        let pad = if self.lang == Target::CSharp {
            "    "
        } else {
            ""
        };
        self.lines
            .push(format!("{pad}{}{}", "    ".repeat(indent), line));
    }

    fn u64_type(&self) -> &'static str {
        match self.lang {
            Target::CLang => "uint64_t",
            Target::Cpp => "u64",
            _ => "ulong",
        }
    }

    fn init_literal(&self, v: u64) -> String {
        match self.lang {
            Target::CSharp => format!("{v}UL"),
            _ => format!("{v}ULL"),
        }
    }

    fn walk_next(&mut self, id: &str, indent: usize) -> Result<(), String> {
        match self.g.exec_next(id) {
            Some(next) => self.walk(&next, indent),
            None => {
                self.emit(indent, "return 0;");
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
        self.emit(indent, &format!("// {}", node.id));

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
                match self.lang {
                    Target::CSharp => self.emit(indent, &format!("Console.WriteLine({value});")),
                    _ => self.emit(
                        indent,
                        &format!("printf(\"%llu\\n\", (unsigned long long)({value}));"),
                    ),
                }
            }
            "PrintStr" => {
                let text = c_string(node.prop("text"));
                match self.lang {
                    Target::CSharp => self.emit(indent, &format!("Console.Write(\"{text}\");")),
                    _ => self.emit(indent, &format!("printf(\"{text}\");")),
                }
            }
            "RawAsm" => {
                let text = node.prop("text");
                for line in text.lines() {
                    self.emit(indent, line);
                }
            }
            "Return" => {
                let value = self.expr_input(&node, "value")?;
                self.emit(indent, &format!("return (int)({value});"));
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
                        self.emit(indent, "return 0;");
                    }
                    (None, Some(f)) => {
                        self.emit(indent, &format!("if (!({cond})) goto {};", label(f)));
                        self.emit(indent, "return 0;");
                    }
                    (None, None) => self.emit(indent, "return 0;"),
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
                return Err("CPU registers are not available in host source targets".to_string());
            }
            kind if kind.starts_with("Mcu") => {
                return Err(format!(
                    "node {} ({}) needs a microcontroller target",
                    node.id, node.kind
                ));
            }
            "Call" | "Syscall" => {
                return Err(format!(
                    "node {} ({}) is not available in source targets",
                    node.id, node.kind
                ));
            }
            other if self.g.custom(other).is_some() => {
                return Err(format!(
                    "custom node '{}' has no source template for C/C++/C#",
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
            None => Ok("0".to_string()),
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
                None => Ok("0".to_string()),
            }
        };
        Ok(match node.kind.as_str() {
            "Const" => self.init_literal(node.prop_u64("value")),
            "VarGet" => var_name(node.prop("name").trim()),
            "BinOp" => {
                let a = bin("a")?;
                let b = bin("b")?;
                let (a, b) = (
                    format!("({})", self.u64_cast(&a)),
                    format!("({})", self.u64_cast(&b)),
                );
                match node.prop("op") {
                    "add" => format!("{a} + {b}"),
                    "sub" => format!("{a} - {b}"),
                    "mul" => format!("{a} * {b}"),
                    "div" => format!("({b} != 0 ? {a} / {b} : 0)"),
                    "and" => format!("{a} & {b}"),
                    "or" => format!("{a} | {b}"),
                    "xor" => format!("{a} ^ {b}"),
                    "shl" => format!("({b} < 64 ? {a} << {b} : 0)"),
                    "shr" => format!("({b} < 64 ? {a} >> {b} : 0)"),
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
                format!("(({a}) {op} ({b}) ? 1 : 0)")
            }
            kind if kind.starts_with("Mcu") => {
                return Err(format!("node {id} ({kind}) needs a microcontroller target"));
            }
            other if self.g.custom(other).is_some() => {
                return Err(format!(
                    "custom node '{}' has no source template for C/C++/C#",
                    other
                ));
            }
            other => return Err(format!("node {id} ({other}) has no data output")),
        })
    }

    /// C and C# need explicit conversions in mixed-width expressions; C++ can
    /// rely on the `u64` alias.
    fn u64_cast(&self, expr: &str) -> String {
        match self.lang {
            Target::CSharp => format!("(ulong)({expr})"),
            Target::CLang => format!("(uint64_t)({expr})"),
            _ => expr.to_string(),
        }
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
