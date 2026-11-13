//! Component library: the catalogue of parts a schematic can hold.
//!
//! The catalogue is data, not code: every entry names a part, its family,
//! its units (gates, flip-flops, counters) and its pins. The schematic editor
//! draws the entry, the solver simulates it, and the library can be written to
//! and read from JSON so users can share their own sets of parts.

use std::collections::BTreeMap;

use wireasm_core::json::Json;

/// Where a part comes from. The families follow the Multisim component
/// toolbar: basic parts, the TTL families, CMOS and the discrete devices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    /// Resistors, capacitors, sources, switches — the parts with no family
    /// history.
    Basic,
    /// The original 74xx bipolar TTL series.
    Ttl,
    /// Schottky TTL: 74LS, 74S, 74F, 74HC and friends.
    TtlSchottky,
    /// Emitter-coupled logic: 10xxx.
    Ecl,
    /// CMOS: CD4000 and 74HC.
    Cmos,
    /// Analog building blocks: amplifiers, comparators, timers.
    Analog,
    /// Discrete semiconductors: diodes, transistors, thyristors.
    Discrete,
    /// Instruments and other parts that only exist inside the simulator.
    Virtual,
    /// The measuring instruments of the Multisim toolbar.
    Instrument,
}

impl Family {
    pub fn key(self) -> &'static str {
        match self {
            Family::Basic => "basic",
            Family::Ttl => "ttl",
            Family::TtlSchottky => "ttl_shottky",
            Family::Ecl => "ecl",
            Family::Cmos => "cmos",
            Family::Analog => "analog",
            Family::Discrete => "discrete",
            Family::Virtual => "virtual",
            Family::Instrument => "instrument",
        }
    }

    pub fn from_key(key: &str) -> Option<Family> {
        match key.trim().to_ascii_lowercase().as_str() {
            "basic" => Some(Family::Basic),
            "ttl" => Some(Family::Ttl),
            "ttl_shottky" | "ttl shottky" | "shottky" => Some(Family::TtlSchottky),
            "ecl" => Some(Family::Ecl),
            "cmos" => Some(Family::Cmos),
            "analog" => Some(Family::Analog),
            "discrete" => Some(Family::Discrete),
            "virtual" => Some(Family::Virtual),
            "instrument" => Some(Family::Instrument),
            _ => None,
        }
    }

    /// Localization key for the family name.
    pub fn label_key(self) -> &'static str {
        match self {
            Family::Basic => "lib.basic",
            Family::Ttl => "lib.ttl",
            Family::TtlSchottky => "lib.ttl_shottky",
            Family::Ecl => "lib.ecl",
            Family::Cmos => "lib.cmos",
            Family::Analog => "lib.analog",
            Family::Discrete => "lib.discrete",
            Family::Virtual => "lib.virtual",
            Family::Instrument => "lib.instrument",
        }
    }
}

/// What one unit of a part does. A quad NAND is four `Nand2` units; a hex
/// inverter is six `Not1` units.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Func {
    // Combinational logic.
    And2,
    And3,
    And4,
    Or2,
    Or3,
    Or4,
    Nand2,
    Nand3,
    Nand4,
    Nor2,
    Nor3,
    Nor4,
    Xor2,
    Xnor2,
    Not1,
    Buf1,
    /// Multiplexer: `A`, `B`, `SEL` in, `Y` out.
    Mux2,
    /// Exclusive OR with an enable.
    Xor2Enable,
    // Sequential logic.
    /// D flip-flop, rising edge: `D`, `CLK` in, `Q`, `/Q` out.
    Dff,
    /// D flip-flop, falling edge.
    DffNeg,
    /// JK flip-flop: `J`, `K`, `CLK` in, `Q`, `/Q` out.
    Jk,
    /// T flip-flop.
    Tff,
    /// Binary counter, up: `CLK` in, `Q0..Q3`, `CARRY` out.
    Counter4,
    /// Binary counter, down.
    Counter4Down,
    /// BCD counter with decade decode.
    BcdCounter,
    /// Eight-bit shift register.
    Shift8,
    /// Four-to-sixteen decoder.
    Decoder4to16,
    /// Three-to-eight decoder: three address inputs, eight outputs.
    Decoder3to8,
    /// Eight-to-three priority encoder.
    Encoder8to3,
}

