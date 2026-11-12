use crate::json::Json;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    Aarch64Macos,
    Aarch64Linux,
    X86_64Linux,
    X86_64Macos,
    AvrAtmega328p,
    Stm32F411,
    Esp32,
    Esp8266,
    CLang,
    Cpp,
    CSharp,
}

impl Target {
    pub fn parse(s: &str) -> Option<Target> {
        match s.trim().to_ascii_lowercase().as_str() {
            "aarch64-macos" | "arm64-macos" | "arm64-darwin" => Some(Target::Aarch64Macos),
            "aarch64-linux" | "arm64-linux" => Some(Target::Aarch64Linux),
            "x86_64-linux" | "x86-64-linux" | "amd64-linux" => Some(Target::X86_64Linux),
            "x86_64-macos" | "x86-64-macos" | "amd64-macos" => Some(Target::X86_64Macos),
            "avr-atmega328p" | "avr" | "atmega328p" | "arduino-uno" | "arduino-nano" => {
                Some(Target::AvrAtmega328p)
            }
            "stm32f411" | "stm32" | "stm32f4" | "stm32f411ce" | "cortex-m4" | "blackpill" => {
                Some(Target::Stm32F411)
            }
            "esp32" | "esp-wroom-32" | "esp32-wroom" => Some(Target::Esp32),
            "esp8266" | "nodemcu" | "esp-12" => Some(Target::Esp8266),
            "c" | "c99" | "c11" => Some(Target::CLang),
            "cpp" | "c++" | "cxx" | "cplusplus" => Some(Target::Cpp),
            "csharp" | "c#" | "cs" | "dotnet" | "c-sharp" => Some(Target::CSharp),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Target::Aarch64Macos => "aarch64-macos",
            Target::Aarch64Linux => "aarch64-linux",
            Target::X86_64Linux => "x86_64-linux",
            Target::X86_64Macos => "x86_64-macos",
            Target::AvrAtmega328p => "avr-atmega328p",
            Target::Stm32F411 => "stm32f411",
            Target::Esp32 => "esp32",
            Target::Esp8266 => "esp8266",
            Target::CLang => "c",
            Target::Cpp => "cpp",
            Target::CSharp => "csharp",
        }
    }

