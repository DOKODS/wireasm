//! Schematic model: placed parts, wires, nets and the netlist they describe.
//!
//! The model is independent of the editor. The IDE stores it in a document,
//! the `wiresim` command-line tool reads the same JSON, and both turn it into
//! a [`crate::Circuit`] for the solver.

use std::collections::{BTreeMap, BTreeSet};

use wireasm_core::json::Json;

use crate::netlist::{
    Circuit, Component, ComponentKind, DiodeModel, Gate, GateKind, LedModel, Source, Waveform,
};
use crate::solver::{McuBinding, Options};
use crate::trace::{AcSweep, Trace};

/// Every part a schematic can hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    Resistor,
    Capacitor,
    Inductor,
    Vsource,
    Isource,
    Diode,
    Led,
    Switch,
    Gate,
    McuPin,
    Ground,
}

pub const PART_KINDS: &[PartKind] = &[
    PartKind::Resistor,
    PartKind::Capacitor,
    PartKind::Inductor,
    PartKind::Vsource,
    PartKind::Isource,
    PartKind::Diode,
    PartKind::Led,
    PartKind::Switch,
    PartKind::Gate,
    PartKind::McuPin,
    PartKind::Ground,
];

impl PartKind {
    pub fn key(self) -> &'static str {
        match self {
            PartKind::Resistor => "resistor",
            PartKind::Capacitor => "capacitor",
            PartKind::Inductor => "inductor",
            PartKind::Vsource => "vsource",
            PartKind::Isource => "isource",
            PartKind::Diode => "diode",
            PartKind::Led => "led",
            PartKind::Switch => "switch",
            PartKind::Gate => "gate",
            PartKind::McuPin => "mcu_pin",
            PartKind::Ground => "ground",
        }
    }

    pub fn from_key(key: &str) -> Option<PartKind> {
        PART_KINDS
            .iter()
            .copied()
            .find(|k| k.key() == key.trim().to_ascii_lowercase())
    }

    /// Reference designator prefix.
    pub fn prefix(self) -> &'static str {
        match self {
            PartKind::Resistor => "R",
            PartKind::Capacitor => "C",
            PartKind::Inductor => "L",
            PartKind::Vsource => "V",
            PartKind::Isource => "I",
            PartKind::Diode => "D",
            PartKind::Led => "LED",
            PartKind::Switch => "SW",
            PartKind::Gate => "U",
            PartKind::McuPin => "P",
            PartKind::Ground => "GND",
        }
    }

    /// Pin names of the part. The gate drops `b` when it is an inverter.
    pub fn pins(self) -> &'static [&'static str] {
        match self {
            PartKind::Gate => &["a", "b", "y"],
            PartKind::McuPin => &["p"],
            PartKind::Ground => &["0"],
            _ => &["1", "2"],
        }
    }
}

/// Source waveform.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaveKind {
    Dc,
    Sine,
    Pulse,
}

impl WaveKind {
    pub fn key(self) -> &'static str {
        match self {
            WaveKind::Dc => "dc",
            WaveKind::Sine => "sine",
            WaveKind::Pulse => "pulse",
        }
    }

    pub fn from_key(key: &str) -> Option<WaveKind> {
        match key.trim().to_ascii_lowercase().as_str() {
            "dc" | "" => Some(WaveKind::Dc),
            "sine" | "sin" => Some(WaveKind::Sine),
            "pulse" | "square" => Some(WaveKind::Pulse),
            _ => None,
        }
    }
}

/// One placed part.
#[derive(Clone, Debug)]
pub struct Part {
    pub id: String,
    pub kind: PartKind,
    pub x: f32,
    pub y: f32,
    /// Primary value: ohms, farads, henries, volts, amperes, gate Vdd.
    pub value: f64,
    /// Secondary value: LED rated current.
    pub value2: f64,
    /// Switch state.
    pub on: bool,
    /// Gate type.
    pub gate: GateKind,
    /// Name of the virtual MCU pin this part drives (`C13`, `13`, …).
    pub pin_name: String,
    /// Source waveform.
    pub wave: WaveKind,
    /// Waveform amplitude.
    pub amp: f64,
    /// Waveform frequency, Hz.
    pub freq: f64,
    /// Pulse duty cycle.
    pub duty: f64,
    /// Catalogue key when the part comes from the component library,
    /// e.g. `7400`.
    pub catalog: Option<String>,
    /// Units of a catalogue part: what each gate or flip-flop does.
    pub catalog_units: Vec<crate::library::Unit>,
    /// Supply of a catalogue part.
    pub catalog_power: Option<crate::library::Power>,
}

impl Part {
    pub fn new(id: String, kind: PartKind, x: f32, y: f32) -> Part {
        let mut p = Part {
            id,
            kind,
            x,
            y,
            value: 0.0,
            value2: 0.0,
            on: false,
            gate: GateKind::And,
            pin_name: String::new(),
            wave: WaveKind::Dc,
            amp: 1.0,
            freq: 1000.0,
            duty: 0.5,
            catalog: None,
            catalog_units: Vec::new(),
            catalog_power: None,
        };
        match kind {
            PartKind::Resistor => p.value = 1000.0,
            PartKind::Capacitor => p.value = 1e-6,
            PartKind::Inductor => p.value = 1e-3,
            PartKind::Vsource => {
                p.value = 5.0;
                p.amp = 5.0;
            }
            PartKind::Isource => p.value = 1e-3,
            PartKind::Led => {
                p.value = 2.0;
                p.value2 = 0.02;
            }
            PartKind::Gate => p.value = 5.0,
            PartKind::McuPin => p.pin_name = "C13".to_string(),
            _ => {}
        }
        p
    }

