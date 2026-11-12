//! Circuit netlist: components, pins and JSON input.
//!
//! A netlist names nodes by strings. Any of `0`, `gnd` or a pin of a `ground`
//! component is the reference node. Components are deliberately flat: the
//! solver asks each one for stamps, and the netlist stays easy to save, diff
//! and generate from the schematic editor.

use std::collections::BTreeMap;

use wireasm_core::json::Json;

/// One independent source, optionally with a time waveform.
#[derive(Clone, Debug)]
pub struct Source {
    pub dc: f64,
    pub ac: f64,
    pub waveform: Option<Waveform>,
}

impl Default for Source {
    fn default() -> Self {
        Source {
            dc: 0.0,
            ac: 0.0,
            waveform: None,
        }
    }
}

impl Source {
    pub fn dc(value: f64) -> Source {
        Source {
            dc: value,
            ..Source::default()
        }
    }

    pub fn ac(amplitude: f64) -> Source {
        Source {
            ac: amplitude,
            ..Source::default()
        }
    }

    /// Instantaneous value at `t` seconds. A waveform replaces `dc`; `dc` is
    /// the quiescent value used when no waveform is set.
    pub fn value(&self, t: f64) -> f64 {
        match &self.waveform {
            Some(w) => w.value(t),
            None => self.dc,
        }
    }

    /// Value used for an AC sweep: the AC amplitude, or a unit signal when the
    /// caller did not set one explicitly.
    pub fn ac_value(&self) -> f64 {
        if self.ac != 0.0 {
            self.ac
        } else {
            1.0
        }
    }
}

#[derive(Clone, Debug)]
pub enum Waveform {
    Sine {
        amplitude: f64,
        frequency: f64,
        phase: f64,
        offset: f64,
    },
    Pulse {
        low: f64,
        high: f64,
        period: f64,
        duty: f64,
        rise: f64,
        fall: f64,
    },
}