    /// Name shown in the target picker.
    pub fn label(self) -> &'static str {
        match self {
            Target::Aarch64Macos => "AArch64 macOS",
            Target::Aarch64Linux => "AArch64 Linux",
            Target::X86_64Linux => "x86-64 Linux",
            Target::X86_64Macos => "x86-64 macOS",
            Target::AvrAtmega328p => "AVR ATmega328P (Uno/Nano)",
            Target::Stm32F411 => "STM32F411 (Cortex-M4)",
            Target::Esp32 => "ESP32 (Arduino core)",
            Target::Esp8266 => "ESP8266 (Arduino core)",
            Target::CLang => "C (host)",
            Target::Cpp => "C++ (host)",
            Target::CSharp => "C# (host)",
        }
    }

    /// Palette / documentation group for the target family.
    pub fn family(self) -> &'static str {
        match self {
            Target::Aarch64Macos
            | Target::Aarch64Linux
            | Target::X86_64Linux
            | Target::X86_64Macos => "host",
            Target::AvrAtmega328p => "avr",
            Target::Stm32F411 => "stm32",
            Target::Esp32 | Target::Esp8266 => "esp",
            Target::CLang | Target::Cpp | Target::CSharp => "source",
        }
    }

    pub fn is_aarch64(self) -> bool {
        matches!(self, Target::Aarch64Macos | Target::Aarch64Linux)
    }

    pub fn is_macos(self) -> bool {
        matches!(self, Target::Aarch64Macos | Target::X86_64Macos)
    }

    /// AVR microcontrollers (Arduino Uno/Nano, ATmega328P).
    pub fn is_avr(self) -> bool {
        matches!(self, Target::AvrAtmega328p)
    }

    /// STM32F411 (Cortex-M4) bare-metal assembly.
    pub fn is_stm32(self) -> bool {
        matches!(self, Target::Stm32F411)
    }

    /// ESP32 / ESP8266: code is generated as an Arduino sketch.
    pub fn is_esp(self) -> bool {
        matches!(self, Target::Esp32 | Target::Esp8266)
    }

    /// Arduino-compatible boards: AVR and ESP.
    pub fn is_arduino(self) -> bool {
        self.is_avr() || self.is_esp()
    }

    /// C, C++ or C# source targets.
    pub fn is_source(self) -> bool {
        matches!(self, Target::CLang | Target::Cpp | Target::CSharp)
    }

    /// Native host assembler targets.
    pub fn is_host_asm(self) -> bool {
        matches!(
            self,
            Target::Aarch64Macos | Target::Aarch64Linux | Target::X86_64Linux | Target::X86_64Macos
        )
    }

    /// Targets that emit assembler (host, AVR, STM32).
    pub fn is_asm(self) -> bool {
        self.is_host_asm() || self.is_avr() || self.is_stm32()
    }

    /// True for microcontroller targets: bare metal or Arduino runtime.
    pub fn is_mcu(self) -> bool {
        self.is_avr() || self.is_stm32() || self.is_esp()
    }

    /// Linux-style `_start` programs. macOS uses `_main`, MCUs use `main`.
    pub fn is_start(self) -> bool {
        matches!(self, Target::Aarch64Linux | Target::X86_64Linux)
    }

    /// Symbol name of the program entry point.
    pub fn entry_symbol(self) -> &'static str {
        if self.is_mcu() || self.is_source() {
            "main"
        } else if self.is_macos() {
            "_main"
        } else {
            "_start"
        }
    }

    /// File extension of the generated artifact before linking.
    pub fn source_extension(self) -> &'static str {
        match self {
            Target::CLang => "c",
            Target::Cpp => "cpp",
            Target::CSharp => "cs",
            Target::Esp32 | Target::Esp8266 => "ino",
            _ => "s",
        }
    }

    pub fn all() -> &'static [Target] {
        &[
            Target::Aarch64Macos,
            Target::Aarch64Linux,
            Target::X86_64Linux,
            Target::X86_64Macos,
            Target::AvrAtmega328p,
            Target::Stm32F411,
            Target::Esp32,
            Target::Esp8266,
            Target::CLang,
            Target::Cpp,
            Target::CSharp,
        ]
    }
}

#[derive(Clone, Debug, Default)]
pub struct Variable {
    pub name: String,
    pub init: u64,
}

#[derive(Clone, Debug)]
pub struct Node {
    pub id: String,
    pub kind: String,
    pub x: f64,
    pub y: f64,
    pub props: BTreeMap<String, String>,
}

impl Node {
    pub fn prop(&self, key: &str) -> &str {
        self.props.get(key).map(|s| s.as_str()).unwrap_or("")
    }

    pub fn prop_u64(&self, key: &str) -> u64 {
        self.props
            .get(key)
            .and_then(|s| crate::json::parse_u64(s).ok())
            .unwrap_or(0)
    }
}

#[derive(Clone, Debug)]
pub struct Edge {
    pub from_node: String,
    pub from_pin: String,
    pub to_node: String,
    pub to_pin: String,
}

/// A pin declaration. Built-in nodes use the table below; custom nodes carry
/// their own declarations in the graph file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinDef {
    pub name: String,
    pub ty: String,
}

impl PinDef {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        PinDef {
            name: name.into(),
            ty: ty.into(),
        }
    }
}

pub const TY_EXEC: &str = "exec";
pub const TY_U64: &str = "u64";

/// One pin of a custom node. Type is either `exec` or `u64`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomPin {
    pub name: String,
    pub ty: String,
}

impl CustomPin {
    pub fn new(name: impl Into<String>, ty: impl Into<String>) -> Self {
        CustomPin {
            name: name.into(),
            ty: ty.into(),
        }
    }
}

/// A user-defined node built from an assembler template.
///
/// * `inputs` / `outputs` describe the pins.
/// * `asm_aarch64` / `asm_x86_64` are templates. During code generation the
///   placeholders `{in0}`..`{in7}`, `{a}`, `{b}`, `{out}`, `{label}` and
///   `{prop:key}` are replaced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CustomNodeDef {
    pub kind: String,
    pub title: String,
    pub category: String,
    pub description: String,
    pub inputs: Vec<CustomPin>,
    pub outputs: Vec<CustomPin>,
    pub asm_aarch64: String,
    pub asm_x86_64: String,
}