    /// Pin names of this part, taking the gate type into account.
    ///
    /// A catalogue part names its pins after the signals of its units, the
    /// way the datasheet does.
    pub fn pins(&self) -> Vec<String> {
        if !self.catalog_units.is_empty() {
            let mut out: Vec<String> = Vec::new();
            for unit in &self.catalog_units {
                for name in &unit.ins {
                    if !out.contains(name) {
                        out.push(name.clone());
                    }
                }
            }
            for unit in &self.catalog_units {
                for name in &unit.outs {
                    if !out.contains(name) {
                        out.push(name.clone());
                    }
                }
            }
            if let Some(power) = &self.catalog_power {
                out.push(power.vcc.clone());
                out.push(power.gnd.clone());
            }
            return out;
        }
        match self.kind {
            PartKind::Gate => {
                if self.gate.inputs() == 1 {
                    vec!["a".to_string(), "y".to_string()]
                } else {
                    vec!["a".to_string(), "b".to_string(), "y".to_string()]
                }
            }
            _ => self.kind.pins().iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Whether the part comes from the component library.
    pub fn is_catalog(&self) -> bool {
        !self.catalog_units.is_empty()
    }

    /// The catalogue entry the part was placed from.
    pub fn catalog_entry(&self) -> Option<crate::library::Entry> {
        let key = self.catalog.as_ref()?;
        crate::library::builtin()
            .entries
            .into_iter()
            .find(|e| &e.key == key)
    }

    /// Short caption drawn inside the body.
    pub fn caption(&self) -> String {
        match self.kind {
            PartKind::Resistor => format!("{} Ω", fmt_value(self.value)),
            PartKind::Capacitor => format!("{} F", fmt_value(self.value)),
            PartKind::Inductor => format!("{} H", fmt_value(self.value)),
            PartKind::Vsource => match self.wave {
                WaveKind::Dc => format!("{} V", fmt_value(self.value)),
                _ => format!("{} V {}", fmt_value(self.amp), fmt_value(self.freq)),
            },
            PartKind::Isource => format!("{} A", fmt_value(self.value)),
            PartKind::Diode => "D".to_string(),
            PartKind::Led => format!("{} V", fmt_value(self.value)),
            PartKind::Switch => {
                if self.on {
                    "closed".to_string()
                } else {
                    "open".to_string()
                }
            }
            PartKind::Gate => format!("{} {}", self.id, gate_short(self.gate)),
            PartKind::McuPin => self.pin_name.clone(),
            PartKind::Ground => "GND".to_string(),
        }
    }

    fn source(&self) -> Source {
        let mut src = Source {
            dc: self.value,
            ac: 0.0,
            waveform: None,
        };
        match self.wave {
            WaveKind::Dc => {}
            WaveKind::Sine => {
                src.waveform = Some(Waveform::Sine {
                    amplitude: self.amp,
                    frequency: self.freq,
                    phase: 0.0,
                    offset: self.value,
                });
            }
            WaveKind::Pulse => {
                let period = if self.freq > 0.0 {
                    1.0 / self.freq
                } else {
                    1e-3
                };
                src.waveform = Some(Waveform::Pulse {
                    low: self.value,
                    high: self.amp,
                    period,
                    duty: self.duty,
                    rise: 0.0,
                    fall: 0.0,
                });
            }
        }
        src
    }
}

/// Short name of a gate type.
pub fn gate_short(kind: GateKind) -> &'static str {
    match kind {
        GateKind::And => "AND",
        GateKind::Or => "OR",
        GateKind::Nand => "NAND",
        GateKind::Nor => "NOR",
        GateKind::Xor => "XOR",
        GateKind::Xnor => "XNOR",
        GateKind::Not => "NOT",
        GateKind::Buffer => "BUF",
    }
}

/// A pin of a placed part.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinAddr {
    pub part: usize,
    pub pin: String,
}

impl PinAddr {
    pub fn new(part: usize, pin: &str) -> PinAddr {
        PinAddr {
            part,
            pin: pin.to_string(),
        }
    }
}

/// A wire between two pins.
#[derive(Clone, Debug)]
pub struct Wire {
    pub a: PinAddr,
    pub b: PinAddr,
}

/// A measurement point on a pin.
#[derive(Clone, Debug)]
pub struct CircuitProbe {
    pub part: usize,
    pub pin: String,
    pub name: String,
}

/// Which analysis the scope runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Analysis {
    Dc,
    Tran,
    Ac,
}

impl Analysis {
    pub fn key(self) -> &'static str {
        match self {
            Analysis::Dc => "dc",
            Analysis::Tran => "tran",
            Analysis::Ac => "ac",
        }
    }

    pub fn from_key(key: &str) -> Analysis {
        match key.trim().to_ascii_lowercase().as_str() {
            "tran" => Analysis::Tran,
            "ac" => Analysis::Ac,
            _ => Analysis::Dc,
        }
    }
}

/// Result of the last simulation.
#[derive(Clone, Debug)]
pub enum SimResult {
    Dc(crate::solver::Solution),
    Tran(Trace),
    Ac(AcSweep),
}

/// A schematic: parts, wires and the settings of the simulation.
#[derive(Clone, Debug)]
pub struct Schematic {
    pub name: String,
    pub description: String,
    pub parts: Vec<Part>,
    pub wires: Vec<Wire>,
    pub probes: Vec<CircuitProbe>,
    pub analysis: Analysis,
    pub opts: Options,
    pub f_start: f64,
    pub f_stop: f64,
    pub ac_points: usize,
    pub mcu_on: bool,
    pub mcu_vdd: f64,
}

impl Default for Schematic {
    fn default() -> Self {
        Schematic::divider()
    }
}

impl Schematic {
    pub fn empty() -> Schematic {
        Schematic {
            name: "circuit".to_string(),
            description: String::new(),
            parts: Vec::new(),
            wires: Vec::new(),
            probes: Vec::new(),
            analysis: Analysis::Dc,
            opts: Options {
                dt: 1e-4,
                t_stop: 2e-3,
                ..Options::default()
            },
            f_start: 10.0,
            f_stop: 100_000.0,
            ac_points: 40,
            mcu_on: false,
            mcu_vdd: 3.3,
        }
    }