impl Waveform {
    pub fn value(&self, t: f64) -> f64 {
        match self {
            Waveform::Sine {
                amplitude,
                frequency,
                phase,
                offset,
            } => offset + amplitude * (2.0 * std::f64::consts::PI * frequency * t + phase).sin(),
            Waveform::Pulse {
                low,
                high,
                period,
                duty,
                rise,
                fall,
            } => {
                if *period <= 0.0 {
                    return *low;
                }
                let phase = t.rem_euclid(*period);
                let high_time = period * duty.clamp(0.0, 1.0);
                if high_time <= 0.0 {
                    return *low;
                }
                let rise = rise.max(0.0).min(high_time * 0.5);
                let fall = fall.max(0.0).min(high_time * 0.5);
                if phase < rise {
                    low + (high - low) * phase / rise
                } else if phase < high_time - fall {
                    *high
                } else if phase < high_time {
                    high - (high - low) * (phase - (high_time - fall)) / fall
                } else {
                    *low
                }
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct DiodeModel {
    pub is: f64,
    pub n: f64,
    pub rs: f64,
}

impl Default for DiodeModel {
    fn default() -> Self {
        DiodeModel {
            is: 1e-14,
            n: 1.0,
            rs: 0.1,
        }
    }
}

/// Light emitting diode. `vf` is the forward voltage at `if_nom`, which keeps
/// the model readable instead of asking for a saturation current.
#[derive(Clone, Debug)]
pub struct LedModel {
    pub vf: f64,
    pub if_nom: f64,
    pub n: f64,
    pub rs: f64,
}

impl Default for LedModel {
    fn default() -> Self {
        LedModel {
            vf: 2.0,
            if_nom: 0.02,
            n: 1.8,
            rs: 5.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GateKind {
    And,
    Or,
    Nand,
    Nor,
    Xor,
    Xnor,
    Not,
    Buffer,
}

impl GateKind {
    pub fn parse(s: &str) -> Option<GateKind> {
        match s.to_ascii_lowercase().as_str() {
            "and" => Some(GateKind::And),
            "or" => Some(GateKind::Or),
            "nand" => Some(GateKind::Nand),
            "nor" => Some(GateKind::Nor),
            "xor" => Some(GateKind::Xor),
            "xnor" => Some(GateKind::Xnor),
            "not" | "inverter" => Some(GateKind::Not),
            "buffer" | "buf" => Some(GateKind::Buffer),
            _ => None,
        }
    }

    /// Number of logic inputs.
    pub fn inputs(self) -> usize {
        match self {
            GateKind::Not | GateKind::Buffer => 1,
            _ => 2,
        }
    }

    pub fn eval(self, a: bool, b: bool) -> bool {
        match self {
            GateKind::And => a && b,
            GateKind::Or => a || b,
            GateKind::Nand => !(a && b),
            GateKind::Nor => !(a || b),
            GateKind::Xor => a ^ b,
            GateKind::Xnor => !(a ^ b),
            GateKind::Not => !a,
            GateKind::Buffer => a,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Gate {
    pub kind: GateKind,
    pub vdd: f64,
    pub r: f64,
    pub vth: f64,
}

impl Gate {
    pub fn cmos(kind: GateKind) -> Gate {
        Gate {
            kind,
            vdd: 5.0,
            r: 50.0,
            vth: 2.5,
        }
    }
}

/// The kind of a sequential unit, as understood by the solver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SeqKind {
    /// D flip-flop, rising edge.
    Dff,
    /// D flip-flop, falling edge.
    DffNeg,
    /// JK flip-flop.
    Jk,
    /// T flip-flop.
    Tff,
    /// 4-bit binary counter, up.
    Counter4,
    /// 4-bit binary counter, down.
    Counter4Down,
    /// Decade counter.
    BcdCounter,
    /// 8-bit shift register.
    Shift8,
}

impl SeqKind {
    pub fn key(self) -> &'static str {
        match self {
            SeqKind::Dff => "dff",
            SeqKind::DffNeg => "dff_neg",
            SeqKind::Jk => "jk",
            SeqKind::Tff => "tff",
            SeqKind::Counter4 => "counter4",
            SeqKind::Counter4Down => "counter4_down",
            SeqKind::BcdCounter => "bcd_counter",
            SeqKind::Shift8 => "shift8",
        }
    }

    /// The catalogue function this kind implements.
    pub fn func(self) -> crate::library::Func {
        use crate::library::Func;
        match self {
            SeqKind::Dff => Func::Dff,
            SeqKind::DffNeg => Func::DffNeg,
            SeqKind::Jk => Func::Jk,
            SeqKind::Tff => Func::Tff,
            SeqKind::Counter4 => Func::Counter4,
            SeqKind::Counter4Down => Func::Counter4Down,
            SeqKind::BcdCounter => Func::BcdCounter,
            SeqKind::Shift8 => Func::Shift8,
        }
    }

    /// Build a kind from a catalogue function.
    pub fn from_func(func: crate::library::Func) -> Option<SeqKind> {
        use crate::library::Func;
        match func {
            Func::Dff => Some(SeqKind::Dff),
            Func::DffNeg => Some(SeqKind::DffNeg),
            Func::Jk => Some(SeqKind::Jk),
            Func::Tff => Some(SeqKind::Tff),
            Func::Counter4 => Some(SeqKind::Counter4),
            Func::Counter4Down => Some(SeqKind::Counter4Down),
            Func::BcdCounter => Some(SeqKind::BcdCounter),
            Func::Shift8 => Some(SeqKind::Shift8),
            _ => None,
        }
    }
}

/// A logic unit that is not clocked: a gate inside a package.
#[derive(Clone, Debug)]
pub struct Logic {
    /// What the unit does.
    pub func: crate::library::Func,
    /// Supply level of the package, in volts.
    pub vdd: f64,
    /// Input threshold, in volts.
    pub vth: f64,
    /// Output resistance of the drivers.
    pub r: f64,
    /// How many of the pins are inputs; the rest are outputs.
    pub n_in: usize,
    /// How many pins of a multiplexer select the data input.
    pub sel_bits: usize,
}

/// A unit of a logic integrated circuit: a gate, a flip-flop or a counter.
#[derive(Clone, Debug)]
pub struct IcUnit {
    pub func: crate::library::Func,
    /// Signal names of the inputs.
    pub ins: Vec<String>,
    /// Signal names of the outputs.
    pub outs: Vec<String>,
}

/// A clocked logic unit: a flip-flop or a counter.
#[derive(Clone, Debug)]
pub struct Seq {
    /// What the unit does.
    pub kind: SeqKind,
    /// Supply level of the package, in volts.
    pub vdd: f64,
    /// Input threshold, in volts.
    pub vth: f64,
    /// Output resistance of the drivers.
    pub r: f64,
    /// How many of the pins are inputs; the rest are outputs.
    pub n_in: usize,
}

#[derive(Clone, Debug)]
pub enum ComponentKind {
    Resistor {
        r: f64,
    },
    Capacitor {
        c: f64,
        ic: f64,
    },
    Inductor {
        l: f64,
        ic: f64,
    },
    VSource(Source),
    ISource(Source),
    Diode(DiodeModel),
    Led(LedModel),
    Switch {
        on: bool,
        ron: f64,
        roff: f64,
    },
    Ground,
    Gate(Gate),
    /// A combinational logic unit from the catalogue.
    Logic(Logic),
    /// A clocked logic unit from the catalogue.
    Seq(Seq),
    /// A pin driven by the virtual MCU. The output voltage is set by the
    /// simulator, not by the netlist.
    McuPin,
    /// A measuring instrument: it watches nodes and reports what it sees.
    Instrument(Instrument),
}

/// A virtual instrument, the way Multisim puts them on the schematic.
#[derive(Clone, Debug)]
pub struct Instrument {
    /// Which instrument it is.
    pub kind: InstrumentKind,
    /// The channels it measures, one node per channel.
    pub channels: Vec<String>,
}

/// The instruments of the Multisim toolbar.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InstrumentKind {
    /// Oscilloscope.
    Scope,
    /// Multimeter.
    Multimeter,
    /// Function generator.
    FunctionGenerator,
    /// Bode plotter.
    BodePlotter,
    /// Logic analyzer.
    LogicAnalyzer,
    /// Wattmeter.
    Wattmeter,
    /// Frequency counter.
    FrequencyCounter,
}

impl InstrumentKind {
    pub fn key(self) -> &'static str {
        match self {
            InstrumentKind::Scope => "scope",
            InstrumentKind::Multimeter => "multimeter",
            InstrumentKind::FunctionGenerator => "function_generator",
            InstrumentKind::BodePlotter => "bode_plotter",
            InstrumentKind::LogicAnalyzer => "logic_analyzer",
            InstrumentKind::Wattmeter => "wattmeter",
            InstrumentKind::FrequencyCounter => "frequency_counter",
        }
    }

    pub fn from_key(key: &str) -> Option<InstrumentKind> {
        match key.trim().to_ascii_lowercase().as_str() {
            "scope" | "oscilloscope" => Some(InstrumentKind::Scope),
            "multimeter" => Some(InstrumentKind::Multimeter),
            "function_generator" | "funcgen" => Some(InstrumentKind::FunctionGenerator),
            "bode_plotter" | "bode" => Some(InstrumentKind::BodePlotter),
            "logic_analyzer" | "logic" => Some(InstrumentKind::LogicAnalyzer),
            "wattmeter" => Some(InstrumentKind::Wattmeter),
            "frequency_counter" | "freqcounter" => Some(InstrumentKind::FrequencyCounter),
            _ => None,
        }
    }

    /// The reference name Multisim gives the instrument, e.g. `XSC1`.
    pub fn prefix(self) -> &'static str {
        match self {
            InstrumentKind::Scope => "XSC",
            InstrumentKind::Multimeter => "XMM",
            InstrumentKind::FunctionGenerator => "XFG",
            InstrumentKind::BodePlotter => "XBP",
            InstrumentKind::LogicAnalyzer => "XLA",
            InstrumentKind::Wattmeter => "XWM",
            InstrumentKind::FrequencyCounter => "XFC",
        }
    }

    /// How many channels the instrument has.
    pub fn channel_count(self) -> usize {
        match self {
            InstrumentKind::Scope => 4,
            InstrumentKind::Multimeter => 1,
            InstrumentKind::FunctionGenerator => 1,
            InstrumentKind::BodePlotter => 2,
            InstrumentKind::LogicAnalyzer => 16,
            InstrumentKind::Wattmeter => 2,
            InstrumentKind::FrequencyCounter => 1,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Component {
    pub id: String,
    pub kind: ComponentKind,
    pub pins: Vec<String>,
}

impl Component {
    pub fn new(id: impl Into<String>, kind: ComponentKind, pins: &[&str]) -> Component {
        Component {
            id: id.into(),
            kind,
            pins: pins.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    pub fn is_ground(&self) -> bool {
        matches!(self.kind, ComponentKind::Ground)
    }
}

#[derive(Clone, Debug)]
pub struct Probe {
    pub name: String,
    pub node: String,
}

#[derive(Clone, Debug, Default)]
pub struct Circuit {
    pub name: String,
    pub description: String,
    pub components: Vec<Component>,
    pub probes: Vec<Probe>,
}

impl Circuit {
    pub fn new(name: impl Into<String>) -> Circuit {
        Circuit {
            name: name.into(),
            ..Circuit::default()
        }
    }

    pub fn add(&mut self, component: Component) -> &mut Self {
        self.components.push(component);
        self
    }

    pub fn probe(&mut self, name: impl Into<String>, node: impl Into<String>) -> &mut Self {
        self.probes.push(Probe {
            name: name.into(),
            node: node.into(),
        });
        self
    }

    pub fn component(&self, id: &str) -> Option<&Component> {
        self.components.iter().find(|c| c.id == id)
    }

    /// All node names that appear on a component pin.
    pub fn all_nodes(&self) -> Vec<String> {
        let mut out = Vec::new();
        for c in &self.components {
            for p in &c.pins {
                if !out.iter().any(|n| n == p) {
                    out.push(p.clone());
                }
            }
        }
        out
    }

    /// Reference node names: `0`, `gnd`, `GND` and every ground pin.
    pub fn ground_nodes(&self) -> Vec<String> {
        let mut out = vec!["0".to_string(), "gnd".to_string(), "GND".to_string()];
        for c in &self.components {
            if c.is_ground() {
                for p in &c.pins {
                    out.push(p.clone());
                }
            }
        }
        out
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.components.is_empty() {
            return Err("circuit has no components".into());
        }
        let mut ids = BTreeMap::new();
        let mut ground = false;
        for c in &self.components {
            if c.id.trim().is_empty() {
                return Err("component with empty id".into());
            }
            if ids.insert(c.id.clone(), true).is_some() {
                return Err(format!("duplicate component id `{}`", c.id));
            }
            if c.is_ground() {
                ground = true;
                if c.pins.is_empty() {
                    return Err(format!("ground `{}` has no node", c.id));
                }
            }
            match &c.kind {
                ComponentKind::Ground => {
                    if c.pins.len() != 1 {
                        return Err(format!("ground `{}` needs one pin", c.id));
                    }
                }
                ComponentKind::McuPin => {
                    if c.pins.len() != 1 {
                        return Err(format!("mcu pin `{}` needs one node", c.id));
                    }
                }
                ComponentKind::Instrument(inst) => {
                    if inst.channels.is_empty() {
                        return Err(format!("instrument `{}` has no channels", c.id));
                    }
                }
                ComponentKind::Logic(l) => {
                    let want = crate::ic::output_count_of(l.func, &c.pins[l.n_in..]);
                    if c.pins.len() != l.n_in + want {
                        return Err(format!(
                            "logic `{}` needs {} inputs and {} outputs",
                            c.id, l.n_in, want
                        ));
                    }
                }
                ComponentKind::Seq(s) => {
                    let want = crate::ic::output_count_of(s.kind.func(), &c.pins[s.n_in..]);
                    if c.pins.len() != s.n_in + want {
                        return Err(format!(
                            "clocked unit `{}` needs {} inputs and {} outputs",
                            c.id, s.n_in, want
                        ));
                    }
                }
                ComponentKind::Gate(g) => {
                    let want = g.kind.inputs() + 1;
                    if c.pins.len() != want {
                        return Err(format!(
                            "gate `{}` needs {} pins ({} inputs + output)",
                            c.id,
                            want,
                            g.kind.inputs()
                        ));
                    }
                    if g.r <= 0.0 || g.vdd <= 0.0 {
                        return Err(format!(
                            "gate `{}` has a bad vdd or output resistance",
                            c.id
                        ));
                    }
                }
                _ => {
                    if c.pins.len() != 2 {
                        return Err(format!("`{}` needs exactly two pins", c.id));
                    }
                }
            }
            match &c.kind {
                ComponentKind::Resistor { r } if !r.is_finite() || *r <= 0.0 => {
                    return Err(format!("resistor `{}` needs r > 0", c.id));
                }
                ComponentKind::Capacitor { c: v, .. } if !v.is_finite() || *v <= 0.0 => {
                    return Err(format!("capacitor `{}` needs c > 0", c.id));
                }
                ComponentKind::Inductor { l, .. } if !l.is_finite() || *l <= 0.0 => {
                    return Err(format!("inductor `{}` needs l > 0", c.id));
                }
                ComponentKind::Diode(d) if d.is <= 0.0 || d.n <= 0.0 || d.rs < 0.0 => {
                    return Err(format!("diode `{}` has a bad model", c.id));
                }
                ComponentKind::Led(l) if l.vf <= 0.0 || l.if_nom <= 0.0 || l.n <= 0.0 => {
                    return Err(format!("LED `{}` has a bad model", c.id));
                }
                _ => {}
            }
        }
        if !ground {
            return Err("circuit needs a ground component or a pin named 0/gnd".into());
        }
        let nodes = self.all_nodes();
        for p in &self.probes {
            if !nodes.iter().any(|n| n == &p.node) {
                return Err(format!(
                    "probe `{}` refers to unknown node `{}`",
                    p.name, p.node
                ));
            }
        }
        Ok(())
    }

    /// Write the circuit back to the JSON shape [`Circuit::from_json`] reads.
    /// The schematic editor uses this to export a netlist.
    pub fn to_json_value(&self) -> Json {
        let mut root = BTreeMap::new();
        root.insert("name".into(), Json::Str(self.name.clone()));
        if !self.description.is_empty() {
            root.insert("description".into(), Json::Str(self.description.clone()));
        }
        let mut items = Vec::new();
        for c in &self.components {
            let mut m = BTreeMap::new();
            m.insert("id".into(), Json::Str(c.id.clone()));
            m.insert(
                "pins".into(),
                Json::Arr(c.pins.iter().map(|p| Json::Str(p.clone())).collect()),
            );
            match &c.kind {
                ComponentKind::Resistor { r } => {
                    m.insert("kind".into(), Json::Str("resistor".into()));
                    m.insert("value".into(), Json::Num(*r));
                }
                ComponentKind::Capacitor { c: cap, ic } => {
                    m.insert("kind".into(), Json::Str("capacitor".into()));
                    m.insert("value".into(), Json::Num(*cap));
                    m.insert("ic".into(), Json::Num(*ic));
                }
                ComponentKind::Inductor { l, ic } => {
                    m.insert("kind".into(), Json::Str("inductor".into()));
                    m.insert("value".into(), Json::Num(*l));
                    m.insert("ic".into(), Json::Num(*ic));
                }
                ComponentKind::VSource(src) => {
                    m.insert("kind".into(), Json::Str("vsource".into()));
                    source_to_json(src, &mut m);
                }
                ComponentKind::ISource(src) => {
                    m.insert("kind".into(), Json::Str("isource".into()));
                    source_to_json(src, &mut m);
                }
                ComponentKind::Diode(d) => {
                    m.insert("kind".into(), Json::Str("diode".into()));
                    m.insert("is".into(), Json::Num(d.is));
                    m.insert("n".into(), Json::Num(d.n));
                    m.insert("rs".into(), Json::Num(d.rs));
                }
                ComponentKind::Led(l) => {
                    m.insert("kind".into(), Json::Str("led".into()));
                    m.insert("vf".into(), Json::Num(l.vf));
                    m.insert("if".into(), Json::Num(l.if_nom));
                    m.insert("n".into(), Json::Num(l.n));
                    m.insert("rs".into(), Json::Num(l.rs));
                }
                ComponentKind::Switch { on, ron, roff } => {
                    m.insert("kind".into(), Json::Str("switch".into()));
                    m.insert("on".into(), Json::Bool(*on));
                    m.insert("ron".into(), Json::Num(*ron));
                    m.insert("roff".into(), Json::Num(*roff));
                }
                ComponentKind::Ground => {
                    m.insert("kind".into(), Json::Str("ground".into()));
                }
                ComponentKind::Gate(g) => {
                    m.insert("kind".into(), Json::Str("gate".into()));
                    m.insert("gate".into(), Json::Str(gate_name(g.kind).into()));
                    m.insert("vdd".into(), Json::Num(g.vdd));
                    m.insert("r".into(), Json::Num(g.r));
                    m.insert("vth".into(), Json::Num(g.vth));
                }
                ComponentKind::McuPin => {
                    m.insert("kind".into(), Json::Str("mcu_pin".into()));
                }
                ComponentKind::Instrument(inst) => {
                    m.insert("kind".into(), Json::Str("instrument".into()));
                    m.insert("instrument".into(), Json::Str(inst.kind.key().into()));
                }
                ComponentKind::Logic(l) => {
                    m.insert("kind".into(), Json::Str("ic_logic".into()));
                    m.insert("func".into(), Json::Str(l.func.key().into()));
                    m.insert("n_in".into(), Json::Num(l.n_in as f64));
                    m.insert("sel_bits".into(), Json::Num(l.sel_bits as f64));
                    m.insert("vdd".into(), Json::Num(l.vdd));
                    m.insert("vth".into(), Json::Num(l.vth));
                    m.insert("r".into(), Json::Num(l.r));
                }
                ComponentKind::Seq(s) => {
                    m.insert("kind".into(), Json::Str("ic_seq".into()));
                    m.insert("seq".into(), Json::Str(s.kind.key().into()));
                    m.insert("n_in".into(), Json::Num(s.n_in as f64));
                    m.insert("vdd".into(), Json::Num(s.vdd));
                    m.insert("vth".into(), Json::Num(s.vth));
                    m.insert("r".into(), Json::Num(s.r));
                }
            }
            items.push(Json::Obj(m));
        }
        root.insert("components".into(), Json::Arr(items));
        let probes = self
            .probes
            .iter()
            .map(|p| {
                let mut m = BTreeMap::new();
                m.insert("name".into(), Json::Str(p.name.clone()));
                m.insert("node".into(), Json::Str(p.node.clone()));
                Json::Obj(m)
            })
            .collect();
        root.insert("probes".into(), Json::Arr(probes));
        Json::Obj(root)
    }

    pub fn from_json_str(src: &str) -> Result<Circuit, String> {
        let v = Json::parse(src)?;
        Circuit::from_json(&v)
    }

    pub fn from_json(v: &Json) -> Result<Circuit, String> {
        let mut c = Circuit::new(v.get("name").and_then(|x| x.as_str()).unwrap_or("circuit"));
        c.description = v
            .get("description")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let list = match v.get("components") {
            Some(Json::Arr(a)) => a,
            _ => return Err("circuit needs a `components` array".into()),
        };
        for item in list {
            c.components.push(parse_component(item)?);
        }
        if let Some(Json::Arr(probes)) = v.get("probes") {
            for p in probes {
                let name = p.get("name").and_then(|x| x.as_str()).unwrap_or("probe");
                let node = p
                    .get("node")
                    .and_then(|x| x.as_str())
                    .ok_or("probe needs a node")?;
                c.probes.push(Probe {
                    name: name.to_string(),
                    node: node.to_string(),
                });
            }
        }
        c.validate()?;
        Ok(c)
    }
}

fn source_to_json(src: &Source, m: &mut BTreeMap<String, Json>) {
    m.insert("dc".into(), Json::Num(src.dc));
    m.insert("ac".into(), Json::Num(src.ac));
    if let Some(w) = &src.waveform {
        let mut wm = BTreeMap::new();
        match w {
            Waveform::Sine {
                amplitude,
                frequency,
                phase,
                offset,
            } => {
                wm.insert("type".into(), Json::Str("sine".into()));
                wm.insert("amplitude".into(), Json::Num(*amplitude));
                wm.insert("frequency".into(), Json::Num(*frequency));
                wm.insert("phase".into(), Json::Num(*phase));
                wm.insert("offset".into(), Json::Num(*offset));
            }
            Waveform::Pulse {
                low,
                high,
                period,
                duty,
                rise,
                fall,
            } => {
                wm.insert("type".into(), Json::Str("pulse".into()));
                wm.insert("low".into(), Json::Num(*low));
                wm.insert("high".into(), Json::Num(*high));
                wm.insert("period".into(), Json::Num(*period));
                wm.insert("duty".into(), Json::Num(*duty));
                wm.insert("rise".into(), Json::Num(*rise));
                wm.insert("fall".into(), Json::Num(*fall));
            }
        }
        m.insert("waveform".into(), Json::Obj(wm));
    }
}

fn gate_name(kind: GateKind) -> &'static str {
    match kind {
        GateKind::And => "and",
        GateKind::Or => "or",
        GateKind::Nand => "nand",
        GateKind::Nor => "nor",
        GateKind::Xor => "xor",
        GateKind::Xnor => "xnor",
        GateKind::Not => "not",
        GateKind::Buffer => "buffer",
    }
}

fn parse_component(v: &Json) -> Result<Component, String> {
    let id = v
        .get("id")
        .and_then(|x| x.as_str())
        .ok_or("component needs an id")?;
    let kind_name = v
        .get("kind")
        .and_then(|x| x.as_str())
        .ok_or_else(|| format!("component `{id}` needs a kind"))?;
    let pins: Vec<String> = match v.get("pins") {
        Some(Json::Arr(a)) => a
            .iter()
            .map(|p| {
                p.as_str()
                    .map(|s| s.to_string())
                    .ok_or_else(|| format!("component `{id}` has a non-string pin"))
            })
            .collect::<Result<_, _>>()?,
        _ => return Err(format!("component `{id}` needs a pins array")),
    };
    let n = |keys: &[&str], default: f64| -> f64 {
        keys.iter()
            .find_map(|k| v.get(k).and_then(|x| x.as_f64()))
            .unwrap_or(default)
    };
    let b = |keys: &[&str], default: bool| -> bool {
        keys.iter()
            .find_map(|k| v.get(k).and_then(|x| x.as_bool()))
            .unwrap_or(default)
    };
    let kind = match kind_name.to_ascii_lowercase().as_str() {
        "resistor" | "res" | "r" => ComponentKind::Resistor {
            r: n(&["value", "r", "ohm", "ohms", "resistance"], 1000.0),
        },
        "capacitor" | "cap" | "c" => ComponentKind::Capacitor {
            c: n(&["value", "c", "farad", "farads"], 1e-6),
            ic: n(&["ic", "v0"], 0.0),
        },
        "inductor" | "ind" | "l" => ComponentKind::Inductor {
            l: n(&["value", "l", "henry", "henries"], 1e-3),
            ic: n(&["ic", "i0"], 0.0),
        },
        "vsource" | "vsrc" | "v" | "voltage" => ComponentKind::VSource(parse_source(v)?),
        "isource" | "isrc" | "i" | "current" => ComponentKind::ISource(parse_source(v)?),
        "diode" | "d" => ComponentKind::Diode(DiodeModel {
            is: n(&["is", "saturation"], 1e-14),
            n: n(&["n", "emission"], 1.0),
            rs: n(&["rs", "series"], 0.1),
        }),
        "led" => ComponentKind::Led(LedModel {
            vf: n(&["vf", "forward"], 2.0),
            if_nom: n(&["if", "if_nom", "current"], 0.02),
            n: n(&["n", "emission"], 1.8),
            rs: n(&["rs", "series"], 5.0),
        }),
        "switch" | "sw" => ComponentKind::Switch {
            on: b(&["on", "closed"], false),
            ron: n(&["ron", "on_resistance"], 0.1),
            roff: n(&["roff", "off_resistance"], 1e9),
        },
        "ground" | "gnd" => ComponentKind::Ground,
        "gate" | "logic" => {
            let gate_name = v.get("gate").and_then(|x| x.as_str()).unwrap_or("and");
            let kind = GateKind::parse(gate_name)
                .ok_or_else(|| format!("gate `{id}` has unknown kind `{gate_name}`"))?;
            ComponentKind::Gate(Gate {
                kind,
                vdd: n(&["vdd", "supply"], 5.0),
                r: n(&["r", "rout", "output_resistance"], 50.0),
                vth: n(&["vth", "threshold"], 2.5),
            })
        }
        "mcu_pin" | "pin" | "mcupin" => ComponentKind::McuPin,
        "instrument" => {
            let kind = v
                .get("instrument")
                .and_then(|x| x.as_str())
                .and_then(InstrumentKind::from_key)
                .ok_or_else(|| format!("component `{id}` has an unknown instrument"))?;
            ComponentKind::Instrument(Instrument {
                kind,
                channels: Vec::new(),
            })
        }
        "ic_logic" | "catalogue_logic" => {
            let func = v
                .get("func")
                .and_then(|x| x.as_str())
                .and_then(crate::library::Func::from_key)
                .ok_or_else(|| format!("component `{id}` has an unknown logic function"))?;
            ComponentKind::Logic(Logic {
                func,
                vdd: n(&["vdd", "supply"], 5.0),
                vth: n(&["vth", "threshold"], 2.5),
                r: n(&["r", "rout", "output_resistance"], 50.0),
                n_in: n(&["n_in", "inputs"], 2.0) as usize,
                sel_bits: n(&["sel_bits", "select"], 0.0) as usize,
            })
        }
        "ic_seq" | "catalogue_seq" => {
            let func = v
                .get("seq")
                .and_then(|x| x.as_str())
                .and_then(crate::library::Func::from_key)
                .and_then(SeqKind::from_func)
                .ok_or_else(|| format!("component `{id}` has an unknown clocked function"))?;
            ComponentKind::Seq(Seq {
                kind: func,
                vdd: n(&["vdd", "supply"], 5.0),
                vth: n(&["vth", "threshold"], 2.5),
                r: n(&["r", "rout", "output_resistance"], 50.0),
                n_in: n(&["n_in", "inputs"], 2.0) as usize,
            })
        }
        other => return Err(format!("component `{id}` has unknown kind `{other}`")),
    };
    Ok(Component {
        id: id.to_string(),
        kind,
        pins,
    })
}

fn parse_source(v: &Json) -> Result<Source, String> {
    let mut src = Source {
        dc: v
            .get("dc")
            .and_then(|x| x.as_f64())
            .or_else(|| v.get("value").and_then(|x| x.as_f64()))
            .or_else(|| v.get("voltage").and_then(|x| x.as_f64()))
            .unwrap_or(0.0),
        ac: v
            .get("ac")
            .and_then(|x| x.as_f64())
            .or_else(|| v.get("amplitude").and_then(|x| x.as_f64()))
            .unwrap_or(0.0),
        waveform: None,
    };
    if let Some(w) = v.get("waveform") {
        let ty = w.get("type").and_then(|x| x.as_str()).unwrap_or("sine");
        let num = |key: &str, default: f64| -> f64 {
            w.get(key).and_then(|x| x.as_f64()).unwrap_or(default)
        };
        src.waveform = Some(match ty {
            "sine" | "sin" => Waveform::Sine {
                amplitude: num("amplitude", 1.0),
                frequency: num("frequency", num("freq", 50.0)),
                phase: num("phase", 0.0),
                offset: num("offset", 0.0),
            },
            "pulse" | "square" => Waveform::Pulse {
                low: num("low", 0.0),
                high: num("high", 5.0),
                period: num("period", 0.001),
                duty: num("duty", 0.5),
                rise: num("rise", 0.0),
                fall: num("fall", 0.0),
            },
            other => return Err(format!("unknown waveform `{other}`")),
        });
    }
    Ok(src)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_divider() {
        let src = r#"{
          "name": "divider",
          "components": [
            {"id":"V1","kind":"vsource","pins":["in","0"],"value":12},
            {"id":"R1","kind":"resistor","pins":["in","out"],"value":10000},
            {"id":"R2","kind":"resistor","pins":["out","0"],"value":4700},
            {"id":"GND","kind":"ground","pins":["0"]}
          ],
          "probes": [{"name":"out","node":"out"}]
        }"#;
        let c = Circuit::from_json_str(src).unwrap();
        assert_eq!(c.components.len(), 4);
        assert_eq!(c.probes.len(), 1);
    }

    #[test]
    fn rejects_bad_values() {
        let src = r#"{
          "components": [
            {"id":"R1","kind":"resistor","pins":["a","0"],"value":-1},
            {"id":"GND","kind":"ground","pins":["0"]}
          ]
        }"#;
        assert!(Circuit::from_json_str(src).is_err());
    }

    #[test]
    fn pulse_edges() {
        let w = Waveform::Pulse {
            low: 0.0,
            high: 5.0,
            period: 1.0,
            duty: 0.5,
            rise: 0.1,
            fall: 0.1,
        };
        assert_eq!(w.value(0.0), 0.0);
        assert!((w.value(0.4) - 5.0).abs() < 1e-9);
        assert!((w.value(0.55) - 0.0).abs() < 1e-9);
        assert!((w.value(1.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn json_round_trip() {
        let src = r#"{
          "name": "round",
          "components": [
            {"id":"V1","kind":"vsource","pins":["in","0"],"dc":5,
             "waveform":{"type":"sine","amplitude":1,"frequency":1000}},
            {"id":"R1","kind":"resistor","pins":["in","out"],"value":1000},
            {"id":"C1","kind":"capacitor","pins":["out","0"],"value":1e-6},
            {"id":"D1","kind":"led","pins":["out","0"],"vf":2,"if":0.02},
            {"id":"G1","kind":"gate","gate":"and","pins":["a","b","y"],"vdd":5},
            {"id":"S1","kind":"switch","pins":["a","b"],"on":true},
            {"id":"U1","kind":"mcu_pin","pins":["mcu"]},
            {"id":"GND","kind":"ground","pins":["0"]}
          ],
          "probes": [{"name":"out","node":"out"}]
        }"#;
        let c = Circuit::from_json_str(src).unwrap();
        let text = c.to_json_value().stringify_pretty();
        let back = Circuit::from_json_str(&text).unwrap();
        assert_eq!(back.components.len(), c.components.len());
        assert_eq!(back.probes.len(), 1);
        match &back.components[3].kind {
            ComponentKind::Led(l) => assert!((l.vf - 2.0).abs() < 1e-9),
            other => panic!("wrong kind {other:?}"),
        }
    }
}