impl Func {
    pub fn key(self) -> &'static str {
        match self {
            Func::And2 => "and2",
            Func::And3 => "and3",
            Func::And4 => "and4",
            Func::Or2 => "or2",
            Func::Or3 => "or3",
            Func::Or4 => "or4",
            Func::Nand2 => "nand2",
            Func::Nand3 => "nand3",
            Func::Nand4 => "nand4",
            Func::Nor2 => "nor2",
            Func::Nor3 => "nor3",
            Func::Nor4 => "nor4",
            Func::Xor2 => "xor2",
            Func::Xnor2 => "xnor2",
            Func::Not1 => "not1",
            Func::Buf1 => "buf1",
            Func::Mux2 => "mux2",
            Func::Xor2Enable => "xor2_enable",
            Func::Dff => "dff",
            Func::DffNeg => "dff_neg",
            Func::Jk => "jk",
            Func::Tff => "tff",
            Func::Counter4 => "counter4",
            Func::Counter4Down => "counter4_down",
            Func::BcdCounter => "bcd_counter",
            Func::Shift8 => "shift8",
            Func::Decoder4to16 => "decoder4to16",
            Func::Decoder3to8 => "decoder3to8",
            Func::Encoder8to3 => "encoder8to3",
        }
    }

    pub fn from_key(key: &str) -> Option<Func> {
        const ALL: &[Func] = &[
            Func::And2,
            Func::And3,
            Func::And4,
            Func::Or2,
            Func::Or3,
            Func::Or4,
            Func::Nand2,
            Func::Nand3,
            Func::Nand4,
            Func::Nor2,
            Func::Nor3,
            Func::Nor4,
            Func::Xor2,
            Func::Xnor2,
            Func::Not1,
            Func::Buf1,
            Func::Mux2,
            Func::Xor2Enable,
            Func::Dff,
            Func::DffNeg,
            Func::Jk,
            Func::Tff,
            Func::Counter4,
            Func::Counter4Down,
            Func::BcdCounter,
            Func::Shift8,
            Func::Decoder4to16,
            Func::Decoder3to8,
            Func::Encoder8to3,
        ];
        ALL.iter().copied().find(|f| f.key() == key.trim())
    }

    /// True for units that remember their state between clock edges.
    pub fn is_sequential(self) -> bool {
        matches!(
            self,
            Func::Dff
                | Func::DffNeg
                | Func::Jk
                | Func::Tff
                | Func::Counter4
                | Func::Counter4Down
                | Func::BcdCounter
                | Func::Shift8
                | Func::Decoder4to16
        )
    }

    /// True for units whose output changes on a clock edge.
    pub fn is_edge_triggered(self) -> bool {
        matches!(
            self,
            Func::Dff | Func::DffNeg | Func::Jk | Func::Tff | Func::Counter4 | Func::Counter4Down
        )
    }
}

/// One unit of a part: a gate, a flip-flop, a counter.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unit {
    pub func: Func,
    /// Signal names of the inputs, e.g. `1A`, `1B`.
    pub ins: Vec<String>,
    /// Signal names of the outputs, e.g. `1Y`.
    pub outs: Vec<String>,
    /// How many of the inputs select the data input of a multiplexer.
    /// `None` means the function does not need select lines.
    pub sel_bits: Option<u8>,
    /// Whether the unit has an enable input.
    pub has_enable: bool,
}

impl Unit {
    pub fn new(func: Func, ins: &[&str], outs: &[&str]) -> Unit {
        Unit {
            func,
            ins: ins.iter().map(|s| s.to_string()).collect(),
            outs: outs.iter().map(|s| s.to_string()).collect(),
            sel_bits: None,
            has_enable: false,
        }
    }

    /// A unit that selects one of its data inputs.
    ///
    /// `sel_bits` says how many lines carry the address, `enable` says
    /// whether the last input enables the unit.
    pub fn mux(func: Func, ins: &[&str], outs: &[&str], sel_bits: u8, enable: bool) -> Unit {
        Unit {
            func,
            ins: ins.iter().map(|s| s.to_string()).collect(),
            outs: outs.iter().map(|s| s.to_string()).collect(),
            sel_bits: Some(sel_bits),
            has_enable: enable,
        }
    }
}