    /// 12 V across 10k and 4.7k — the classic first schematic.
    pub fn divider() -> Schematic {
        let mut s = Schematic::empty();
        s.name = "divider".to_string();
        s.description = "12 V across 10k and 4.7k: out is 3.837 V".to_string();
        let v = s.add_part(PartKind::Vsource, -190.0, 0.0);
        s.parts[v].value = 12.0;
        let r1 = s.add_part(PartKind::Resistor, -60.0, 0.0);
        s.parts[r1].value = 10_000.0;
        let r2 = s.add_part(PartKind::Resistor, 70.0, 0.0);
        s.parts[r2].value = 4700.0;
        let gnd = s.add_part(PartKind::Ground, 70.0, 90.0);
        s.wire(v, "1", r1, "1");
        s.wire(r1, "2", r2, "1");
        s.wire(r2, "2", gnd, "0");
        s.wire(v, "2", gnd, "0");
        s.probe(r2, "1", "out");
        s
    }

    /// 1k + 1uF low-pass driven by a 1 kHz sine.
    pub fn rc_lowpass() -> Schematic {
        let mut s = Schematic::empty();
        s.name = "rc_lowpass".to_string();
        s.description = "1k + 1uF low-pass driven by a 1 kHz sine".to_string();
        let v = s.add_part(PartKind::Vsource, -190.0, 0.0);
        s.parts[v].wave = WaveKind::Sine;
        s.parts[v].amp = 1.0;
        s.parts[v].value = 0.0;
        s.parts[v].freq = 1000.0;
        let r = s.add_part(PartKind::Resistor, -60.0, 0.0);
        s.parts[r].value = 1000.0;
        let c = s.add_part(PartKind::Capacitor, 70.0, 0.0);
        s.parts[c].value = 1e-6;
        let gnd = s.add_part(PartKind::Ground, 70.0, 90.0);
        s.wire(v, "1", r, "1");
        s.wire(r, "2", c, "1");
        s.wire(c, "2", gnd, "0");
        s.wire(v, "2", gnd, "0");
        s.probe(r, "2", "out");
        s.analysis = Analysis::Tran;
        s.opts.dt = 2e-5;
        s.opts.t_stop = 2e-3;
        s
    }

    /// 5 V, 220 Ω and a red LED.
    pub fn led() -> Schematic {
        let mut s = Schematic::empty();
        s.name = "led".to_string();
        s.description = "5 V through 220 ohm into a red LED".to_string();
        let v = s.add_part(PartKind::Vsource, -190.0, 0.0);
        s.parts[v].value = 5.0;
        let r = s.add_part(PartKind::Resistor, -60.0, 0.0);
        s.parts[r].value = 220.0;
        let d = s.add_part(PartKind::Led, 70.0, 0.0);
        let gnd = s.add_part(PartKind::Ground, 70.0, 90.0);
        s.wire(v, "1", r, "1");
        s.wire(r, "2", d, "1");
        s.wire(d, "2", gnd, "0");
        s.wire(v, "2", gnd, "0");
        s.probe(d, "1", "anode");
        s
    }

    /// A virtual MCU pin driving an LED through 220 Ω.
    pub fn mcu_led() -> Schematic {
        let mut s = Schematic::empty();
        s.name = "mcu_led".to_string();
        s.description = "Virtual MCU pin C13 through 220 ohm into an LED".to_string();
        let p = s.add_part(PartKind::McuPin, -190.0, 0.0);
        s.parts[p].pin_name = "C13".to_string();
        let r = s.add_part(PartKind::Resistor, -60.0, 0.0);
        s.parts[r].value = 220.0;
        let d = s.add_part(PartKind::Led, 70.0, 0.0);
        let gnd = s.add_part(PartKind::Ground, 70.0, 90.0);
        s.wire(p, "p", r, "1");
        s.wire(r, "2", d, "1");
        s.wire(d, "2", gnd, "0");
        s.probe(p, "p", "mcu");
        s.mcu_on = true;
        s.analysis = Analysis::Tran;
        s.opts.dt = 1e-3;
        s.opts.t_stop = 2.0;
        s
    }

    /// Built-in examples, in menu order.
    pub fn examples() -> Vec<(&'static str, Schematic)> {
        vec![
            ("circuit.example_divider", Schematic::divider()),
            ("circuit.example_rc", Schematic::rc_lowpass()),
            ("circuit.example_led", Schematic::led()),
            ("circuit.example_mcu", Schematic::mcu_led()),
        ]
    }

    // ------------------------------------------------------------- editing

    pub fn add_part(&mut self, kind: PartKind, x: f32, y: f32) -> usize {
        let id = self.unique_id(kind);
        let part = Part::new(id, kind, x, y);
        self.parts.push(part);
        self.parts.len() - 1
    }

    fn unique_id(&self, kind: PartKind) -> String {
        let prefix = kind.prefix();
        let mut n = 1;
        loop {
            let id = format!("{prefix}{n}");
            if !self.parts.iter().any(|p| p.id == id) {
                return id;
            }
            n += 1;
        }
    }

    pub fn wire(&mut self, a_part: usize, a_pin: &str, b_part: usize, b_pin: &str) {
        let a = PinAddr::new(a_part, a_pin);
        let b = PinAddr::new(b_part, b_pin);
        if a == b {
            return;
        }
        if self
            .wires
            .iter()
            .any(|w| (w.a == a && w.b == b) || (w.a == b && w.b == a))
        {
            return;
        }
        self.wires.push(Wire { a, b });
    }

    pub fn probe(&mut self, part: usize, pin: &str, name: &str) {
        if self.probes.iter().any(|p| p.part == part && p.pin == pin) {
            return;
        }
        self.probes.push(CircuitProbe {
            part,
            pin: pin.to_string(),
            name: name.to_string(),
        });
    }

    pub fn remove_probe(&mut self, idx: usize) {
        if idx < self.probes.len() {
            self.probes.remove(idx);
        }
    }

    /// Delete a part and fix up the indices that follow it.
    pub fn remove_part(&mut self, idx: usize) {
        if idx >= self.parts.len() {
            return;
        }
        self.parts.remove(idx);
        self.wires.retain(|w| w.a.part != idx && w.b.part != idx);
        self.probes.retain(|p| p.part != idx);
        for w in &mut self.wires {
            if w.a.part > idx {
                w.a.part -= 1;
            }
            if w.b.part > idx {
                w.b.part -= 1;
            }
        }
        for p in &mut self.probes {
            if p.part > idx {
                p.part -= 1;
            }
        }
    }

