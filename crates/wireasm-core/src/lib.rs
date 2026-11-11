//! WireASM core: a Blueprint-style graph language that compiles to real
//! AArch64 / x86-64 assembler.
//!
//! The public surface is deliberately small:
//!   * parse a graph from JSON,
//!   * validate it,
//!   * generate assembler for one of four targets.

pub mod codegen;
pub mod json;
pub mod model;
pub mod toolchain;
pub mod validate;

pub use codegen::{compile, CompileOutput, CompileStats, VERSION};
pub use model::{
    CustomNodeDef, CustomPin, Graph, Library, Node, PinDef, Target, Variable, TY_EXEC, TY_U64,
};
pub use validate::{Diagnostic, Severity};

/// Parse and compile a graph in one step.
pub fn compile_json(src: &str) -> Result<CompileOutput, String> {
    let v = json::Json::parse(src)?;
    let g = Graph::from_json_value(&v)?;
    Ok(compile(&g))
}

/// A filesystem-safe stem for generated artifacts.
pub fn artifact_stem(g: &Graph) -> String {
    let mut s = String::new();
    for c in g.name.chars() {
        if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
            s.push(c);
        } else if c == ' ' || c == '.' {
            s.push('_');
        }
    }
    let s = s.trim_matches('_').to_string();
    if s.is_empty() {
        "program".to_string()
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph(nodes: &[(&str, &str)], edges: &[(&str, &str, &str, &str)]) -> Graph {
        let mut g = Graph {
            name: "test".to_string(),
            target: "aarch64-macos".to_string(),
            ..Graph::default()
        };
        for (id, kind) in nodes {
            g.nodes.push(Node {
                id: (*id).into(),
                kind: (*kind).into(),
                x: 0.0,
                y: 0.0,
                props: Default::default(),
            });
        }
        for (a, ap, b, bp) in edges {
            g.edges.push(model::Edge {
                from_node: (*a).into(),
                from_pin: (*ap).into(),
                to_node: (*b).into(),
                to_pin: (*bp).into(),
            });
        }
        g
    }

    #[test]
    fn minimal_graph_compiles_to_arm64() {
        let g = graph(
            &[("e", "Entry"), ("r", "Return")],
            &[("e", "exec", "r", "exec")],
        );
        let out = compile(&g);
        assert!(out.ok, "{:?}", out.diagnostics);
        assert!(out.asm.contains("_main:"));
        assert!(out.asm.contains("ret"));
    }

    #[test]
    fn missing_cond_is_an_error() {
        let g = graph(
            &[("e", "Entry"), ("b", "Branch")],
            &[("e", "exec", "b", "exec")],
        );
        let out = compile(&g);
        assert!(!out.ok);
        assert!(out.diagnostics.iter().any(|d| d.message.contains("'cond'")));
    }

    #[test]
    fn type_mismatch_is_reported() {
        let g = graph(
            &[("e", "Entry"), ("c", "Const"), ("p", "PrintU64")],
            &[("e", "exec", "p", "exec"), ("c", "value", "p", "exec")],
        );
        let out = compile(&g);
        assert!(!out.ok);
        assert!(out
            .diagnostics
            .iter()
            .any(|d| d.message.contains("type mismatch")));
    }

    #[test]
    fn compile_json_rejects_broken_json() {
        assert!(compile_json("{ nope").is_err());
    }

    #[test]
    fn call_to_print_u64_emits_runtime() {
        let mut g = graph(
            &[
                ("e", "Entry"),
                ("c", "Const"),
                ("call", "Call"),
                ("r", "Return"),
            ],
            &[
                ("e", "exec", "call", "exec"),
                ("c", "value", "call", "a0"),
                ("call", "exec", "r", "exec"),
            ],
        );
        g.nodes[2].props.insert("func".into(), "print_u64".into());
        g.nodes[1].props.insert("value".into(), "42".into());
        let out = compile(&g);
        assert!(out.ok, "{:?}", out.diagnostics);
        assert!(out.asm.contains("print_u64:"));
        assert!(out.asm.contains("bl print_u64"));
    }

    #[test]
    fn loop_needs_anchor() {
        let g = graph(
            &[("e", "Entry"), ("b", "Branch"), ("r", "Return")],
            &[
                ("e", "exec", "b", "exec"),
                ("b", "false", "r", "exec"),
                ("b", "true", "b", "exec"),
            ],
        );
        let out = compile(&g);
        assert!(out.diagnostics.iter().any(|d| d.message.contains("Anchor")));
    }

    #[test]
    fn custom_flow_node_expands_template() {
        let mut g = graph(
            &[
                ("e", "Entry"),
                ("c", "Const"),
                ("n", "my_add1"),
                ("p", "PrintU64"),
                ("r", "Return"),
            ],
            &[
                ("e", "exec", "n", "exec"),
                ("n", "exec", "p", "exec"),
                ("p", "exec", "r", "exec"),
                ("c", "value", "n", "value"),
                ("n", "result", "p", "value"),
            ],
        );
        g.nodes[1].props.insert("value".into(), "41".into());
        g.custom_nodes.push(CustomNodeDef {
            kind: "my_add1".into(),
            title: "Add one".into(),
            inputs: vec![
                CustomPin::new("exec", TY_EXEC),
                CustomPin::new("value", TY_U64),
            ],
            outputs: vec![
                CustomPin::new("exec", TY_EXEC),
                CustomPin::new("result", TY_U64),
            ],
            asm_aarch64: "add {out}, {in0}, #1".into(),
            asm_x86_64: "add {out}, 1".into(),
            ..CustomNodeDef::default()
        });
        let out = compile(&g);
        assert!(out.ok, "{:?}", out.diagnostics);
        assert!(out.asm.contains("add x0, x0, #1"), "{}", out.asm);

        g.target = "x86_64-linux".into();
        let out = compile(&g);
        assert!(out.ok, "{:?}", out.diagnostics);
        assert!(out.asm.contains("add rax, 1"), "{}", out.asm);
    }

    #[test]
    fn custom_data_node_is_inlined() {
        let mut g = graph(
            &[
                ("e", "Entry"),
                ("c", "Const"),
                ("d", "double"),
                ("p", "PrintU64"),
                ("r", "Return"),
            ],
            &[
                ("e", "exec", "p", "exec"),
                ("p", "exec", "r", "exec"),
                ("c", "value", "d", "value"),
                ("d", "result", "p", "value"),
            ],
        );
        g.nodes[1].props.insert("value".into(), "21".into());
        g.custom_nodes.push(CustomNodeDef {
            kind: "double".into(),
            title: "Double".into(),
            inputs: vec![CustomPin::new("value", TY_U64)],
            outputs: vec![CustomPin::new("result", TY_U64)],
            asm_aarch64: "lsl {out}, {in0}, #1".into(),
            asm_x86_64: "mov rax, rdi\nshl rax, 1".into(),
            ..CustomNodeDef::default()
        });
        let out = compile(&g);
        assert!(out.ok, "{:?}", out.diagnostics);
        assert!(out.asm.contains("lsl x0, x0, #1"), "{}", out.asm);

        g.target = "x86_64-linux".into();
        let out = compile(&g);
        assert!(out.ok, "{:?}", out.diagnostics);
        assert!(out.asm.contains("shl rax, 1"), "{}", out.asm);
    }

    #[test]
    fn avr_blink_compiles_to_atmega328p_asm() {
        let mut g = graph(
            &[
                ("e", "Entry"),
                ("m", "McuPinMode"),
                ("w", "McuDigitalWrite"),
                ("c1", "Const"),
                ("d", "McuDelayMs"),
                ("c2", "Const"),
                ("r", "Return"),
            ],
            &[
                ("e", "exec", "m", "exec"),
                ("m", "exec", "w", "exec"),
                ("c1", "value", "w", "value"),
                ("w", "exec", "d", "exec"),
                ("c2", "value", "d", "value"),
                ("d", "exec", "r", "exec"),
            ],
        );
        g.target = "avr-atmega328p".into();
        g.nodes[1].props.insert("pin".into(), "13".into());
        g.nodes[1].props.insert("mode".into(), "out".into());
        g.nodes[2].props.insert("pin".into(), "13".into());
        g.nodes[3].props.insert("value".into(), "1".into());
        g.nodes[5].props.insert("value".into(), "250".into());

        let out = compile(&g);
        assert!(out.ok, "{:?}", out.diagnostics);
        assert!(out.asm.contains(".global main"), "{}", out.asm);
        assert!(out.asm.contains("sbi 0x04, 5"), "{}", out.asm); // DDRB, bit 5
        assert!(out.asm.contains("sbi 0x05, 5"), "{}", out.asm); // PORTB, bit 5
        assert!(out.asm.contains("call delay_ms"), "{}", out.asm);
        assert!(!out.asm.contains("//"), "AVR comments must use ';'");
        assert!(out.asm.contains(".section .bss"), "{}", out.asm);
    }

    #[test]
    fn avr_rejects_host_nodes() {
        let mut g = graph(
            &[("e", "Entry"), ("c", "Call"), ("r", "Return")],
            &[("e", "exec", "c", "exec"), ("c", "exec", "r", "exec")],
        );
        g.target = "avr-atmega328p".into();
        g.nodes[1].props.insert("func".into(), "print_u64".into());
        let out = compile(&g);
        assert!(!out.ok);
        assert!(out
            .diagnostics
            .iter()
            .any(|d| d.message.contains("not available on AVR")));
    }

    #[test]
    fn avr_validates_pins_and_uart() {
        let mut g = graph(
            &[
                ("e", "Entry"),
                ("m", "McuPinMode"),
                ("u", "McuUartInit"),
                ("r", "Return"),
            ],
            &[
                ("e", "exec", "m", "exec"),
                ("m", "exec", "u", "exec"),
                ("u", "exec", "r", "exec"),
            ],
        );
        g.target = "avr-atmega328p".into();
        g.nodes[1].props.insert("pin".into(), "42".into());
        g.nodes[1].props.insert("mode".into(), "out".into());
        g.nodes[2].props.insert("baud".into(), "9600".into());
        let out = compile(&g);
        assert!(!out.ok);
        assert!(out
            .diagnostics
            .iter()
            .any(|d| d.message.contains("out of range (0..19)")));
    }

    #[test]
    fn avr_uart_text_lands_in_data_section() {
        let mut g = graph(
            &[("e", "Entry"), ("s", "McuUartWriteStr"), ("r", "Return")],
            &[("e", "exec", "s", "exec"), ("s", "exec", "r", "exec")],
        );
        g.target = "avr-atmega328p".into();
        g.nodes[1].props.insert("text".into(), "hi\\n".into());
        let out = compile(&g);
        assert!(out.ok, "{:?}", out.diagnostics);
        assert!(out.asm.contains(".section .data"), "{}", out.asm);
        assert!(out.asm.contains("call uart_write_str"), "{}", out.asm);
        assert!(out.asm.contains("ldi r24, 103"), "{}", out.asm); // 9600 baud auto-init (UBRR)
    }

    #[test]
    fn library_parses_and_merges() {
        let src = r#"{
          "format": "wireasm-library",
          "name": "Test",
          "nodes": [
            {
              "kind": "lib_add",
              "title": "Library add",
              "inputs": [{"name": "a", "type": "u64"}, {"name": "b", "type": "u64"}],
              "outputs": [{"name": "result", "type": "u64"}],
              "asm": {
                "aarch64": "add {out}, {a}, {b}",
                "x86_64": "mov rax, rdi\\nadd rax, rsi"
              }
            }
          ]
        }"#;
        let lib = Library::parse(src).unwrap();
        assert_eq!(lib.nodes.len(), 1);
        let mut g = Graph::default();
        assert_eq!(g.merge_library(&lib), 1);
        assert!(g.custom("lib_add").is_some());
    }
}