/// Supply pins and the logic levels they imply.
#[derive(Clone, Debug, PartialEq)]
pub struct Power {
    /// Name of the supply pin, usually `VCC`.
    pub vcc: String,
    /// Name of the ground pin, usually `GND`.
    pub gnd: String,
    /// Voltage of the supply pin.
    pub vdd: f64,
    /// Voltage of the ground pin.
    pub vss: f64,
    /// Input threshold, in volts.
    pub vth: f64,
}

impl Default for Power {
    fn default() -> Self {
        Power {
            vcc: "VCC".to_string(),
            gnd: "GND".to_string(),
            vdd: 5.0,
            vss: 0.0,
            vth: 2.5,
        }
    }
}

impl Power {
    pub fn ttl() -> Power {
        Power {
            vdd: 5.0,
            vth: 1.4,
            ..Power::default()
        }
    }

    pub fn cmos() -> Power {
        Power {
            vdd: 5.0,
            vth: 2.5,
            ..Power::default()
        }
    }
}

/// A part in the catalogue.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Catalogue key, e.g. `7400` or `74LS00`.
    pub key: String,
    /// Human name, e.g. "Quad 2-input NAND".
    pub title: String,
    pub family: Family,
    /// Reference designator prefix, e.g. `U`.
    pub prefix: String,
    /// What the part does.
    pub units: Vec<Unit>,
    /// Physical pin numbers: `(number, signal name)`.
    pub dip: Vec<(u8, String)>,
    pub power: Power,
    /// Supply current draw in milliamperes, for the parts that have one.
    pub ma: Option<f64>,
    /// The device the part is, when it is not made of logic units.
    pub device: Option<Device>,
}

/// A part that is not made of logic units: a diode, a transistor, an
/// amplifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Device {
    Diode,
    Led,
    Zener,
    /// A bipolar transistor.
    Bjt { npn: bool },
    /// A field-effect transistor.
    Mosfet { nch: bool },
    /// An operational amplifier.
    OpAmp,
    /// A comparator.
    Comparator,
    /// The 555 timer.
    Timer555,
}

impl Device {
    pub fn key(self) -> &'static str {
        match self {
            Device::Diode => "diode",
            Device::Led => "led",
            Device::Zener => "zener",
            Device::Bjt { npn: true } => "npn",
            Device::Bjt { npn: false } => "pnp",
            Device::Mosfet { nch: true } => "nmfet",
            Device::Mosfet { nch: false } => "pmfet",
            Device::OpAmp => "opamp",
            Device::Comparator => "comparator",
            Device::Timer555 => "timer555",
        }
    }

    pub fn from_key(key: &str) -> Option<Device> {
        match key.trim().to_ascii_lowercase().as_str() {
            "diode" => Some(Device::Diode),
            "led" => Some(Device::Led),
            "zener" => Some(Device::Zener),
            "npn" => Some(Device::Bjt { npn: true }),
            "pnp" => Some(Device::Bjt { npn: false }),
            "nmfet" => Some(Device::Mosfet { nch: true }),
            "pmfet" => Some(Device::Mosfet { nch: false }),
            "opamp" => Some(Device::OpAmp),
            "comparator" => Some(Device::Comparator),
            "timer555" => Some(Device::Timer555),
            _ => None,
        }
    }

    /// The names of the pins, in the order the solver expects them.
    pub fn pin_names(self) -> Vec<String> {
        match self {
            Device::Bjt { .. } => vec!["B".into(), "C".into(), "E".into()],
            Device::Mosfet { .. } => vec!["G".into(), "D".into(), "S".into()],
            Device::OpAmp => vec!["IN+".into(), "IN-".into(), "OUT".into()],
            Device::Comparator => vec!["IN+".into(), "IN-".into(), "OUT".into()],
            Device::Timer555 => vec![
                "GND".into(),
                "TRIG".into(),
                "THR".into(),
                "OUT".into(),
                "DISCH".into(),
                "CTRL".into(),
                "VCC".into(),
            ],
            _ => vec!["A".into(), "K".into()],
        }
    }
}

impl Entry {
    /// Signal names of every pin, supply included.
    pub fn pin_names(&self) -> Vec<String> {
        let mut out: Vec<String> = self.dip.iter().map(|(_, n)| n.clone()).collect();
        out.push(self.power.vcc.clone());
        out.push(self.power.gnd.clone());
        out
    }