    /// Where a pin sits relative to the body of its part, in world units.
    ///
    /// A catalogue part is a package: the inputs sit on the left edge, the
    /// outputs on the right one, and the supply pins on top and bottom.
    pub fn pin_offset(&self, part: usize, pin: &str) -> (f32, f32) {
        let Some(p) = self.parts.get(part) else {
            return (0.0, 0.0);
        };
        if p.is_catalog() {
            return self.catalog_pin_offset(p, pin);
        }
        match p.kind {
            PartKind::Ground => (0.0, -PART_H * 0.5),
            PartKind::McuPin => (PART_W * 0.5, 0.0),
            PartKind::Gate => match pin {
                "a" => (-PART_W * 0.5, -11.0),
                "b" => (-PART_W * 0.5, 11.0),
                _ => (PART_W * 0.5, 0.0),
            },
            _ => {
                if pin == "1" {
                    (-PART_W * 0.5, 0.0)
                } else {
                    (PART_W * 0.5, 0.0)
                }
            }
        }
    }

    /// The place of one pin of a catalogue part.
    fn catalog_pin_offset(&self, part: &Part, pin: &str) -> (f32, f32) {
        let mut inputs: Vec<&String> = Vec::new();
        let mut outputs: Vec<&String> = Vec::new();
        for unit in &part.catalog_units {
            for name in &unit.ins {
                if !inputs.contains(&name) {
                    inputs.push(name);
                }
            }
        }
        for unit in &part.catalog_units {
            for name in &unit.outs {
                if !outputs.contains(&name) {
                    outputs.push(name);
                }
            }
        }
        let half_w = CATALOG_W * 0.5;
        let row = |list: &[&String], name: &str| -> f32 {
            match list.iter().position(|n| n.as_str() == name) {
                Some(i) => {
                    let count = list.len().max(1) as f32;
                    let spacing = 16.0;
                    let total = (count - 1.0) * spacing;
                    -(total * 0.5) + i as f32 * spacing
                }
                None => 0.0,
            }
        };
        if inputs.iter().any(|n| n.as_str() == pin) {
            return (-half_w, row(&inputs, pin));
        }
        if outputs.iter().any(|n| n.as_str() == pin) {
            return (half_w, row(&outputs, pin));
        }
        // The supply pins sit on top and bottom.
        let power = part.catalog_power.clone().unwrap_or_default();
        if pin == power.vcc {
            return (0.0, -CATALOG_H * 0.5);
        }
        if pin == power.gnd {
            return (0.0, CATALOG_H * 0.5);
        }
        (0.0, 0.0)
    }

    // ----------------------------------------------------------- net naming

    fn pin_list(&self) -> Vec<(usize, String)> {
        let mut out = Vec::new();
        for (i, part) in self.parts.iter().enumerate() {
            for pin in part.pins() {
                out.push((i, pin.to_string()));
            }
        }
        out
    }

    /// Map every pin to the name of the net it belongs to.
    pub fn nets(&self) -> BTreeMap<(usize, String), String> {
        let pins = self.pin_list();
        let mut index: BTreeMap<(usize, String), usize> = BTreeMap::new();
        for (i, key) in pins.iter().enumerate() {
            index.insert(key.clone(), i);
        }
        let mut parent: Vec<usize> = (0..pins.len()).collect();

        fn find(parent: &mut [usize], i: usize) -> usize {
            let mut i = i;
            while parent[i] != i {
                parent[i] = parent[parent[i]];
                i = parent[i];
            }
            i
        }

        fn union(parent: &mut [usize], a: usize, b: usize) {
            let (ra, rb) = (find(parent, a), find(parent, b));
            if ra != rb {
                parent[rb] = ra;
            }
        }

        for w in &self.wires {
            let (Some(&a), Some(&b)) = (
                index.get(&(w.a.part, w.a.pin.clone())),
                index.get(&(w.b.part, w.b.pin.clone())),
            ) else {
                continue;
            };
            union(&mut parent, a, b);
        }

        // Every ground pin is the reference node.
        let grounds: Vec<usize> = self
            .parts
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind == PartKind::Ground)
            .filter_map(|(i, _)| index.get(&(i, "0".to_string())).copied())
            .collect();
        for pair in grounds.windows(2) {
            union(&mut parent, pair[0], pair[1]);
        }

        let mut groups: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
        for i in 0..pins.len() {
            let root = find(&mut parent, i);
            groups.entry(root).or_default().push(i);
        }

