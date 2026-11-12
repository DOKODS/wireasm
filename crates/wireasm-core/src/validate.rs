use crate::model::*;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

impl Severity {
    pub fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub node: Option<String>,
    pub severity: Severity,
    pub message: String,
}

impl Diagnostic {
    pub fn error(node: Option<&str>, message: impl Into<String>) -> Self {
        Diagnostic {
            node: node.map(|s| s.to_string()),
            severity: Severity::Error,
            message: message.into(),
        }
    }

    pub fn warning(node: Option<&str>, message: impl Into<String>) -> Self {
        Diagnostic {
            node: node.map(|s| s.to_string()),
            severity: Severity::Warning,
            message: message.into(),
        }
    }
}

pub fn validate(g: &Graph) -> Vec<Diagnostic> {
    let mut d = Vec::new();
    let mut kinds: HashMap<&str, &Node> = HashMap::new();

    for n in &g.nodes {
        if kinds.insert(n.id.as_str(), n).is_some() {
            d.push(Diagnostic::error(
                Some(&n.id),
                format!("duplicate node id '{}'", n.id),
            ));
        }
        if g.pins(&n.kind).is_none() {
            d.push(Diagnostic::error(
                Some(&n.id),
                format!(
                    "unknown node kind '{}' (add a customNodes definition or use a built-in kind)",
                    n.kind
                ),
            ));
        }
    }

    validate_custom_nodes(g, &mut d);

    let entries: Vec<&Node> = g.nodes.iter().filter(|n| n.kind == "Entry").collect();
    match entries.len() {
        0 => d.push(Diagnostic::error(
            None,
            "graph has no Entry node (Event Begin)",
        )),
        1 => {}
        _ => d.push(Diagnostic::error(
            Some(&entries[1].id),
            "graph has more than one Entry node",
        )),
    }

    let target = match Target::parse(&g.target) {
        Some(t) => Some(t),
        None => {
            d.push(Diagnostic::error(
                None,
                format!(
                    "unknown target '{}' (use aarch64-macos, aarch64-linux, x86_64-linux, x86_64-macos, avr-atmega328p, stm32f411, esp32, esp8266, c, cpp or csharp)",
                    g.target
                ),
            ));
            None
        }
    };
    let avr = target.map(|t| t.is_avr()).unwrap_or(false);
    let stm32 = target.map(|t| t.is_stm32()).unwrap_or(false);
    let esp = target.map(|t| t.is_esp()).unwrap_or(false);
    let source = target.map(|t| t.is_source()).unwrap_or(false);
    let mcu = target.map(|t| t.is_mcu()).unwrap_or(false);

    for e in &g.edges {
        let from = match kinds.get(e.from_node.as_str()) {
            Some(n) => *n,
            None => {
                d.push(Diagnostic::error(
                    None,
                    format!("edge references missing source node '{}'", e.from_node),
                ));
                continue;
            }
        };
        let to = match kinds.get(e.to_node.as_str()) {
            Some(n) => *n,
            None => {
                d.push(Diagnostic::error(
                    None,
                    format!("edge references missing target node '{}'", e.to_node),
                ));
                continue;
            }
        };
        if from.id == to.id {
            d.push(Diagnostic::error(
                Some(&from.id),
                "node is connected to itself",
            ));
            continue;
        }
        let outs = match g.pins(&from.kind) {
            Some((_, outs)) => outs,
            None => continue,
        };
        let ins = match g.pins(&to.kind) {
            Some((ins, _)) => ins,
            None => continue,
        };
        let op = outs.iter().find(|p| p.name == e.from_pin);
        let ip = ins.iter().find(|p| p.name == e.to_pin);
        let (op, ip) = match (op, ip) {
            (Some(o), Some(i)) => (o, i),
            _ => {
                d.push(Diagnostic::error(
                    Some(&from.id),
                    format!(
                        "edge {}:{} -> {}:{} uses a pin that does not exist",
                        from.id, e.from_pin, to.id, e.to_pin
                    ),
                ));
                continue;
            }
        };
        if op.ty != ip.ty {
            d.push(Diagnostic::error(
                Some(&from.id),
                format!(
                    "type mismatch: {} output '{}' ({}) -> {} input '{}' ({})",
                    from.id, op.name, op.ty, to.id, ip.name, ip.ty
                ),
            ));
        }
    }

    // Single connection rules.
    let mut input_use: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut exec_out_use: BTreeMap<(String, String), usize> = BTreeMap::new();
    for e in &g.edges {
        *input_use
            .entry((e.to_node.clone(), e.to_pin.clone()))
            .or_default() += 1;
        if e.from_pin == "exec" || e.from_pin == "true" || e.from_pin == "false" {
            *exec_out_use
                .entry((e.from_node.clone(), e.from_pin.clone()))
                .or_default() += 1;
        }
    }
    for ((node, pin), count) in &input_use {
        if pin == "exec" {
            continue;
        }
        if *count > 1 {
            d.push(Diagnostic::error(
                Some(node),
                format!("input '{pin}' has {count} connections (only one allowed)"),
            ));
        }
    }
    for ((node, pin), count) in &exec_out_use {
        if *count > 1 {
            d.push(Diagnostic::error(
                Some(node),
                format!("exec output '{pin}' has {count} connections (only one allowed)"),
            ));
        }
    }

    // Required pins and properties.
    for n in &g.nodes {
        let (ins, _) = match g.pins(&n.kind) {
            Some(p) => p,
            None => continue,
        };
        let required: &[&str] = match n.kind.as_str() {
            "Branch" => &["cond"],
            "BinOp" | "Cmp" => &["a", "b"],
            "PrintU64" => &["value"],
            "VarSet" | "RegSet" => &["value"],
            _ => &[],
        };
        for pin in required {
            if g.incoming(&n.id, pin).is_none() {
                d.push(Diagnostic::error(
                    Some(&n.id),
                    format!("input '{pin}' is not connected"),
                ));
            }
        }
        let _ = ins;

        match n.kind.as_str() {
            "Const" => {
                if n.prop("value").trim().is_empty() {
                    d.push(Diagnostic::error(
                        Some(&n.id),
                        "constant has no value (use decimal, 0x hex or 0b binary)",
                    ));
                } else if crate::json::parse_u64(n.prop("value")).is_err() {
                    d.push(Diagnostic::error(
                        Some(&n.id),
                        format!("constant '{}' is not a valid u64", n.prop("value")),
                    ));
                } else if avr && n.prop_u64("value") > 0xffff {
                    d.push(Diagnostic::warning(
                        Some(&n.id),
                        format!(
                            "value {} does not fit into the AVR 16-bit accumulator; it will be truncated to {}",
                            n.prop_u64("value"),
                            n.prop_u64("value") & 0xffff
                        ),
                    ));
                } else if (stm32 || esp) && n.prop_u64("value") > 0xffff_ffff {
                    d.push(Diagnostic::warning(
                        Some(&n.id),
                        format!(
                            "value {} does not fit into the 32-bit {target_word}; it will be truncated to {}",
                            n.prop_u64("value"),
                            n.prop_u64("value") & 0xffff_ffff,
                            target_word = if stm32 { "STM32 register" } else { "ESP value" }
                        ),
                    ));
                }
            }
            "VarGet" | "VarSet" => {
                if n.prop("name").trim().is_empty() {
                    d.push(Diagnostic::error(Some(&n.id), "variable name is empty"));
                }
            }
            "BinOp" => {
                if !is_binary_op(n.prop("op")) {
                    d.push(Diagnostic::error(
                        Some(&n.id),
                        format!(
                            "unknown binary op '{}' (add sub mul div and or xor shl shr)",
                            n.prop("op")
                        ),
                    ));
                }
            }
            "Cmp" => {
                if !is_cmp_op(n.prop("op")) {
                    d.push(Diagnostic::error(
                        Some(&n.id),
                        format!("unknown compare op '{}' (eq ne lt le gt ge)", n.prop("op")),
                    ));
                }
            }
            "RegGet" | "RegSet" => {
                if n.prop("reg").trim().is_empty() {
                    d.push(Diagnostic::error(Some(&n.id), "register name is empty"));
                } else if let Some(t) = target {
                    if !valid_register(t, n.prop("reg")) {
                        d.push(Diagnostic::warning(
                            Some(&n.id),
                            format!(
                                "register '{}' is not a known {} register",
                                n.prop("reg"),
                                t.as_str()
                            ),
                        ));
                    }
                }
            }
            "Call" => {
                if avr || stm32 || esp || source {
                    d.push(Diagnostic::error(
                        Some(&n.id),
                        "Call is not available on this target: there is no host ABI to call into",
                    ));
                } else if n.prop("func").trim().is_empty() {
                    d.push(Diagnostic::error(Some(&n.id), "function name is empty"));
                }
            }
            "Syscall" => {
                if mcu || source {
                    d.push(Diagnostic::error(
                        Some(&n.id),
                        "Syscall is not available on this target: it has no operating system interface",
                    ));
                } else if n.prop("number").trim().is_empty() {
                    d.push(Diagnostic::error(Some(&n.id), "syscall number is empty"));
                } else if crate::json::parse_u64(n.prop("number")).is_err() {
                    d.push(Diagnostic::error(
                        Some(&n.id),
                        "syscall number must be a number",
                    ));
                }
            }
            "RawAsm" if n.prop("text").trim().is_empty() => {
                d.push(Diagnostic::error(Some(&n.id), "raw assembly text is empty"));
            }
            kind if is_mcu_kind(kind) => {
                if !mcu {
                    d.push(Diagnostic::error(
                        Some(&n.id),
                        format!(
                            "'{kind}' is an MCU node: set the target to avr-atmega328p, stm32f411, esp32 or esp8266"
                        ),
                    ));
                }
                match kind {
                    "McuPinMode" => {
                        check_mcu_pin(target, &n.id, n, &mut d);
                        let mode = n.prop("mode").trim();
                        if !matches!(mode, "out" | "in" | "pullup") {
                            d.push(Diagnostic::error(
                                Some(&n.id),
                                format!("unknown pin mode '{mode}' (out, in, pullup)"),
                            ));
                        }
                    }
                    "McuDigitalWrite" | "McuDigitalRead" => {
                        check_mcu_pin(target, &n.id, n, &mut d);
                    }
                    "McuAdcRead" => {
                        let ch = n.prop_u64("channel");
                        let max = if avr {
                            7
                        } else if stm32 {
                            15
                        } else if target == Some(Target::Esp8266) {
                            1
                        } else {
                            39
                        };
                        if ch > max {
                            d.push(Diagnostic::error(
                                Some(&n.id),
                                format!(
                                    "ADC channel {ch} is out of range for {} (0..{max})",
                                    target.map(|t| t.as_str()).unwrap_or("this target")
                                ),
                            ));
                        }
                        if stm32 && ch > 15 {
                            d.push(Diagnostic::error(
                                Some(&n.id),
                                "STM32F411 ADC1 has channels 0..15",
                            ));
                        }
                    }
                    "McuUartInit" => {
                        let baud = n.prop_u64("baud");
                        if baud == 0 {
                            d.push(Diagnostic::error(
                                Some(&n.id),
                                "baud rate must be greater than zero",
                            ));
                        } else if avr && baud > 115_200 {
                            d.push(Diagnostic::warning(
                                Some(&n.id),
                                format!(
                                    "baud rate {baud} is above 115200; the AVR may not keep up"
                                ),
                            ));
                        } else if stm32 && baud > 1_000_000 {
                            d.push(Diagnostic::warning(
                                Some(&n.id),
                                format!("baud rate {baud} is too high for the 16 MHz HSI clock"),
                            ));
                        }
                    }
                    "McuRegisterWrite" | "McuRegisterRead" => {
                        if n.prop("address").trim().is_empty() {
                            d.push(Diagnostic::error(
                                Some(&n.id),
                                "data address is empty (use 0x25, 0x40023830, …)",
                            ));
                        } else if crate::json::parse_u64(n.prop("address"))
                            .map(|a| a > if avr { 0xffff } else { 0xffff_ffff })
                            .unwrap_or(true)
                        {
                            d.push(Diagnostic::error(
                                Some(&n.id),
                                format!(
                                    "'{}' is not a {} address",
                                    n.prop("address"),
                                    if avr { "16-bit data" } else { "32-bit" }
                                ),
                            ));
                        } else if avr && n.prop_u64("address") < 0x20 {
                            d.push(Diagnostic::error(
                                Some(&n.id),
                                format!(
                                    "address 0x{:02X} is below 0x20: lds/sts need data-space addresses (I/O 0x{:02X} maps to 0x{:02X})",
                                    n.prop_u64("address"),
                                    n.prop_u64("address"),
                                    n.prop_u64("address") + 0x20
                                ),
                            ));
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }

    // Data flow cycles (exec cycles are fine, data cycles are not).
    let mut data_edges: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for e in &g.edges {
        let is_exec = e.from_pin == "exec" || e.from_pin == "true" || e.from_pin == "false";
        if !is_exec {
            if let Some(f) = kinds.get(e.from_node.as_str()) {
                if let Some(t) = kinds.get(e.to_node.as_str()) {
                    if g.pins(&f.kind).is_some() && g.pins(&t.kind).is_some() {
                        data_edges
                            .entry(f.id.as_str())
                            .or_default()
                            .push(t.id.as_str());
                    }
                }
            }
        }
    }
    {
        let mut state: HashMap<&str, u8> = HashMap::new();
        let mut stack: Vec<&str> = Vec::new();
        for n in &g.nodes {
            if !state.contains_key(n.id.as_str()) {
                if let Some(cyc) = dfs_cycle(n.id.as_str(), &data_edges, &mut state, &mut stack) {
                    d.push(Diagnostic::error(
                        Some(cyc.0.as_str()),
                        format!("data cycle: {} -> ... -> {}", cyc.0, cyc.1),
                    ));
                }
            }
        }
    }

    // Exec loops must have an explicit Anchor so the generated label is stable.
    let exec_edges: BTreeMap<&str, Vec<&str>> = {
        let mut m: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for e in &g.edges {
            if e.from_pin == "exec" || e.from_pin == "true" || e.from_pin == "false" {
                if let (Some(f), Some(t)) = (
                    kinds.get(e.from_node.as_str()),
                    kinds.get(e.to_node.as_str()),
                ) {
                    m.entry(f.id.as_str()).or_default().push(t.id.as_str());
                }
            }
        }
        m
    };
    {
        let mut state: HashMap<&str, u8> = HashMap::new();
        let mut stack: Vec<&str> = Vec::new();
        let mut seen_cycles: BTreeSet<String> = BTreeSet::new();
        for n in &g.nodes {
            if !state.contains_key(n.id.as_str()) {
                if let Some((from, to)) =
                    dfs_cycle(n.id.as_str(), &exec_edges, &mut state, &mut stack)
                {
                    if let Some(target_node) = kinds.get(to.as_str()) {
                        if target_node.kind != "Anchor" {
                            let key = format!("{from}|{to}");
                            if seen_cycles.insert(key) {
                                d.push(Diagnostic::error(
                                    Some(to.as_str()),
                                    "exec loop must jump back to an Anchor node (add an Anchor as the loop head)",
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    // Reachability from Entry: exec edges forward, data edges backwards into producers.
    if let Some(entry) = g.entry() {
        let mut reach: HashSet<&str> = HashSet::new();
        let mut stack = vec![entry.id.as_str()];
        while let Some(id) = stack.pop() {
            if !reach.insert(id) {
                continue;
            }
            for e in &g.edges {
                if e.from_node == id {
                    let is_exec =
                        e.from_pin == "exec" || e.from_pin == "true" || e.from_pin == "false";
                    if is_exec {
                        stack.push(e.to_node.as_str());
                    }
                }
                if e.to_node == id {
                    stack.push(e.from_node.as_str());
                }
            }
        }
        for n in &g.nodes {
            if n.kind != "Comment" && !reach.contains(n.id.as_str()) {
                d.push(Diagnostic::warning(
                    Some(&n.id),
                    format!("node '{}' is not reachable from Entry", n.id),
                ));
            }
        }
    }

    if let Some(entry) = g.entry() {
        if g.exec_next(&entry.id).is_none() {
            d.push(Diagnostic::warning(
                Some(&entry.id),
                "Entry has no exec connection",
            ));
        }
    }

    d
}

fn validate_custom_nodes(g: &Graph, d: &mut Vec<Diagnostic>) {
    let target = Target::parse(&g.target);
    if let Some(t) = target {
        if !t.is_host_asm() {
            for def in &g.custom_nodes {
                d.push(Diagnostic::error(
                    None,
                    format!(
                        "custom node '{}': custom nodes only compile for host assembler targets, not {}",
                        def.kind,
                        t.as_str()
                    ),
                ));
            }
            return;
        }
    }
    let mut seen: BTreeSet<String> = BTreeSet::new();
    for def in &g.custom_nodes {
        let label = format!("custom node '{}'", def.kind);
        if def.kind.trim().is_empty() {
            d.push(Diagnostic::error(None, "custom node has an empty kind"));
            continue;
        }
        if !valid_custom_kind(&def.kind) {
            d.push(Diagnostic::error(
                None,
                format!(
                    "{label}: kind must start with a letter or '_' and contain only letters, digits, '_' or '-'"
                ),
            ));
        }
        if pins_for(&def.kind).is_some() {
            d.push(Diagnostic::error(
                None,
                format!("{label}: kind collides with a built-in node"),
            ));
        }
        if !seen.insert(def.kind.clone()) {
            d.push(Diagnostic::error(
                None,
                format!("{label}: duplicate custom node kind"),
            ));
        }
        if def.title.trim().is_empty() {
            d.push(Diagnostic::warning(
                None,
                format!("{label}: title is empty, the kind is used instead"),
            ));
        }

        let mut input_names: BTreeSet<&str> = BTreeSet::new();
        let mut exec_inputs = 0usize;
        let mut u64_inputs = 0usize;
        for p in &def.inputs {
            if p.name.trim().is_empty() {
                d.push(Diagnostic::error(
                    None,
                    format!("{label}: input pin without a name"),
                ));
                continue;
            }
            if !input_names.insert(p.name.as_str()) {
                d.push(Diagnostic::error(
                    None,
                    format!("{label}: duplicate input pin '{}'", p.name),
                ));
            }
            match p.ty.as_str() {
                TY_EXEC => {
                    exec_inputs += 1;
                    if p.name != "exec" {
                        d.push(Diagnostic::error(
                            None,
                            format!("{label}: the exec input pin must be named 'exec'"),
                        ));
                    }
                }
                TY_U64 => u64_inputs += 1,
                other => d.push(Diagnostic::error(
                    None,
                    format!("{label}: input '{}' has unknown type '{other}'", p.name),
                )),
            }
        }

        let mut output_names: BTreeSet<&str> = BTreeSet::new();
        let mut exec_outputs = 0usize;
        let mut u64_outputs = 0usize;
        for p in &def.outputs {
            if p.name.trim().is_empty() {
                d.push(Diagnostic::error(
                    None,
                    format!("{label}: output pin without a name"),
                ));
                continue;
            }
            if !output_names.insert(p.name.as_str()) {
                d.push(Diagnostic::error(
                    None,
                    format!("{label}: duplicate output pin '{}'", p.name),
                ));
            }
            match p.ty.as_str() {
                TY_EXEC => {
                    exec_outputs += 1;
                    if p.name != "exec" {
                        d.push(Diagnostic::error(
                            None,
                            format!("{label}: the exec output pin must be named 'exec'"),
                        ));
                    }
                }
                TY_U64 => u64_outputs += 1,
                other => d.push(Diagnostic::error(
                    None,
                    format!("{label}: output '{}' has unknown type '{other}'", p.name),
                )),
            }
        }

        if exec_inputs > 1 {
            d.push(Diagnostic::error(
                None,
                format!("{label}: at most one exec input is allowed"),
            ));
        }
        if exec_outputs > 1 {
            d.push(Diagnostic::error(
                None,
                format!("{label}: at most one exec output is allowed"),
            ));
        }
        if exec_outputs > 0 && exec_inputs == 0 {
            d.push(Diagnostic::error(
                None,
                format!("{label}: an exec output requires an exec input"),
            ));
        }
        if u64_outputs > 1 {
            d.push(Diagnostic::error(
                None,
                format!("{label}: version 0.4.0 supports at most one u64 output"),
            ));
        }
        if exec_inputs == 0 && u64_outputs == 0 {
            d.push(Diagnostic::warning(
                None,
                format!("{label}: node has neither an exec input nor a u64 output"),
            ));
        }
        if u64_inputs > 8 {
            d.push(Diagnostic::error(
                None,
                format!("{label}: at most 8 u64 inputs are supported"),
            ));
        }

        let (arch, text, other, other_arch) = match target {
            Some(t) if !t.is_aarch64() => ("x86_64", &def.asm_x86_64, &def.asm_aarch64, "aarch64"),
            _ => ("aarch64", &def.asm_aarch64, &def.asm_x86_64, "x86_64"),
        };
        if text.trim().is_empty() {
            d.push(Diagnostic::error(
                None,
                format!("{label}: the {arch} assembler template is empty"),
            ));
        }
        if other.trim().is_empty() {
            d.push(Diagnostic::warning(
                None,
                format!(
                    "{label}: the {other_arch} template is empty; this node will not compile for the other architecture"
                ),
            ));
        }
    }
}

fn valid_custom_kind(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_mcu_kind(kind: &str) -> bool {
    kind.starts_with("Mcu")
}

fn check_mcu_pin(target: Option<Target>, node: &str, n: &Node, d: &mut Vec<Diagnostic>) {
    let pin = n.prop_u64("pin");
    match target {
        Some(t) if t.is_stm32() => {
            let port = n.prop("port").trim().to_ascii_uppercase();
            if !port.is_empty()
                && !matches!(port.as_str(), "A" | "B" | "C" | "D" | "E" | "F" | "G" | "H")
            {
                d.push(Diagnostic::error(
                    Some(node),
                    format!("STM32 port '{port}' does not exist (use A..H)"),
                ));
            }
            if pin > 15 {
                d.push(Diagnostic::error(
                    Some(node),
                    format!("STM32 pin {pin} is out of range (0..15)"),
                ));
            }
        }
        Some(Target::Esp8266) => {
            if pin > 16 {
                d.push(Diagnostic::error(
                    Some(node),
                    format!("ESP8266 pin {pin} is out of range (0..16)"),
                ));
            }
        }
        Some(Target::Esp32) => {
            if pin > 39 {
                d.push(Diagnostic::error(
                    Some(node),
                    format!("ESP32 pin {pin} is out of range (0..39)"),
                ));
            }
        }
        _ => {
            if pin > 19 {
                d.push(Diagnostic::error(
                    Some(node),
                    format!("Arduino pin {pin} is out of range (0..19)"),
                ));
            }
        }
    }
}

fn valid_register(t: Target, reg: &str) -> bool {
    let r = reg.trim().trim_start_matches('%');
    if t.is_avr() {
        return r
            .strip_prefix('r')
            .and_then(|n| n.parse::<u8>().ok())
            .map(|n| n <= 31)
            .unwrap_or(false);
    }
    if t.is_stm32() {
        let aliases = ["sp", "lr", "pc"];
        if aliases.contains(&r) {
            return true;
        }
        if let Some(num) = r.strip_prefix('r') {
            if let Ok(n) = num.parse::<u8>() {
                return n <= 12;
            }
        }
        return false;
    }
    if t.is_source() {
        return false;
    }
    if t.is_aarch64() {
        let aliases = ["sp", "fp", "lr", "xzr", "wzr"];
        if aliases.contains(&r) {
            return true;
        }
        if let Some(num) = r.strip_prefix('x').or_else(|| r.strip_prefix('w')) {
            return num.parse::<u8>().map(|n| n <= 30).unwrap_or(false);
        }
        false
    } else {
        if r == "rip" {
            return true;
        }
        let names64 = [
            "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "rbp", "rsp", "r8", "r9", "r10", "r11",
            "r12", "r13", "r14", "r15",
        ];
        if names64.contains(&r) {
            return true;
        }
        let names32 = [
            "eax", "ebx", "ecx", "edx", "esi", "edi", "ebp", "esp", "r8d", "r9d", "r10d", "r11d",
            "r12d", "r13d", "r14d", "r15d",
        ];
        names32.contains(&r)
    }
}

fn dfs_cycle<'a>(
    start: &'a str,
    adj: &BTreeMap<&'a str, Vec<&'a str>>,
    state: &mut HashMap<&'a str, u8>,
    stack: &mut Vec<&'a str>,
) -> Option<(String, String)> {
    state.insert(start, 1);
    stack.push(start);
    if let Some(nexts) = adj.get(start) {
        for &next in nexts {
            match state.get(next).copied().unwrap_or(0) {
                0 => {
                    if let Some(c) = dfs_cycle(next, adj, state, stack) {
                        return Some(c);
                    }
                }
                1 => {
                    return Some((start.to_string(), next.to_string()));
                }
                _ => {}
            }
        }
    }
    state.insert(start, 2);
    stack.pop();
    None
}