    /// Signal names in the order the parts are drawn: the inputs of every
    /// unit, then its outputs, then the supply pins.
    pub fn ordered_pins(&self) -> Vec<String> {
        let mut out = Vec::new();
        for unit in &self.units {
            for name in &unit.ins {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
        }
        for unit in &self.units {
            for name in &unit.outs {
                if !out.contains(name) {
                    out.push(name.clone());
                }
            }
        }
        out.push(self.power.vcc.clone());
        out.push(self.power.gnd.clone());
        out
    }

    /// The units of a package, as the solver sees them.
    pub fn solver_units(&self) -> Vec<crate::netlist::IcUnit> {
        self.units
            .iter()
            .map(|u| crate::netlist::IcUnit {
                func: u.func,
                ins: u.ins.clone(),
                outs: u.outs.clone(),
            })
            .collect()
    }

    /// How many pins the package has, supply included.
    pub fn pin_count(&self) -> usize {
        self.dip.len() + 2
    }

    /// The pin number a signal sits on.
    pub fn pin_number(&self, signal: &str) -> Option<u8> {
        self.dip
            .iter()
            .find(|(_, n)| n == signal)
            .map(|(num, _)| *num)
            .or_else(|| {
                let max = self.dip.iter().map(|(n, _)| *n).max().unwrap_or(0);
                if signal == self.power.vcc {
                    Some(self.power.vcc_pin(max))
                } else if signal == self.power.gnd {
                    Some(self.power.gnd_pin(max))
                } else {
                    None
                }
            })
    }

    /// Units whose inputs are the given signal.
    pub fn unit_of_input(&self, signal: &str) -> Option<&Unit> {
        self.units
            .iter()
            .find(|u| u.ins.iter().any(|s| s == signal))
    }

    pub fn unit_of_output(&self, signal: &str) -> Option<&Unit> {
        self.units
            .iter()
            .find(|u| u.outs.iter().any(|s| s == signal))
    }

    /// How many units the part has.
    pub fn unit_count(&self) -> usize {
        self.units.len()
    }

    /// The pin the supply sits on. For a 14-pin part it is 14, for a 16-pin
    /// part 16, and so on.
    pub fn vcc_pin(&self) -> u8 {
        self.power_pin(true)
    }

    /// The pin the ground sits on.
    pub fn gnd_pin(&self) -> u8 {
        self.power_pin(false)
    }

    fn power_pin(&self, high: bool) -> u8 {
        let max = self.dip.iter().map(|(n, _)| *n).max().unwrap_or(0);
        if high {
            max
        } else {
            max / 2 + 1
        }
    }
}

/// The supply pins of a part: the pin numbers and the levels.
impl Power {
    /// The pin the supply sits on, derived from the pin map.
    pub fn vcc_pin(&self, dip_max: u8) -> u8 {
        dip_max
    }

    /// The pin the ground sits on.
    pub fn gnd_pin(&self, dip_max: u8) -> u8 {
        dip_max / 2 + 1
    }
}

/// A named set of parts.
#[derive(Clone, Debug, Default)]
pub struct Library {
    pub name: String,
    pub entries: Vec<Entry>,
}

impl Library {
    pub fn new(name: impl Into<String>) -> Library {
        Library {
            name: name.into(),
            entries: Vec::new(),
        }
    }

    pub fn find(&self, key: &str) -> Option<&Entry> {
        self.entries.iter().find(|e| e.key == key)
    }

    /// Write the library to the JSON shape [`Library::from_json`] reads.
    pub fn to_json_value(&self) -> Json {
        let mut root = std::collections::BTreeMap::new();
        root.insert(
            "format".into(),
            Json::Str("wireasm-component-library".into()),
        );
        root.insert("version".into(), Json::Str("1".into()));
        root.insert("name".into(), Json::Str(self.name.clone()));
        let items = self.entries.iter().map(entry_to_json).collect();
        root.insert("components".into(), Json::Arr(items));
        Json::Obj(root)
    }