        let mut used: BTreeSet<String> = BTreeSet::new();
        used.insert("0".to_string());
        let mut out = BTreeMap::new();
        for members in groups.values() {
            let is_ground = members.iter().any(|i| {
                let (part, _) = &pins[*i];
                self.parts[*part].kind == PartKind::Ground
            });
            let name = if is_ground {
                "0".to_string()
            } else {
                self.net_name(&pins, members, &mut used)
            };
            for i in members {
                out.insert(pins[*i].clone(), name.clone());
            }
        }
        out
    }

    fn net_name(
        &self,
        pins: &[(usize, String)],
        members: &[usize],
        used: &mut BTreeSet<String>,
    ) -> String {
        let mut candidates: Vec<String> = Vec::new();
        for i in members {
            let (part, pin) = &pins[*i];
            if let Some(p) = self
                .probes
                .iter()
                .find(|p| p.part == *part && p.pin == *pin)
            {
                if !p.name.trim().is_empty() {
                    candidates.push(p.name.clone());
                }
            }
            if self.parts[*part].kind == PartKind::McuPin {
                let name = self.parts[*part].pin_name.trim();
                if !name.is_empty() {
                    candidates.push(name.to_string());
                }
            }
        }
        for i in members {
            let (part, pin) = &pins[*i];
            candidates.push(format!("{}_{}", self.parts[*part].id, pin));
        }
        let base = candidates
            .iter()
            .map(|c| sanitize_net(c))
            .find(|c| !c.is_empty() && c != "0")
            .unwrap_or_else(|| "n".to_string());
        let mut name = base.clone();
        let mut n = 2;
        while !used.insert(name.clone()) {
            name = format!("{base}_{n}");
            n += 1;
        }
        name
    }

    // ------------------------------------------------------------- netlist

    /// Turn the schematic into a netlist the solver understands.
    pub fn netlist(&self) -> Result<Circuit, String> {
        if self.parts.is_empty() {
            return Err("schematic is empty".to_string());
        }
        let nets = self.nets();
        let mut circuit = Circuit::new(if self.name.trim().is_empty() {
            "circuit"
        } else {
            self.name.trim()
        });
        circuit.description = self.description.clone();
        let mut has_ground = false;
        for (idx, part) in self.parts.iter().enumerate() {
            // A catalogue part is a package of units: every unit becomes its
            // own component in the netlist.
            if part.is_catalog() {
                let power = part
                    .catalog_power
                    .clone()
                    .unwrap_or_else(crate::library::Power::default);
                let supply = nets
                    .get(&(idx, power.vcc.clone()))
                    .cloned()
                    .unwrap_or_else(|| "0".to_string());
                for (unit_index, unit) in part.catalog_units.iter().enumerate() {
                    let mut pins: Vec<String> = Vec::new();
                    for name in &unit.ins {
                        let node = nets
                            .get(&(idx, name.clone()))
                            .cloned()
                            .ok_or_else(|| format!("{} pin {name} is not wired", part.id))?;
                        pins.push(node);
                    }
                    for name in &unit.outs {
                        let node = nets
                            .get(&(idx, name.clone()))
                            .cloned()
                            .ok_or_else(|| format!("{} pin {name} is not wired", part.id))?;
                        pins.push(node);
                    }
                    let n_in = unit.ins.len();
                    let id = format!("{}.{}", part.id, unit_index + 1);
                    let kind = if unit.func.is_sequential() {
                        ComponentKind::Seq(crate::netlist::Seq {
                            kind: crate::netlist::SeqKind::from_func(unit.func)
                                .ok_or_else(|| format!("{} has an unknown unit", part.id))?,
                            vdd: power.vdd,
                            vth: power.vth,
                            r: 50.0,
                            n_in,
                        })
                    } else {
                        ComponentKind::Logic(crate::netlist::Logic {
                            func: unit.func,
                            vdd: power.vdd,
                            vth: power.vth,
                            r: 50.0,
                            n_in,
                            sel_bits: unit.ins.len().saturating_sub(1),
                        })
                    };
                    circuit.add(Component { id, kind, pins });
                }
                let _ = supply;
                continue;
            }
            let mut pins: Vec<String> = Vec::new();
            for pin in part.pins() {
                let node = nets
                    .get(&(idx, pin.to_string()))
                    .cloned()
                    .ok_or_else(|| format!("{} has an unconnected pin {pin}", part.id))?;
                pins.push(node);
            }
            let kind = match part.kind {
                PartKind::Resistor => {
                    if !(part.value.is_finite() && part.value > 0.0) {
                        return Err(format!("{} needs a positive resistance", part.id));
                    }
                    ComponentKind::Resistor { r: part.value }
                }
                PartKind::Capacitor => {
                    if !(part.value.is_finite() && part.value > 0.0) {
                        return Err(format!("{} needs a positive capacitance", part.id));
                    }
                    ComponentKind::Capacitor {
                        c: part.value,
                        ic: 0.0,
                    }
                }
                PartKind::Inductor => {
                    if !(part.value.is_finite() && part.value > 0.0) {
                        return Err(format!("{} needs a positive inductance", part.id));
                    }
                    ComponentKind::Inductor {
                        l: part.value,
                        ic: 0.0,
                    }
                }
                PartKind::Vsource => ComponentKind::VSource(part.source()),
                PartKind::Isource => ComponentKind::ISource(part.source()),
                PartKind::Diode => ComponentKind::Diode(DiodeModel::default()),
                PartKind::Led => ComponentKind::Led(LedModel {
                    vf: part.value,
                    if_nom: if part.value2 > 0.0 { part.value2 } else { 0.02 },
                    ..LedModel::default()
                }),
                PartKind::Switch => ComponentKind::Switch {
                    on: part.on,
                    ron: 0.1,
                    roff: 1e9,
                },
                PartKind::Gate => ComponentKind::Gate(Gate {
                    kind: part.gate,
                    vdd: if part.value > 0.0 { part.value } else { 5.0 },
                    r: 50.0,
                    vth: if part.value > 0.0 {
                        part.value * 0.5
                    } else {
                        2.5
                    },
                }),
                PartKind::McuPin => ComponentKind::McuPin,
                PartKind::Ground => {
                    has_ground = true;
                    ComponentKind::Ground
                }
            };
            circuit.add(Component {
                id: part.id.clone(),
                kind,
                pins,
            });
        }
        if !has_ground {
            let id = if circuit.component("GND").is_some() {
                "GND2"
            } else {
                "GND"
            };
            circuit.add(Component::new(id, ComponentKind::Ground, &["0"]));
        }

        let mut names: BTreeSet<String> = BTreeSet::new();
        for probe in &self.probes {
            let Some(node) = nets.get(&(probe.part, probe.pin.clone())) else {
                continue;
            };
            if node == "0" {
                continue;
            }
            let mut name = sanitize_net(&probe.name);
            if name.is_empty() {
                name = node.clone();
            }
            let base = name.clone();
            let mut n = 2;
            while !names.insert(name.clone()) {
                name = format!("{base}_{n}");
                n += 1;
            }
            circuit.probe(name, node.clone());
        }
        circuit.validate()?;
        Ok(circuit)
    }

    /// MCU pins of the schematic, bound to the nets they drive.
    pub fn mcu_bindings(&self) -> Result<Vec<McuBinding>, String> {
        let nets = self.nets();
        let mut out: Vec<McuBinding> = Vec::new();
        for (idx, part) in self.parts.iter().enumerate() {
            if part.kind != PartKind::McuPin {
                continue;
            }
            let pin = part.pin_name.trim();
            if pin.is_empty() {
                return Err(format!("{} has an empty MCU pin name", part.id));
            }
            let Some(node) = nets.get(&(idx, "p".to_string())) else {
                return Err(format!("{} is not connected", part.id));
            };
            if out.iter().any(|b| b.pin == pin) {
                continue;
            }
            out.push(McuBinding {
                pin: pin.to_string(),
                node: node.clone(),
            });
        }
        Ok(out)
    }

    // --------------------------------------------------------- persistence

    pub fn to_json_value(&self) -> Json {
        let mut root = BTreeMap::new();
        root.insert("kind".into(), Json::Str("wireasm-circuit".into()));
        root.insert("version".into(), Json::Str("1".into()));
        root.insert("name".into(), Json::Str(self.name.clone()));
        if !self.description.is_empty() {
            root.insert("description".into(), Json::Str(self.description.clone()));
        }
        let parts = self
            .parts
            .iter()
            .map(|p| {
                let mut m = BTreeMap::new();
                m.insert("id".into(), Json::Str(p.id.clone()));
                m.insert("kind".into(), Json::Str(p.kind.key().into()));
                m.insert("x".into(), Json::Num(p.x as f64));
                m.insert("y".into(), Json::Num(p.y as f64));
                m.insert("value".into(), Json::Num(p.value));
                m.insert("value2".into(), Json::Num(p.value2));
                m.insert("on".into(), Json::Bool(p.on));
                m.insert("gate".into(), Json::Str(gate_short(p.gate).to_lowercase()));
                m.insert("pin".into(), Json::Str(p.pin_name.clone()));
                m.insert("wave".into(), Json::Str(p.wave.key().into()));
                m.insert("amp".into(), Json::Num(p.amp));
                m.insert("freq".into(), Json::Num(p.freq));
                m.insert("duty".into(), Json::Num(p.duty));
                if let Some(key) = &p.catalog {
                    m.insert("catalog".into(), Json::Str(key.clone()));
                    let units = p
                        .catalog_units
                        .iter()
                        .map(|u| {
                            let mut um = std::collections::BTreeMap::new();
                            um.insert("func".into(), Json::Str(u.func.key().into()));
                            um.insert(
                                "ins".into(),
                                Json::Arr(u.ins.iter().map(|s| Json::Str(s.clone())).collect()),
                            );
                            um.insert(
                                "outs".into(),
                                Json::Arr(u.outs.iter().map(|s| Json::Str(s.clone())).collect()),
                            );
                            Json::Obj(um)
                        })
                        .collect();
                    m.insert("units".into(), Json::Arr(units));
                    if let Some(power) = &p.catalog_power {
                        let mut pm = std::collections::BTreeMap::new();
                        pm.insert("vcc".into(), Json::Str(power.vcc.clone()));
                        pm.insert("gnd".into(), Json::Str(power.gnd.clone()));
                        pm.insert("vdd".into(), Json::Num(power.vdd));
                        pm.insert("vss".into(), Json::Num(power.vss));
                        pm.insert("vth".into(), Json::Num(power.vth));
                        m.insert("power".into(), Json::Obj(pm));
                    }
                }
                Json::Obj(m)
            })
            .collect();
        root.insert("parts".into(), Json::Arr(parts));
        let wires = self
            .wires
            .iter()
            .map(|w| {
                let mut m = BTreeMap::new();
                m.insert("a".into(), pin_json(&w.a));
                m.insert("b".into(), pin_json(&w.b));
                Json::Obj(m)
            })
            .collect();
        root.insert("wires".into(), Json::Arr(wires));
        let probes = self
            .probes
            .iter()
            .map(|p| {
                let mut m = BTreeMap::new();
                m.insert("part".into(), Json::Num(p.part as f64));
                m.insert("pin".into(), Json::Str(p.pin.clone()));
                m.insert("name".into(), Json::Str(p.name.clone()));
                Json::Obj(m)
            })
            .collect();
        root.insert("probes".into(), Json::Arr(probes));
        root.insert("analysis".into(), Json::Str(self.analysis.key().into()));
        root.insert("dt".into(), Json::Num(self.opts.dt));
        root.insert("stop".into(), Json::Num(self.opts.t_stop));
        root.insert("fstart".into(), Json::Num(self.f_start));
        root.insert("fstop".into(), Json::Num(self.f_stop));
        root.insert("points".into(), Json::Num(self.ac_points as f64));
        root.insert("mcu".into(), Json::Bool(self.mcu_on));
        root.insert("mcu_vdd".into(), Json::Num(self.mcu_vdd));
        Json::Obj(root)
    }

    pub fn from_json_value(v: &Json) -> Result<Schematic, String> {
        let mut s = Schematic::empty();
        s.name = v
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("circuit")
            .to_string();
        s.description = v
            .get("description")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let mut parts: Vec<Part> = Vec::new();
        if let Some(Json::Arr(items)) = v.get("parts") {
            for item in items {
                let kind = item
                    .get("kind")
                    .and_then(|x| x.as_str())
                    .and_then(PartKind::from_key)
                    .ok_or("part without a known kind")?;
                let id = item
                    .get("id")
                    .and_then(|x| x.as_str())
                    .unwrap_or("")
                    .to_string();
                let mut p = Part::new(id.clone(), kind, 0.0, 0.0);
                p.x = item.get("x").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                p.y = item.get("y").and_then(|x| x.as_f64()).unwrap_or(0.0) as f32;
                p.value = item
                    .get("value")
                    .and_then(|x| x.as_f64())
                    .unwrap_or(p.value);
                p.value2 = item
                    .get("value2")
                    .and_then(|x| x.as_f64())
                    .unwrap_or(p.value2);
                p.on = item.get("on").and_then(|x| x.as_bool()).unwrap_or(p.on);
                if let Some(g) = item.get("gate").and_then(|x| x.as_str()) {
                    if let Some(k) = GateKind::parse(g) {
                        p.gate = k;
                    }
                }
                if let Some(pin) = item.get("pin").and_then(|x| x.as_str()) {
                    p.pin_name = pin.to_string();
                }
                if let Some(w) = item.get("wave").and_then(|x| x.as_str()) {
                    if let Some(k) = WaveKind::from_key(w) {
                        p.wave = k;
                    }
                }
                p.amp = item.get("amp").and_then(|x| x.as_f64()).unwrap_or(p.amp);
                p.freq = item.get("freq").and_then(|x| x.as_f64()).unwrap_or(p.freq);
                p.duty = item.get("duty").and_then(|x| x.as_f64()).unwrap_or(p.duty);
                if let Some(key) = item.get("catalog").and_then(|x| x.as_str()) {
                    p.catalog = Some(key.to_string());
                    if let Some(Json::Arr(units)) = item.get("units") {
                        for u in units {
                            let func = u
                                .get("func")
                                .and_then(|x| x.as_str())
                                .and_then(crate::library::Func::from_key)
                                .ok_or_else(|| format!("part {id} has an unknown unit"))?;
                            let mut ins = Vec::new();
                            if let Some(Json::Arr(items)) = u.get("ins") {
                                for i in items {
                                    if let Some(s) = i.as_str() {
                                        ins.push(s.to_string());
                                    }
                                }
                            }
                            let mut outs = Vec::new();
                            if let Some(Json::Arr(items)) = u.get("outs") {
                                for i in items {
                                    if let Some(s) = i.as_str() {
                                        outs.push(s.to_string());
                                    }
                                }
                            }
                            p.catalog_units
                                .push(crate::library::Unit { func, ins, outs });
                        }
                    }
                    if let Some(pw) = item.get("power") {
                        p.catalog_power = Some(crate::library::Power {
                            vcc: pw
                                .get("vcc")
                                .and_then(|x| x.as_str())
                                .unwrap_or("VCC")
                                .to_string(),
                            gnd: pw
                                .get("gnd")
                                .and_then(|x| x.as_str())
                                .unwrap_or("GND")
                                .to_string(),
                            vdd: pw.get("vdd").and_then(|x| x.as_f64()).unwrap_or(5.0),
                            vss: pw.get("vss").and_then(|x| x.as_f64()).unwrap_or(0.0),
                            vth: pw.get("vth").and_then(|x| x.as_f64()).unwrap_or(2.5),
                        });
                    }
                }
                parts.push(p);
            }
        }
        s.parts = parts;
        if let Some(Json::Arr(items)) = v.get("wires") {
            for item in items {
                let (Some(a), Some(b)) = (item.get("a"), item.get("b")) else {
                    continue;
                };
                let (Some(a_part), Some(b_part)) = (
                    a.get("part").and_then(|x| x.as_u64()),
                    b.get("part").and_then(|x| x.as_u64()),
                ) else {
                    continue;
                };
                s.wires.push(Wire {
                    a: PinAddr::new(
                        a_part as usize,
                        a.get("pin").and_then(|x| x.as_str()).unwrap_or("1"),
                    ),
                    b: PinAddr::new(
                        b_part as usize,
                        b.get("pin").and_then(|x| x.as_str()).unwrap_or("2"),
                    ),
                });
            }
        }
        if let Some(Json::Arr(items)) = v.get("probes") {
            for item in items {
                let Some(part) = item.get("part").and_then(|x| x.as_u64()) else {
                    continue;
                };
                s.probes.push(CircuitProbe {
                    part: part as usize,
                    pin: item
                        .get("pin")
                        .and_then(|x| x.as_str())
                        .unwrap_or("1")
                        .to_string(),
                    name: item
                        .get("name")
                        .and_then(|x| x.as_str())
                        .unwrap_or("probe")
                        .to_string(),
                });
            }
        }
        if let Some(a) = v.get("analysis").and_then(|x| x.as_str()) {
            s.analysis = Analysis::from_key(a);
        }
        if let Some(dt) = v.get("dt").and_then(|x| x.as_f64()) {
            s.opts.dt = dt;
        }
        if let Some(stop) = v.get("stop").and_then(|x| x.as_f64()) {
            s.opts.t_stop = stop;
        }
        if let Some(f) = v.get("fstart").and_then(|x| x.as_f64()) {
            s.f_start = f;
        }
        if let Some(f) = v.get("fstop").and_then(|x| x.as_f64()) {
            s.f_stop = f;
        }
        if let Some(p) = v.get("points").and_then(|x| x.as_u64()) {
            s.ac_points = p as usize;
        }
        if let Some(m) = v.get("mcu").and_then(|x| x.as_bool()) {
            s.mcu_on = m;
        }
        if let Some(v) = v.get("mcu_vdd").and_then(|x| x.as_f64()) {
            s.mcu_vdd = v;
        }
        Ok(s)
    }
}