impl CustomNodeDef {
    pub fn has_exec_input(&self) -> bool {
        self.inputs.iter().any(|p| p.ty == TY_EXEC)
    }

    pub fn has_exec_output(&self) -> bool {
        self.outputs.iter().any(|p| p.ty == TY_EXEC)
    }

    pub fn u64_inputs(&self) -> Vec<&CustomPin> {
        self.inputs.iter().filter(|p| p.ty == TY_U64).collect()
    }

    pub fn u64_output(&self) -> Option<&CustomPin> {
        self.outputs.iter().find(|p| p.ty == TY_U64)
    }

    pub fn from_json_value(v: &Json) -> Result<CustomNodeDef, String> {
        let kind = v
            .get("kind")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if kind.is_empty() {
            return Err("custom node without kind".to_string());
        }
        let title = v
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or(&kind)
            .to_string();
        let category = v
            .get("category")
            .and_then(|x| x.as_str())
            .unwrap_or("Custom")
            .to_string();
        let description = v
            .get("description")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();

        let mut def = CustomNodeDef {
            kind,
            title,
            category,
            description,
            ..CustomNodeDef::default()
        };
        def.inputs = parse_custom_pins(v.get("inputs"))?;
        def.outputs = parse_custom_pins(v.get("outputs"))?;

        if let Some(Json::Obj(asm)) = v.get("asm") {
            def.asm_aarch64 = asm
                .get("aarch64")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
            def.asm_x86_64 = asm
                .get("x86_64")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
        }
        // Accept flat keys too: easier for hand-written libraries.
        if def.asm_aarch64.is_empty() {
            def.asm_aarch64 = v
                .get("asmAarch64")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
        }
        if def.asm_x86_64.is_empty() {
            def.asm_x86_64 = v
                .get("asmX86_64")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string();
        }
        Ok(def)
    }

    pub fn to_json_value(&self) -> Json {
        let mut m = BTreeMap::new();
        m.insert("kind".into(), Json::Str(self.kind.clone()));
        m.insert("title".into(), Json::Str(self.title.clone()));
        m.insert("category".into(), Json::Str(self.category.clone()));
        if !self.description.is_empty() {
            m.insert("description".into(), Json::Str(self.description.clone()));
        }
        m.insert("inputs".into(), pins_to_json(&self.inputs));
        m.insert("outputs".into(), pins_to_json(&self.outputs));
        m.insert(
            "asm".into(),
            Json::Obj(BTreeMap::from([
                ("aarch64".into(), Json::Str(self.asm_aarch64.clone())),
                ("x86_64".into(), Json::Str(self.asm_x86_64.clone())),
            ])),
        );
        Json::Obj(m)
    }
}

fn parse_custom_pins(v: Option<&Json>) -> Result<Vec<CustomPin>, String> {
    let mut out = Vec::new();
    if let Some(Json::Arr(items)) = v {
        for item in items {
            let name = item
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .trim()
                .to_string();
            let ty = item
                .get("type")
                .or_else(|| item.get("ty"))
                .and_then(|x| x.as_str())
                .unwrap_or(TY_U64)
                .trim()
                .to_string();
            if name.is_empty() {
                return Err("custom pin without name".to_string());
            }
            out.push(CustomPin { name, ty });
        }
    }
    Ok(out)
}

fn pins_to_json(pins: &[CustomPin]) -> Json {
    Json::Arr(
        pins.iter()
            .map(|p| {
                Json::Obj(BTreeMap::from([
                    ("name".into(), Json::Str(p.name.clone())),
                    ("type".into(), Json::Str(p.ty.clone())),
                ]))
            })
            .collect(),
    )
}

/// A reusable collection of custom nodes (`*.wirelib.json`).
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub name: String,
    pub version: String,
    pub description: String,
    pub nodes: Vec<CustomNodeDef>,
}