    pub fn from_json_value(v: &Json) -> Result<Library, String> {
        let format = v.get("format").and_then(|x| x.as_str()).unwrap_or("");
        if !format.is_empty() && format != "wireasm-component-library" {
            return Err(format!("unknown library format `{format}`"));
        }
        let mut lib = Library::new(
            v.get("name")
                .and_then(|x| x.as_str())
                .unwrap_or("library")
                .to_string(),
        );
        let Some(Json::Arr(items)) = v.get("components") else {
            return Err("library has no 'components' array".into());
        };
        for item in items {
            lib.entries.push(entry_from_json(item)?);
        }
        Ok(lib)
    }
}

fn entry_to_json(e: &Entry) -> Json {
    let mut m = std::collections::BTreeMap::new();
    m.insert("key".into(), Json::Str(e.key.clone()));
    m.insert("title".into(), Json::Str(e.title.clone()));
    m.insert("family".into(), Json::Str(e.family.key().into()));
    m.insert("prefix".into(), Json::Str(e.prefix.clone()));
    let units = e
        .units
        .iter()
        .map(|u| {
            let mut um = std::collections::BTreeMap::new();
            um.insert("func".into(), Json::Str(u.func.key().into()));
            if let Some(bits) = u.sel_bits {
                um.insert("selBits".into(), Json::Num(bits as f64));
            }
            if u.has_enable {
                um.insert("enable".into(), Json::Bool(true));
            }
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
    let dip = e
        .dip
        .iter()
        .map(|(n, s)| {
            let mut dm = std::collections::BTreeMap::new();
            dm.insert("pin".into(), Json::Num(*n as f64));
            dm.insert("name".into(), Json::Str(s.clone()));
            Json::Obj(dm)
        })
        .collect();
    m.insert("dip".into(), Json::Arr(dip));
    let mut pm = std::collections::BTreeMap::new();
    pm.insert("vcc".into(), Json::Str(e.power.vcc.clone()));
    pm.insert("gnd".into(), Json::Str(e.power.gnd.clone()));
    pm.insert("vdd".into(), Json::Num(e.power.vdd));
    pm.insert("vss".into(), Json::Num(e.power.vss));
    pm.insert("vth".into(), Json::Num(e.power.vth));
    m.insert("power".into(), Json::Obj(pm));
    if let Some(ma) = e.ma {
        m.insert("ma".into(), Json::Num(ma));
    }
    if let Some(device) = e.device {
        m.insert("device".into(), Json::Str(device.key().into()));
    }
    Json::Obj(m)
}

fn entry_from_json(v: &Json) -> Result<Entry, String> {
    let key = v
        .get("key")
        .and_then(|x| x.as_str())
        .ok_or("component without a key")?
        .to_string();
    let family = v
        .get("family")
        .and_then(|x| x.as_str())
        .and_then(Family::from_key)
        .unwrap_or(Family::Basic);
    let mut units = Vec::new();
    if let Some(Json::Arr(items)) = v.get("units") {
        for item in items {
            let func = item
                .get("func")
                .and_then(|x| x.as_str())
                .and_then(Func::from_key)
                .ok_or_else(|| format!("component `{key}` has an unknown function"))?;
            let ins = string_list(item.get("ins"));
            let outs = string_list(item.get("outs"));
            units.push(Unit {
                func,
                ins,
                outs,
                sel_bits: item.get("selBits").and_then(|x| x.as_u64()).map(|v| v as u8),
                has_enable: item.get("enable").and_then(|x| x.as_bool()).unwrap_or(false),
            });
        }
    }
    let mut dip = Vec::new();
    if let Some(Json::Arr(items)) = v.get("dip") {
        for item in items {
            let Some(num) = item.get("pin").and_then(|x| x.as_u64()) else {
                continue;
            };
            let Some(name) = item.get("name").and_then(|x| x.as_str()) else {
                continue;
            };
            dip.push((num as u8, name.to_string()));
        }
    }
    let power = match v.get("power") {
        Some(p) => Power {
            vcc: p
                .get("vcc")
                .and_then(|x| x.as_str())
                .unwrap_or("VCC")
                .to_string(),
            gnd: p
                .get("gnd")
                .and_then(|x| x.as_str())
                .unwrap_or("GND")
                .to_string(),
            vdd: p.get("vdd").and_then(|x| x.as_f64()).unwrap_or(5.0),
            vss: p.get("vss").and_then(|x| x.as_f64()).unwrap_or(0.0),
            vth: p.get("vth").and_then(|x| x.as_f64()).unwrap_or(2.5),
        },
        None => Power::default(),
    };
    Ok(Entry {
        key,
        title: v
            .get("title")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string(),
        family,
        prefix: v
            .get("prefix")
            .and_then(|x| x.as_str())
            .unwrap_or("U")
            .to_string(),
        units,
        dip,
        power,
        ma: v.get("ma").and_then(|x| x.as_f64()),
        device: v
            .get("device")
            .and_then(|x| x.as_str())
            .and_then(Device::from_key),
    })
}

fn string_list(v: Option<&Json>) -> Vec<String> {
    match v {
        Some(Json::Arr(items)) => items
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect(),
        _ => Vec::new(),
    }
}

/// The catalogue that ships with the program.
pub fn builtin() -> Library {
    crate::library_data::all()
}

/// The catalogue indexed by key.
pub fn builtin_index() -> BTreeMap<String, Entry> {
    builtin()
        .entries
        .into_iter()
        .map(|e| (e.key.clone(), e))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalogue_is_large() {
        let lib = builtin();
        let mut counts = BTreeMap::new();
        for e in &lib.entries {
            *counts.entry(e.family).or_insert(0usize) += 1;
        }
        println!("{counts:?}");
        assert!(counts[&Family::Ttl] >= 40);
        assert!(counts[&Family::TtlSchottky] >= 20);
        assert!(counts[&Family::Cmos] >= 15);
    }

    #[test]
    fn builtin_catalogue_has_families() {
        let lib = builtin();
        assert!(
            lib.entries.len() > 100,
            "only {} entries",
            lib.entries.len()
        );
        for family in [
            Family::Basic,
            Family::Ttl,
            Family::TtlSchottky,
            Family::Cmos,
            Family::Analog,
            Family::Discrete,
        ] {
            assert!(
                lib.entries.iter().any(|e| e.family == family),
                "no {family:?} parts"
            );
        }
    }

    #[test]
    fn keys_are_unique() {
        let lib = builtin();
        let mut seen = std::collections::BTreeSet::new();
        for e in &lib.entries {
            assert!(seen.insert(e.key.clone()), "duplicate key {}", e.key);
        }
    }

    #[test]
    fn units_match_their_pins() {
        let lib = builtin();
        let mut missing: Vec<String> = Vec::new();
        for e in &lib.entries {
            if e.units.is_empty() {
                continue;
            }
            for u in &e.units {
                for name in u.ins.iter().chain(u.outs.iter()) {
                    if !e.dip.iter().any(|(_, n)| n == name) {
                        missing.push(format!("{}: {name}", e.key));
                    }
                }
            }
        }
        assert!(missing.is_empty(), "signals without a pin: {missing:?}");
    }

    #[test]
    fn json_round_trip() {
        let lib = builtin();
        let text = lib.to_json_value().stringify_pretty();
        let v = Json::parse(&text).unwrap();
        let back = Library::from_json_value(&v).unwrap();
        assert_eq!(back.entries.len(), lib.entries.len());
        assert_eq!(back.find("7400").map(|e| e.units.len()), Some(4));
    }

    #[test]
    fn a_library_can_be_shared() {
        // Import a library with one part and check that it lands intact.
        let mut lib = Library::new("mine");
        lib.entries.push(Entry {
            key: "MYGATE".to_string(),
            title: "My gate".to_string(),
            family: Family::Ttl,
            prefix: "U".to_string(),
            units: vec![Unit::new(Func::And2, &["A", "B"], &["Y"])],
            dip: vec![
                (1, "A".to_string()),
                (2, "B".to_string()),
                (3, "Y".to_string()),
            ],
            power: Power::ttl(),
            ma: None,
            device: None,
        });
        let text = lib.to_json_value().stringify_pretty();
        let back = Library::from_json_value(&Json::parse(&text).unwrap()).unwrap();
        let entry = back.find("MYGATE").expect("the part survived");
        assert_eq!(entry.units.len(), 1);
        assert_eq!(entry.units[0].func, Func::And2);
        assert_eq!(entry.power.vth, 1.4);
    }

    #[test]
    fn every_family_has_parts() {
        let lib = builtin();
        for family in [
            Family::Basic,
            Family::Ttl,
            Family::TtlSchottky,
            Family::Ecl,
            Family::Cmos,
            Family::Analog,
            Family::Discrete,
        ] {
            assert!(
                lib.entries.iter().any(|e| e.family == family),
                "the {family:?} family is empty"
            );
        }
    }
}