fn pin_json(p: &PinAddr) -> Json {
    let mut m = BTreeMap::new();
    m.insert("part".into(), Json::Num(p.part as f64));
    m.insert("pin".into(), Json::Str(p.pin.clone()));
    Json::Obj(m)
}

/// Body size of a catalogue part, in world units.
pub const CATALOG_W: f32 = 84.0;
/// Body height of a catalogue part, in world units.
pub const CATALOG_H: f32 = 56.0;

/// Body size of a placed part, in world units.
pub const PART_W: f32 = 84.0;
/// Body height of a placed part, in world units.
pub const PART_H: f32 = 40.0;

/// Distance from a point to a line segment.
pub fn distance_to_segment(x: f32, y: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
    let abx = bx - ax;
    let aby = by - ay;
    let len2 = abx * abx + aby * aby;
    if len2 <= f32::EPSILON {
        return ((x - ax).powi(2) + (y - ay).powi(2)).sqrt();
    }
    let t = (((x - ax) * abx + (y - ay) * aby) / len2).clamp(0.0, 1.0);
    let px = ax + abx * t;
    let py = ay + aby * t;
    ((x - px).powi(2) + (y - py).powi(2)).sqrt()
}

pub fn sanitize_net(name: &str) -> String {
    let mut out = String::new();
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        return String::new();
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, 'n');
    }
    out
}