impl Library {
    pub fn from_json_value(v: &Json) -> Result<Library, String> {
        let mut lib = Library {
            name: v
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("library")
                .to_string(),
            version: v
                .get("version")
                .map(|x| match x {
                    Json::Str(s) => s.clone(),
                    Json::Num(n) => format!("{n}"),
                    _ => "1".to_string(),
                })
                .unwrap_or_else(|| "1".to_string()),
            description: v
                .get("description")
                .and_then(|x| x.as_str())
                .unwrap_or("")
                .to_string(),
            ..Library::default()
        };
        let items = match v.get("nodes").or_else(|| v.get("customNodes")) {
            Some(Json::Arr(items)) => items.clone(),
            Some(other) => {
                return Err(format!(
                    "library 'nodes' must be an array, found {}",
                    other.stringify()
                ))
            }
            None => match v {
                Json::Arr(items) => items.clone(),
                _ => return Err("library has no 'nodes' array".to_string()),
            },
        };
        for item in &items {
            lib.nodes.push(CustomNodeDef::from_json_value(item)?);
        }
        if lib.nodes.is_empty() {
            return Err("library contains no custom nodes".to_string());
        }
        Ok(lib)
    }

    pub fn parse(src: &str) -> Result<Library, String> {
        let v = Json::parse(src)?;
        Library::from_json_value(&v)
    }

    pub fn to_json_value(&self) -> Json {
        Json::Obj(BTreeMap::from([
            ("format".into(), Json::Str("wireasm-library".into())),
            ("version".into(), Json::Str(self.version.clone())),
            ("name".into(), Json::Str(self.name.clone())),
            ("description".into(), Json::Str(self.description.clone())),
            (
                "nodes".into(),
                Json::Arr(self.nodes.iter().map(|n| n.to_json_value()).collect()),
            ),
        ]))
    }