/// Compact value text: `4.7 k`, `1 µ`, `12`.
pub fn fmt_value(v: f64) -> String {
    let a = v.abs();
    if a >= 1e9 {
        format!("{:.2} G", v / 1e9)
    } else if a >= 1e6 {
        format!("{:.2} M", v / 1e6)
    } else if a >= 1e3 {
        format!("{:.2} k", v / 1e3)
    } else if a >= 1.0 || a == 0.0 {
        format!("{:.2}", v)
    } else if a >= 1e-3 {
        format!("{:.2} m", v * 1e3)
    } else if a >= 1e-6 {
        format!("{:.2} µ", v * 1e6)
    } else {
        format!("{:.2} n", v * 1e9)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::Simulator;

    #[test]
    fn divider_netlist_and_dc() {
        let s = Schematic::divider();
        let circuit = s.netlist().unwrap();
        assert_eq!(circuit.components.len(), 4);
        let mut sim = Simulator::new(&circuit, Options::default()).unwrap();
        let sol = sim.dc().unwrap();
        let expected = 12.0 * 4700.0 / 14700.0;
        let out = sol.node("out");
        assert!((out - expected).abs() < 1e-6, "out={out}");
    }

    #[test]
    fn led_example_lights() {
        let s = Schematic::led();
        let circuit = s.netlist().unwrap();
        let mut sim = Simulator::new(&circuit, Options::default()).unwrap();
        let sol = sim.dc().unwrap();
        let anode = sol.node("anode");
        assert!(anode > 1.5 && anode < 2.5, "anode={anode}");
    }

    #[test]
    fn wires_create_nets() {
        let mut s = Schematic::empty();
        let v = s.add_part(PartKind::Vsource, 0.0, 0.0);
        let r = s.add_part(PartKind::Resistor, 100.0, 0.0);
        let g = s.add_part(PartKind::Ground, 100.0, 100.0);
        s.wire(v, "1", r, "1");
        s.wire(r, "2", g, "0");
        s.wire(v, "2", g, "0");
        let nets = s.nets();
        let a = nets.get(&(v, "1".to_string())).unwrap().clone();
        let b = nets.get(&(r, "1".to_string())).unwrap().clone();
        assert_eq!(a, b);
        let ground = nets.get(&(g, "0".to_string())).unwrap();
        assert_eq!(ground, "0");
    }

    #[test]
    fn json_round_trip() {
        let s = Schematic::rc_lowpass();
        let text = s.to_json_value().stringify_pretty();
        let v = Json::parse(&text).unwrap();
        let back = Schematic::from_json_value(&v).unwrap();
        assert_eq!(back.parts.len(), s.parts.len());
        assert_eq!(back.wires.len(), s.wires.len());
        assert_eq!(back.probes.len(), s.probes.len());
        assert_eq!(back.analysis, Analysis::Tran);
    }

    #[test]
    fn mcu_bindings_follow_pins() {
        let s = Schematic::mcu_led();
        let bindings = s.mcu_bindings().unwrap();
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].pin, "C13");
        assert!(!bindings[0].node.is_empty());
    }

    #[test]
    fn catalog_part_becomes_its_units() {
        // A 7400 is four NAND gates; in a netlist it becomes four components.
        let entry = crate::library::builtin()
            .entries
            .into_iter()
            .find(|e| e.key == "7400")
            .expect("7400 is in the catalogue");
        let mut s = Schematic::empty();
        let mut part = Part::new("U1".into(), PartKind::Gate, 0.0, 0.0);
        part.catalog = Some(entry.key.clone());
        part.catalog_units = entry.units.clone();
        part.catalog_power = Some(entry.power.clone());
        s.parts.push(part);
        let gnd = s.add_part(PartKind::Ground, 0.0, 100.0);
        // Wire the first gate: both inputs high, so the output is low.
        s.wire(0, "1A", gnd, "0");
        s.wire(0, "1B", gnd, "0");
        s.wire(0, "1Y", gnd, "0");
        let circuit = s.netlist().unwrap();
        let units = circuit
            .components
            .iter()
            .filter(|c| matches!(c.kind, ComponentKind::Logic(_)))
            .count();
        assert_eq!(units, 4, "7400 has four gates");
    }

    #[test]
    fn catalog_nand_gates_the_led() {
        // A 7400 wired as an inverter: the input high means the output low.
        let entry = crate::library::builtin()
            .entries
            .into_iter()
            .find(|e| e.key == "7400")
            .expect("7400 is in the catalogue");
        let mut s = Schematic::empty();
        let mut part = Part::new("U1".into(), PartKind::Gate, 0.0, 0.0);
        part.catalog = Some(entry.key.clone());
        part.catalog_units = entry.units.clone();
        part.catalog_power = Some(entry.power.clone());
        s.parts.push(part);
        // A 5 V source on input A, ground on input B, output to a probe.
        let v = s.add_part(PartKind::Vsource, -160.0, 0.0);
        s.parts[v].value = 5.0;
        let gnd = s.add_part(PartKind::Ground, 0.0, 90.0);
        s.wire(v, "1", 0, "1A");
        s.wire(v, "2", 0, "1B");
        s.wire(0, "1Y", gnd, "0");
        s.probe(0, "1Y", "out");
        let circuit = s.netlist().unwrap();
        let mut sim = Simulator::new(&circuit, s.opts.clone()).unwrap();
        let sol = sim.dc().unwrap();
        // Both inputs are high, so the NAND output is low.
        assert!(sol.node("out") < 0.5, "out={}", sol.node("out"));
    }

    #[test]
    fn mcu_schematic_drives_the_led() {
        // The blink graph toggles C13; the schematic must show it as a square
        // wave on the MCU pin.
        let graph = wireasm_core::model::Graph::from_json_value(
            &Json::parse(include_str!("../../../examples/stm32_blink.json")).unwrap(),
        )
        .unwrap();
        let mut s = Schematic::mcu_led();
        s.opts.dt = 1e-1;
        s.opts.t_stop = 2.0;
        let circuit = s.netlist().unwrap();
        let mut sim = Simulator::new(&circuit, s.opts.clone()).unwrap();
        let bindings = s.mcu_bindings().unwrap();
        let mut mcu = crate::mcu::VirtualMcu::new(graph).unwrap();
        mcu.run_until(0.0).unwrap();
        sim.attach_mcu(mcu, bindings, s.mcu_vdd, 30.0);
        let trace = sim.transient().unwrap();
        let pin = trace.series.get("mcu").expect("probe mcu");
        assert!(pin.iter().any(|v| *v > 2.0), "pin never went high");
        assert!(pin.iter().any(|v| *v < 0.5), "pin never went low");
    }
}