    pub fn to_json(&self) -> String {
        self.to_json_value().stringify()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Graph {
    pub name: String,
    pub target: String,
    pub variables: Vec<Variable>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub custom_nodes: Vec<CustomNodeDef>,
}

/// Pins of a built-in node kind.
pub fn pins_for(kind: &str) -> Option<(Vec<PinDef>, Vec<PinDef>)> {
    let p = |name: &str, ty: &str| PinDef::new(name, ty);
    let inputs: Vec<PinDef>;
    let outputs: Vec<PinDef>;
    match kind {
        "Entry" => {
            inputs = vec![];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "Const" => {
            inputs = vec![];
            outputs = vec![p("value", TY_U64)];
        }
        "VarGet" => {
            inputs = vec![];
            outputs = vec![p("value", TY_U64)];
        }
        "VarSet" => {
            inputs = vec![p("exec", TY_EXEC), p("value", TY_U64)];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "BinOp" | "Cmp" => {
            inputs = vec![p("a", TY_U64), p("b", TY_U64)];
            outputs = vec![p("result", TY_U64)];
        }
        "Branch" => {
            inputs = vec![p("exec", TY_EXEC), p("cond", TY_U64)];
            outputs = vec![p("true", TY_EXEC), p("false", TY_EXEC)];
        }
        "PrintU64" => {
            inputs = vec![p("exec", TY_EXEC), p("value", TY_U64)];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "PrintStr" => {
            inputs = vec![p("exec", TY_EXEC)];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "Return" => {
            inputs = vec![p("exec", TY_EXEC), p("value", TY_U64)];
            outputs = vec![];
        }
        "RegGet" => {
            inputs = vec![];
            outputs = vec![p("value", TY_U64)];
        }
        "RegSet" => {
            inputs = vec![p("exec", TY_EXEC), p("value", TY_U64)];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "Call" | "Syscall" => {
            inputs = vec![
                p("exec", TY_EXEC),
                p("a0", TY_U64),
                p("a1", TY_U64),
                p("a2", TY_U64),
                p("a3", TY_U64),
            ];
            outputs = vec![p("exec", TY_EXEC), p("result", TY_U64)];
        }
        "RawAsm" => {
            inputs = vec![p("exec", TY_EXEC), p("a", TY_U64), p("b", TY_U64)];
            outputs = vec![p("exec", TY_EXEC), p("result", TY_U64)];
        }
        "Anchor" => {
            inputs = vec![p("exec", TY_EXEC)];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "Comment" => {
            inputs = vec![];
            outputs = vec![];
        }
        "McuPinMode" => {
            inputs = vec![p("exec", TY_EXEC)];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "McuDigitalWrite" => {
            inputs = vec![p("exec", TY_EXEC), p("value", TY_U64)];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "McuDigitalRead" => {
            inputs = vec![p("exec", TY_EXEC)];
            outputs = vec![p("exec", TY_EXEC), p("value", TY_U64)];
        }
        "McuDelayMs" | "McuDelayUs" | "McuUartWrite" | "McuRegisterWrite" => {
            inputs = vec![p("exec", TY_EXEC), p("value", TY_U64)];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "McuUartInit" | "McuUartWriteStr" => {
            inputs = vec![p("exec", TY_EXEC)];
            outputs = vec![p("exec", TY_EXEC)];
        }
        "McuAdcRead" | "McuRegisterRead" => {
            inputs = vec![p("exec", TY_EXEC)];
            outputs = vec![p("exec", TY_EXEC), p("value", TY_U64)];
        }
        _ => return None,
    }
    Some((inputs, outputs))
}

/// Every built-in node kind, in palette order.
pub const BUILTIN_KINDS: &[&str] = &[
    "Entry",
    "Branch",
    "Anchor",
    "Return",
    "Const",
    "VarGet",
    "VarSet",
    "BinOp",
    "Cmp",
    "PrintU64",
    "PrintStr",
    "Call",
    "Syscall",
    "RegGet",
    "RegSet",
    "RawAsm",
    "McuPinMode",
    "McuDigitalWrite",
    "McuDigitalRead",
    "McuDelayMs",
    "McuDelayUs",
    "McuUartInit",
    "McuUartWrite",
    "McuUartWriteStr",
    "McuAdcRead",
    "McuRegisterWrite",
    "McuRegisterRead",
    "Comment",
];

impl Graph {
    pub fn node(&self, id: &str) -> Option<&Node> {
        self.nodes.iter().find(|n| n.id == id)
    }

    pub fn node_mut(&mut self, id: &str) -> Option<&mut Node> {
        self.nodes.iter_mut().find(|n| n.id == id)
    }

    pub fn entry(&self) -> Option<&Node> {
        self.nodes.iter().find(|n| n.kind == "Entry")
    }

    pub fn custom(&self, kind: &str) -> Option<&CustomNodeDef> {
        self.custom_nodes.iter().find(|d| d.kind == kind)
    }

    /// Pins for any kind, built-in first, then custom definitions.
    pub fn pins(&self, kind: &str) -> Option<(Vec<PinDef>, Vec<PinDef>)> {
        if let Some(builtin) = pins_for(kind) {
            return Some(builtin);
        }
        let def = self.custom(kind)?;
        let to_pin = |p: &CustomPin| PinDef::new(p.name.clone(), p.ty.clone());
        Some((
            def.inputs.iter().map(to_pin).collect(),
            def.outputs.iter().map(to_pin).collect(),
        ))
    }

    pub fn inputs_of(&self, node: &Node) -> Vec<PinDef> {
        self.pins(&node.kind).map(|(i, _)| i).unwrap_or_default()
    }

    pub fn outputs_of(&self, node: &Node) -> Vec<PinDef> {
        self.pins(&node.kind).map(|(_, o)| o).unwrap_or_default()
    }

    /// Display title for a node kind.
    pub fn title_of(&self, kind: &str) -> String {
        self.custom(kind)
            .map(|d| {
                if d.title.is_empty() {
                    d.kind.clone()
                } else {
                    d.title.clone()
                }
            })
            .unwrap_or_else(|| node_title(kind).to_string())
    }

    /// Edge feeding a given input pin.
    pub fn incoming(&self, node_id: &str, pin: &str) -> Option<&Edge> {
        self.edges
            .iter()
            .find(|e| e.to_node == node_id && e.to_pin == pin)
    }

    /// All edges leaving an output pin.
    pub fn outgoing<'a>(
        &'a self,
        node_id: &'a str,
        pin: &'a str,
    ) -> impl Iterator<Item = &'a Edge> {
        self.edges
            .iter()
            .filter(move |e| e.from_node == node_id && e.from_pin == pin)
    }

    /// Exec successor for kinds with a single exec output.
    pub fn exec_next(&self, node_id: &str) -> Option<String> {
        self.outgoing(node_id, "exec")
            .next()
            .map(|e| e.to_node.clone())
    }

    /// Every variable name used by the graph, sorted and deduplicated.
    pub fn all_variable_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.variables.iter().map(|v| v.name.clone()).collect();
        for n in &self.nodes {
            if n.kind == "VarGet" || n.kind == "VarSet" {
                let name = n.prop("name").trim();
                if !name.is_empty() {
                    names.push(name.to_string());
                }
            }
        }
        names.sort();
        names.dedup();
        names
    }

    pub fn init_of(&self, name: &str) -> u64 {
        self.variables
            .iter()
            .find(|v| v.name == name)
            .map(|v| v.init)
            .unwrap_or(0)
    }

    /// Merge a parsed library into this graph. Nodes with the same `kind` are
    /// replaced; the return value is the number of added/replaced definitions.
    pub fn merge_library(&mut self, lib: &Library) -> usize {
        let mut changed = 0;
        for def in &lib.nodes {
            if let Some(slot) = self.custom_nodes.iter_mut().find(|d| d.kind == def.kind) {
                *slot = def.clone();
                changed += 1;
            } else {
                self.custom_nodes.push(def.clone());
                changed += 1;
            }
        }
        changed
    }

    /// A library containing all custom nodes of this graph.
    pub fn as_library(&self) -> Library {
        Library {
            name: self.name.clone(),
            version: "1".to_string(),
            description: format!("Custom nodes from {}", self.name),
            nodes: self.custom_nodes.clone(),
        }
    }

    pub fn from_json_value(v: &Json) -> Result<Graph, String> {
        let mut g = Graph {
            name: v
                .get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("program")
                .to_string(),
            target: v
                .get("target")
                .and_then(|x| x.as_str())
                .unwrap_or("aarch64-macos")
                .to_string(),
            ..Graph::default()
        };

        if let Some(Json::Arr(vars)) = v.get("variables") {
            for item in vars {
                let name = item
                    .get("name")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if name.is_empty() {
                    continue;
                }
                let init = item.get("init").and_then(|x| x.as_u64()).unwrap_or(0);
                g.variables.push(Variable { name, init });
            }
        }

        if let Some(Json::Arr(defs)) = v.get("customNodes") {
            for item in defs {
                g.custom_nodes.push(CustomNodeDef::from_json_value(item)?);
            }
        }

        let nodes = match v.get("nodes") {
            Some(Json::Arr(nodes)) => nodes,
            _ => return Err("graph has no 'nodes' array".to_string()),
        };
        for item in nodes {
            let id = item
                .get("id")
                .and_then(|x| x.as_str())
                .ok_or_else(|| "node without id".to_string())?
                .to_string();
            let kind = item
                .get("kind")
                .and_then(|x| x.as_str())
                .ok_or_else(|| format!("node {id} has no kind"))?
                .to_string();
            let x = item.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let y = item.get("y").and_then(|x| x.as_f64()).unwrap_or(0.0);
            let mut props = BTreeMap::new();
            if let Some(Json::Obj(m)) = item.get("props") {
                for (k, val) in m {
                    let s = match val {
                        Json::Str(s) => s.clone(),
                        Json::Num(n) => {
                            if n.fract() == 0.0 {
                                format!("{}", *n as i64)
                            } else {
                                format!("{n}")
                            }
                        }
                        Json::Bool(b) => b.to_string(),
                        Json::Null => String::new(),
                        other => other.stringify(),
                    };
                    props.insert(k.clone(), s);
                }
            }
            g.nodes.push(Node {
                id,
                kind,
                x,
                y,
                props,
            });
        }

        if let Some(Json::Arr(edges)) = v.get("edges") {
            for item in edges {
                let from_node = item.get("fromNode").and_then(|x| x.as_str()).unwrap_or("");
                let from_pin = item.get("fromPin").and_then(|x| x.as_str()).unwrap_or("");
                let to_node = item.get("toNode").and_then(|x| x.as_str()).unwrap_or("");
                let to_pin = item.get("toPin").and_then(|x| x.as_str()).unwrap_or("");
                if from_node.is_empty() || to_node.is_empty() {
                    continue;
                }
                g.edges.push(Edge {
                    from_node: from_node.to_string(),
                    from_pin: from_pin.to_string(),
                    to_node: to_node.to_string(),
                    to_pin: to_pin.to_string(),
                });
            }
        }
        Ok(g)
    }

    pub fn to_json_value(&self) -> Json {
        use crate::json::Json;
        let mut m = BTreeMap::new();
        m.insert("name".into(), Json::Str(self.name.clone()));
        m.insert("target".into(), Json::Str(self.target.clone()));
        m.insert(
            "variables".into(),
            Json::Arr(
                self.variables
                    .iter()
                    .map(|v| {
                        Json::Obj(BTreeMap::from([
                            ("name".into(), Json::Str(v.name.clone())),
                            ("init".into(), Json::Num(v.init as f64)),
                        ]))
                    })
                    .collect(),
            ),
        );
        m.insert(
            "customNodes".into(),
            Json::Arr(
                self.custom_nodes
                    .iter()
                    .map(|d| d.to_json_value())
                    .collect(),
            ),
        );
        m.insert(
            "nodes".into(),
            Json::Arr(
                self.nodes
                    .iter()
                    .map(|n| {
                        let props: BTreeMap<String, Json> = n
                            .props
                            .iter()
                            .map(|(k, v)| (k.clone(), Json::Str(v.clone())))
                            .collect();
                        Json::Obj(BTreeMap::from([
                            ("id".into(), Json::Str(n.id.clone())),
                            ("kind".into(), Json::Str(n.kind.clone())),
                            ("x".into(), Json::Num(n.x)),
                            ("y".into(), Json::Num(n.y)),
                            ("props".into(), Json::Obj(props)),
                        ]))
                    })
                    .collect(),
            ),
        );
        m.insert(
            "edges".into(),
            Json::Arr(
                self.edges
                    .iter()
                    .map(|e| {
                        Json::Obj(BTreeMap::from([
                            ("fromNode".into(), Json::Str(e.from_node.clone())),
                            ("fromPin".into(), Json::Str(e.from_pin.clone())),
                            ("toNode".into(), Json::Str(e.to_node.clone())),
                            ("toPin".into(), Json::Str(e.to_pin.clone())),
                        ]))
                    })
                    .collect(),
            ),
        );
        Json::Obj(m)
    }

    pub fn to_json(&self) -> String {
        self.to_json_value().stringify()
    }
}

pub fn node_title(kind: &str) -> &'static str {
    match kind {
        "Entry" => "Event Begin",
        "Const" => "Const",
        "VarGet" => "Get Var",
        "VarSet" => "Set Var",
        "BinOp" => "Binary Op",
        "Cmp" => "Compare",
        "Branch" => "Branch",
        "PrintU64" => "Print Number",
        "PrintStr" => "Print Text",
        "Return" => "Return",
        "RegGet" => "Read Register",
        "RegSet" => "Write Register",
        "Call" => "Call",
        "Syscall" => "Syscall",
        "RawAsm" => "Raw Assembly",
        "Anchor" => "Anchor",
        "Comment" => "Comment",
        "McuPinMode" => "Pin Mode",
        "McuDigitalWrite" => "Digital Write",
        "McuDigitalRead" => "Digital Read",
        "McuDelayMs" => "Delay ms",
        "McuDelayUs" => "Delay µs",
        "McuUartInit" => "UART Init",
        "McuUartWrite" => "UART Write Byte",
        "McuUartWriteStr" => "UART Write Text",
        "McuAdcRead" => "ADC Read",
        "McuRegisterWrite" => "Write Register",
        "McuRegisterRead" => "Read Register",
        _ => "Unknown",
    }
}

pub fn is_binary_op(op: &str) -> bool {
    matches!(
        op,
        "add" | "sub" | "mul" | "div" | "and" | "or" | "xor" | "shl" | "shr"
    )
}

pub fn is_cmp_op(op: &str) -> bool {
    matches!(op, "eq" | "ne" | "lt" | "le" | "gt" | "ge")
}
