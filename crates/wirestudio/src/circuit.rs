//! Circuit workspace: a schematic editor with analog and mixed-signal simulation.
//!
//! Parts are dropped on a grid, pins are wired into nets, and the schematic is
//! converted into a [`wireasm_sim::Circuit`]. The same crate then runs DC,
//! transient and AC analyses. The current graph can be attached as a virtual
//! MCU, so firmware drives real pins on the schematic (Proteus style).
//!
//! The document model lives in `wireasm_sim::schematic`; this file is the
//! editor: palette, canvas, inspector and scope.

use std::collections::BTreeMap;
use std::path::PathBuf;

use eframe::egui::{self, Color32, FontId, Pos2, Rect, RichText, Sense, Stroke, StrokeKind, Vec2};
use wireasm_core::model::{Graph, Target};
use wireasm_sim::schematic::{
    gate_short, Analysis, PartKind, PinAddr, Schematic, SimResult, WaveKind,
};
use wireasm_sim::{McuBinding, Simulator, Solution, Trace, VirtualMcu};

use crate::app::{View, WireStudio};
use crate::i18n::t;

const BG: Color32 = Color32::from_rgb(16, 18, 24);
const GRID: Color32 = Color32::from_rgb(36, 40, 52);
const GRID_STRONG: Color32 = Color32::from_rgb(52, 58, 74);
const WIRE: Color32 = Color32::from_rgb(104, 196, 172);
const WIRE_SEL: Color32 = Color32::from_rgb(250, 200, 90);
const PART_BODY: Color32 = Color32::from_rgb(32, 36, 48);
const PART_BODY_SEL: Color32 = Color32::from_rgb(40, 50, 68);
const PART_BORDER: Color32 = Color32::from_rgb(86, 94, 116);
const PART_BORDER_SEL: Color32 = Color32::from_rgb(96, 165, 250);
const PIN: Color32 = Color32::from_rgb(206, 214, 228);
const PIN_HOT: Color32 = Color32::from_rgb(250, 200, 90);
const TEXT: Color32 = Color32::from_rgb(226, 231, 240);
const MUTED: Color32 = Color32::from_rgb(150, 158, 176);
const PROBE: Color32 = Color32::from_rgb(235, 130, 90);
const ERROR: Color32 = Color32::from_rgb(235, 105, 105);
const OK: Color32 = Color32::from_rgb(120, 200, 140);

/// Body size of a placed part, in world units.
const PART_W: f32 = 84.0;
const PART_H: f32 = 40.0;
/// Radius of a pin dot, in world units.
const PIN_R: f32 = 3.6;

const SERIES_COLORS: [Color32; 6] = [
    Color32::from_rgb(96, 165, 250),
    Color32::from_rgb(250, 176, 90),
    Color32::from_rgb(120, 210, 150),
    Color32::from_rgb(230, 120, 200),
    Color32::from_rgb(200, 200, 110),
    Color32::from_rgb(150, 150, 230),
];

/// Parts offered in the palette, in menu order.
pub const PALETTE: &[PartKind] = &[
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

/// Localization key of a part name.
pub fn part_label_key(kind: PartKind) -> &'static str {
    match kind {
        PartKind::Resistor => "part.resistor",
        PartKind::Capacitor => "part.capacitor",
        PartKind::Inductor => "part.inductor",
        PartKind::Vsource => "part.vsource",
        PartKind::Isource => "part.isource",
        PartKind::Diode => "part.diode",
        PartKind::Led => "part.led",
        PartKind::Switch => "part.switch",
        PartKind::Gate => "part.gate",
        PartKind::McuPin => "part.mcu_pin",
        PartKind::Ground => "part.ground",
    }
}

/// Localization key of a waveform name.
pub fn wave_label_key(wave: WaveKind) -> &'static str {
    match wave {
        WaveKind::Dc => "wave.dc",
        WaveKind::Sine => "wave.sine",
        WaveKind::Pulse => "wave.pulse",
    }
}

/// Localization key of an analysis name.
pub fn analysis_label_key(analysis: Analysis) -> &'static str {
    match analysis {
        Analysis::Dc => "circuit.dc",
        Analysis::Tran => "circuit.tran",
        Analysis::Ac => "circuit.ac",
    }
}

/// Current selection in the schematic.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sel {
    Part(usize),
    Wire(usize),
}

#[derive(Clone, Copy, Debug)]
pub struct DragPart {
    pub index: usize,
    pub grab: Vec2,
    pub moved: bool,
}

/// The editor state around a [`Schematic`] document.
pub struct CircuitDoc {
    pub model: Schematic,
    pub view: View,
    pub selected: Option<Sel>,
    pub pending: Option<PinAddr>,
    pub drag: Option<DragPart>,
    pub panning: bool,
    pub fit_requested: bool,
    pub rect: Rect,
    pub result: Option<SimResult>,
    pub uart: String,
    pub error: String,
    pub status: String,
    pub visible: BTreeMap<String, bool>,
    pub path: Option<PathBuf>,
    pub dirty: bool,
}

impl Default for CircuitDoc {
    fn default() -> Self {
        CircuitDoc {
            model: Schematic::divider(),
            view: View::default(),
            selected: None,
            pending: None,
            drag: None,
            panning: false,
            fit_requested: true,
            rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 600.0)),
            result: None,
            uart: String::new(),
            error: String::new(),
            status: String::new(),
            visible: BTreeMap::new(),
            path: None,
            dirty: false,
        }
    }
}

// ------------------------------------------------------------------- ui

impl WireStudio {
    /// The whole circuit workspace: palette, canvas, inspector and scope.
    pub fn circuit_ui(&mut self, ui: &mut egui::Ui) {
        self.circuit_palette(ui);
        self.circuit_inspector(ui);
        self.circuit_scope(ui);
        egui::CentralPanel::default().show(ui, |ui| self.circuit_canvas(ui));
    }

    fn circuit_palette(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        egui::Panel::left("circuit_palette")
            .resizable(true)
            .default_size(210.0)
            .min_size(160.0)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.heading(s("circuit.parts"));
                ui.separator();
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for kind in PALETTE {
                        let label = s(part_label_key(*kind));
                        if ui.selectable_label(false, label).clicked() {
                            let center = self.circuit_center();
                            let idx = self.circuit.model.add_part(*kind, center.x, center.y);
                            self.circuit.selected = Some(Sel::Part(idx));
                            self.circuit.dirty = true;
                            self.status = self.tr("status.circuit_added").to_string();
                        }
                    }
                    ui.separator();
                    if ui.button(s("circuit.fit")).clicked() {
                        self.circuit.fit_requested = true;
                    }
                    if ui.button(s("circuit.clear")).clicked() {
                        let mut model = Schematic::empty();
                        model.name = "circuit".to_string();
                        self.circuit.model = model;
                        self.circuit.result = None;
                        self.circuit.dirty = true;
                        self.status = self.tr("status.circuit_cleared").to_string();
                    }
                });
            });
    }

    fn circuit_center(&self) -> Pos2 {
        self.circuit
            .view
            .s2w(self.circuit.rect, self.circuit.rect.center())
    }

    fn circuit_inspector(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("circuit_inspector")
            .resizable(true)
            .default_size(280.0)
            .min_size(220.0)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.circuit_selection(ui);
                    ui.add_space(8.0);
                    ui.separator();
                    self.circuit_analysis_panel(ui);
                    ui.add_space(8.0);
                    ui.separator();
                    self.circuit_probe_panel(ui);
                });
            });
    }

    fn circuit_selection(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        ui.heading(s("circuit.inspector"));
        let Some(Sel::Part(idx)) = self.circuit.selected else {
            ui.label(RichText::new(s("circuit.no_selection")).weak());
            return;
        };
        let Some(part) = self.circuit.model.parts.get(idx).cloned() else {
            return;
        };
        ui.horizontal(|ui| {
            ui.label(s("circuit.name"));
            let mut id = part.id.clone();
            if ui
                .add(egui::TextEdit::singleline(&mut id).desired_width(110.0))
                .changed()
            {
                let id = id.trim().to_string();
                if !id.is_empty() && !self.circuit.model.parts.iter().any(|p| p.id == id) {
                    self.circuit.model.parts[idx].id = id;
                    self.circuit.dirty = true;
                }
            }
        });
        ui.label(RichText::new(s(part_label_key(part.kind))).weak());

        let mut part = part;
        let mut changed = false;
        match part.kind {
            PartKind::Resistor | PartKind::Inductor => {
                ui.horizontal(|ui| {
                    ui.label(s("circuit.value"));
                    let mut v = part.value;
                    let resp = ui.add(
                        egui::DragValue::new(&mut v)
                            .speed(10.0)
                            .range(0.0..=1e9)
                            .suffix(" Ω"),
                    );
                    if resp.changed() {
                        part.value = v;
                        changed = true;
                    }
                });
            }
            PartKind::Capacitor => {
                ui.horizontal(|ui| {
                    ui.label(s("circuit.value"));
                    let mut v = part.value;
                    let resp = ui.add(
                        egui::DragValue::new(&mut v)
                            .speed(1e-8)
                            .range(1e-15..=1.0)
                            .suffix(" F"),
                    );
                    if resp.changed() {
                        part.value = v;
                        changed = true;
                    }
                });
            }
            PartKind::Vsource | PartKind::Isource => {
                ui.horizontal(|ui| {
                    ui.label(s("circuit.wave"));
                    egui::ComboBox::from_id_salt("circuit_wave")
                        .selected_text(s(wave_label_key(part.wave)))
                        .show_ui(ui, |ui| {
                            for w in [WaveKind::Dc, WaveKind::Sine, WaveKind::Pulse] {
                                if ui
                                    .selectable_label(part.wave == w, s(wave_label_key(w)))
                                    .clicked()
                                {
                                    part.wave = w;
                                    changed = true;
                                }
                            }
                        });
                });
                if part.wave == WaveKind::Dc {
                    ui.horizontal(|ui| {
                        ui.label(s("circuit.value"));
                        let mut v = part.value;
                        let resp = ui.add(
                            egui::DragValue::new(&mut v)
                                .speed(0.1)
                                .range(-1000.0..=1000.0)
                                .suffix(if part.kind == PartKind::Vsource {
                                    " V"
                                } else {
                                    " A"
                                }),
                        );
                        if resp.changed() {
                            part.value = v;
                            changed = true;
                        }
                    });
                } else {
                    ui.horizontal(|ui| {
                        ui.label(s("circuit.amplitude"));
                        let mut v = part.amp;
                        if ui
                            .add(
                                egui::DragValue::new(&mut v)
                                    .speed(0.1)
                                    .range(-1000.0..=1000.0)
                                    .suffix(" V"),
                            )
                            .changed()
                        {
                            part.amp = v;
                            changed = true;
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label(s("circuit.frequency"));
                        let mut f = part.freq;
                        if ui
                            .add(
                                egui::DragValue::new(&mut f)
                                    .speed(1.0)
                                    .range(0.01..=1e7)
                                    .suffix(" Hz"),
                            )
                            .changed()
                        {
                            part.freq = f;
                            changed = true;
                        }
                    });
                    if part.wave == WaveKind::Pulse {
                        ui.horizontal(|ui| {
                            ui.label(s("circuit.duty"));
                            let mut d = part.duty;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut d)
                                        .speed(0.01)
                                        .range(0.0..=1.0)
                                        .suffix(" %"),
                                )
                                .changed()
                            {
                                part.duty = d;
                                changed = true;
                            }
                        });
                    }
                }
            }
            PartKind::Diode => {
                ui.label(RichText::new(s("part.diode")).weak());
            }
            PartKind::Led => {
                ui.horizontal(|ui| {
                    ui.label(s("part.led_vf"));
                    let mut v = part.value;
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .speed(0.05)
                                .range(0.5..=4.0)
                                .suffix(" V"),
                        )
                        .changed()
                    {
                        part.value = v;
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(s("part.led_if"));
                    let mut v = part.value2;
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .speed(0.001)
                                .range(0.001..=1.0)
                                .suffix(" A"),
                        )
                        .changed()
                    {
                        part.value2 = v;
                        changed = true;
                    }
                });
            }
            PartKind::Switch => {
                ui.horizontal(|ui| {
                    let mut on = part.on;
                    if ui.checkbox(&mut on, s("part.switch_on")).changed() {
                        part.on = on;
                        changed = true;
                    }
                });
            }
            PartKind::Gate => {
                ui.horizontal(|ui| {
                    ui.label(s("part.gate_kind"));
                    egui::ComboBox::from_id_salt("circuit_gate")
                        .selected_text(gate_short(part.gate))
                        .show_ui(ui, |ui| {
                            for g in [
                                wireasm_sim::GateKind::And,
                                wireasm_sim::GateKind::Or,
                                wireasm_sim::GateKind::Nand,
                                wireasm_sim::GateKind::Nor,
                                wireasm_sim::GateKind::Xor,
                                wireasm_sim::GateKind::Xnor,
                                wireasm_sim::GateKind::Not,
                                wireasm_sim::GateKind::Buffer,
                            ] {
                                if ui.selectable_label(part.gate == g, gate_short(g)).clicked() {
                                    part.gate = g;
                                    changed = true;
                                }
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label(s("circuit.value"));
                    let mut v = part.value;
                    if ui
                        .add(
                            egui::DragValue::new(&mut v)
                                .speed(0.1)
                                .range(0.5..=24.0)
                                .suffix(" V"),
                        )
                        .changed()
                    {
                        part.value = v;
                        changed = true;
                    }
                });
            }
            PartKind::McuPin => {
                ui.horizontal(|ui| {
                    ui.label(s("part.mcu_pin_name"));
                    let mut pin = part.pin_name.clone();
                    if ui
                        .add(egui::TextEdit::singleline(&mut pin).desired_width(110.0))
                        .changed()
                    {
                        part.pin_name = pin.trim().to_string();
                        changed = true;
                    }
                });
                ui.label(RichText::new(s("circuit.mcu_pin_hint")).weak());
            }
            PartKind::Ground => {
                ui.label(RichText::new(s("part.ground")).weak());
            }
        }

        if changed {
            self.circuit.model.parts[idx] = part;
            self.circuit.dirty = true;
        }

        ui.horizontal(|ui| {
            if ui.button(s("circuit.delete")).clicked() {
                self.circuit.selected = Some(Sel::Part(idx));
                self.circuit_delete_selected();
            }
        });
    }

    fn circuit_analysis_panel(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        ui.heading(s("circuit.analysis"));
        ui.horizontal(|ui| {
            for a in [Analysis::Dc, Analysis::Tran, Analysis::Ac] {
                if ui
                    .selectable_label(self.circuit.model.analysis == a, s(analysis_label_key(a)))
                    .clicked()
                {
                    self.circuit.model.analysis = a;
                }
            }
        });
        match self.circuit.model.analysis {
            Analysis::Tran => {
                ui.horizontal(|ui| {
                    ui.label(s("circuit.dt"));
                    let mut dt = self.circuit.model.opts.dt;
                    if ui
                        .add(
                            egui::DragValue::new(&mut dt)
                                .speed(1e-6)
                                .range(1e-9..=1.0)
                                .suffix(" s"),
                        )
                        .changed()
                    {
                        self.circuit.model.opts.dt = dt;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(s("circuit.stop"));
                    let mut stop = self.circuit.model.opts.t_stop;
                    if ui
                        .add(
                            egui::DragValue::new(&mut stop)
                                .speed(1e-4)
                                .range(1e-6..=1e4)
                                .suffix(" s"),
                        )
                        .changed()
                    {
                        self.circuit.model.opts.t_stop = stop;
                    }
                });
            }
            Analysis::Ac => {
                ui.horizontal(|ui| {
                    ui.label(s("circuit.fstart"));
                    let mut f = self.circuit.model.f_start;
                    if ui
                        .add(
                            egui::DragValue::new(&mut f)
                                .speed(1.0)
                                .range(0.01..=1e9)
                                .suffix(" Hz"),
                        )
                        .changed()
                    {
                        self.circuit.model.f_start = f;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(s("circuit.fstop"));
                    let mut f = self.circuit.model.f_stop;
                    if ui
                        .add(
                            egui::DragValue::new(&mut f)
                                .speed(1.0)
                                .range(0.02..=1e9)
                                .suffix(" Hz"),
                        )
                        .changed()
                    {
                        self.circuit.model.f_stop = f;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(s("circuit.points"));
                    let mut p = self.circuit.model.ac_points;
                    if ui
                        .add(egui::DragValue::new(&mut p).speed(1.0).range(2..=2000))
                        .changed()
                    {
                        self.circuit.model.ac_points = p;
                    }
                });
            }
            Analysis::Dc => {}
        }

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.circuit.model.mcu_on, s("circuit.mcu"));
            let mut vdd = self.circuit.model.mcu_vdd;
            if ui
                .add(
                    egui::DragValue::new(&mut vdd)
                        .speed(0.1)
                        .range(1.0..=5.5)
                        .suffix(" V"),
                )
                .changed()
            {
                self.circuit.model.mcu_vdd = vdd;
            }
        });
        if self.circuit.model.mcu_on {
            let target = Target::parse(&self.graph.target);
            let is_mcu = target.map(|t| t.is_mcu()).unwrap_or(false);
            if !is_mcu {
                ui.label(RichText::new(s("circuit.mcu_bad_target")).color(ERROR));
                // Let the user pick the MCU family right here instead of
                // walking back to the graph workspace.
                let mut pick = self.graph.target.clone();
                let mut changed_target: Option<Target> = None;
                egui::ComboBox::from_id_salt("circuit_mcu_target")
                    .selected_text(pick.clone())
                    .width(170.0)
                    .show_ui(ui, |ui| {
                        ui.label(RichText::new(s("circuit.mcu_target")).weak());
                        for t in [
                            Target::AvrAtmega328p,
                            Target::Stm32F411,
                            Target::Esp32,
                            Target::Esp8266,
                        ] {
                            if ui.selectable_label(pick == t.as_str(), t.label()).clicked() {
                                pick = t.as_str().to_string();
                                changed_target = Some(t);
                            }
                        }
                    });
                if let Some(t) = changed_target {
                    self.graph.target = t.as_str().to_string();
                    self.touch();
                    self.status = self.tr("status.target_changed").replace("{}", t.as_str());
                }
            } else {
                ui.label(RichText::new(s("circuit.mcu_hint")).weak());
                // The graph must drive the pins it claims to drive.
                let has_mcu_nodes = self.graph.nodes.iter().any(|n| n.kind.starts_with("Mcu"));
                if !has_mcu_nodes {
                    ui.label(RichText::new(s("circuit.mcu_no_nodes")).color(ERROR));
                    if ui.button(s("circuit.mcu_load_blink")).clicked() {
                        self.load_blink_example();
                    }
                }
                match self.circuit.model.mcu_bindings() {
                    Ok(bindings) => {
                        if bindings.is_empty() {
                            ui.label(RichText::new(s("circuit.mcu_no_pins")).weak());
                        }
                        for b in bindings {
                            ui.label(RichText::new(format!("{} → {}", b.pin, b.node)).monospace());
                        }
                    }
                    Err(e) => {
                        ui.label(RichText::new(e).color(ERROR));
                    }
                }
            }
        }

        if ui.button(s("circuit.run")).clicked() {
            self.run_circuit();
        }
        if !self.circuit.error.is_empty() {
            ui.colored_label(ERROR, &self.circuit.error);
        }
    }

    fn circuit_probe_panel(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        ui.heading(s("circuit.probes"));
        let probes = self.circuit.model.probes.clone();
        if probes.is_empty() {
            ui.label(RichText::new(s("circuit.probe_hint")).weak());
        }
        for (i, p) in probes.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&p.name).monospace());
                if ui.small_button("×").clicked() {
                    self.circuit.model.remove_probe(i);
                    self.circuit.dirty = true;
                }
            });
        }
        let pins = self
            .circuit
            .model
            .parts
            .iter()
            .enumerate()
            .flat_map(|(i, part)| {
                part.pins()
                    .into_iter()
                    .map(move |pin| (i, pin.to_string()))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut add: Option<(usize, String)> = None;
        for (i, pin) in pins {
            let already = self
                .circuit
                .model
                .probes
                .iter()
                .any(|p| p.part == i && p.pin == pin);
            if already {
                continue;
            }
            let label = format!("{}·{}", self.circuit.model.parts[i].id, pin);
            if ui.small_button(label).clicked() {
                add = Some((i, pin));
            }
        }
        if let Some((part, pin)) = add {
            let nets = self.circuit.model.nets();
            let name = nets
                .get(&(part, pin.clone()))
                .cloned()
                .unwrap_or_else(|| format!("{}_{}", self.circuit.model.parts[part].id, pin));
            self.circuit.model.probe(part, &pin, &name);
            self.circuit.dirty = true;
        }
    }

    fn circuit_scope(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        egui::Panel::bottom("circuit_scope")
            .resizable(true)
            .default_size(240.0)
            .min_size(140.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(s("circuit.scope")).strong());
                    ui.label(
                        RichText::new(s(analysis_label_key(self.circuit.model.analysis))).weak(),
                    );
                    if let Some(result) = &self.circuit.result {
                        let text = match result {
                            SimResult::Dc(sol) => format!("{} nodes", sol.nodes.len()),
                            SimResult::Tran(tr) => format!("{} points", tr.len()),
                            SimResult::Ac(sw) => format!("{} points", sw.frequency.len()),
                        };
                        ui.label(RichText::new(text).weak());
                    }
                    if !self.circuit.status.is_empty() {
                        ui.label(RichText::new(&self.circuit.status).color(OK).weak());
                    }
                });
                ui.separator();
                let available = ui.available_size();
                let (rect, _) = ui.allocate_exact_size(available, egui::Sense::hover());
                let result = self.circuit.result.clone();
                match result {
                    None => {
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            s("circuit.empty"),
                            FontId::monospace(13.0),
                            MUTED,
                        );
                    }
                    Some(SimResult::Dc(sol)) => {
                        draw_dc_table(ui, rect, &sol);
                    }
                    Some(SimResult::Tran(tr)) => {
                        self.draw_tran(ui, rect, &tr);
                    }
                    Some(SimResult::Ac(sw)) => {
                        self.draw_ac(ui, rect, &sw);
                    }
                }
                if !self.circuit.uart.is_empty() {
                    ui.separator();
                    ui.label(RichText::new(s("circuit.uart")).weak());
                    ui.add(
                        egui::Label::new(RichText::new(&self.circuit.uart).monospace())
                            .selectable(true),
                    );
                }
            });
    }

    fn draw_tran(&mut self, ui: &mut egui::Ui, rect: Rect, trace: &Trace) {
        let names = trace
            .names()
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>();
        let mut series: Vec<(String, Vec<f64>)> = Vec::new();
        for n in &names {
            if !self.circuit.visible.get(n).copied().unwrap_or(true) {
                continue;
            }
            if let Some(v) = trace.series.get(n) {
                series.push((n.clone(), v.clone()));
            }
        }
        if series.is_empty() {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.tr("circuit.empty"),
                FontId::monospace(13.0),
                MUTED,
            );
            return;
        }
        draw_plot(ui, rect, &trace.time, &series, false, "t, s", "V");
        self.legend(ui, &names);
    }

    fn draw_ac(&mut self, ui: &mut egui::Ui, rect: Rect, sweep: &wireasm_sim::AcSweep) {
        let names = sweep.series.keys().cloned().collect::<Vec<_>>();
        let mut series: Vec<(String, Vec<f64>)> = Vec::new();
        for n in &names {
            if !self.circuit.visible.get(n).copied().unwrap_or(true) {
                continue;
            }
            let mag = sweep.magnitude(n);
            let db = mag
                .iter()
                .map(|m| 20.0 * m.max(1e-18).log10())
                .collect::<Vec<f64>>();
            series.push((n.clone(), db));
        }
        if series.is_empty() {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                self.tr("circuit.empty"),
                FontId::monospace(13.0),
                MUTED,
            );
            return;
        }
        draw_plot(ui, rect, &sweep.frequency, &series, true, "f, Hz", "dB");
        self.legend(ui, &names);
    }

    fn legend(&mut self, ui: &mut egui::Ui, names: &[String]) {
        ui.horizontal_wrapped(|ui| {
            for (i, n) in names.iter().enumerate() {
                let color = SERIES_COLORS[i % SERIES_COLORS.len()];
                let on = self.circuit.visible.get(n).copied().unwrap_or(true);
                let text = RichText::new(n).color(if on { color } else { MUTED });
                if ui.selectable_label(on, text).clicked() {
                    self.circuit.visible.insert(n.clone(), !on);
                }
            }
        });
    }

    fn circuit_canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        self.circuit.rect = rect;
        if self.circuit.fit_requested {
            self.circuit_fit(rect);
            self.circuit.fit_requested = false;
        }
        painter.rect_filled(rect, 0.0, BG);
        self.draw_circuit_grid(&painter, rect);

        for (i, w) in self.circuit.model.wires.iter().enumerate() {
            let a = self.circuit.model.pin_pos(w.a.part, &w.a.pin);
            let b = self.circuit.model.pin_pos(w.b.part, &w.b.pin);
            let selected = self.circuit.selected == Some(Sel::Wire(i));
            let color = if selected { WIRE_SEL } else { WIRE };
            let points = elbow(a, b);
            for pair in points.windows(2) {
                painter.line_segment(
                    [
                        self.circuit.view.w2s(rect, pair[0]),
                        self.circuit.view.w2s(rect, pair[1]),
                    ],
                    Stroke::new(2.0, color),
                );
            }
        }

        for i in 0..self.circuit.model.parts.len() {
            let selected = self.circuit.selected == Some(Sel::Part(i));
            self.draw_part(&painter, rect, i, selected);
        }

        for i in 0..self.circuit.model.parts.len() {
            let part = self.circuit.model.parts[i].clone();
            for pin in part.pins() {
                let pos = self.circuit.model.pin_pos(i, pin);
                let screen = self.circuit.view.w2s(rect, pos);
                let hot = self
                    .circuit
                    .pending
                    .as_ref()
                    .map(|p| p.part == i && p.pin == pin)
                    .unwrap_or(false);
                painter.circle_filled(screen, PIN_R * self.circuit.view.zoom.max(0.6), PIN);
                if hot {
                    painter.circle_stroke(
                        screen,
                        (PIN_R + 3.0) * self.circuit.view.zoom.max(0.6),
                        Stroke::new(1.6, PIN_HOT),
                    );
                }
            }
        }

        for p in self.circuit.model.probes.clone() {
            let pos = self.circuit.model.pin_pos(p.part, &p.pin);
            let screen = self.circuit.view.w2s(rect, pos);
            painter.circle_stroke(screen, 7.0, Stroke::new(1.6, PROBE));
            painter.text(
                screen + Vec2::new(9.0, -9.0),
                egui::Align2::LEFT_BOTTOM,
                &p.name,
                FontId::monospace(11.0),
                PROBE,
            );
        }

        if let Some(link) = self.circuit.pending.clone() {
            let from = self.circuit.model.pin_pos(link.part, &link.pin);
            let screen = self.circuit.view.w2s(rect, from);
            if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
                painter.line_segment([screen, cursor], Stroke::new(1.6, PIN_HOT));
                painter.circle_filled(cursor, 3.0, PIN_HOT);
            }
        }

        self.circuit_input(ui, rect);
    }

    fn circuit_fit(&mut self, rect: Rect) {
        if self.circuit.model.parts.is_empty() {
            self.circuit.view.zoom = 1.0;
            self.circuit.view.pan = Vec2::new(-40.0, -40.0);
            return;
        }
        let mut min = Pos2::new(f32::MAX, f32::MAX);
        let mut max = Pos2::new(f32::MIN, f32::MIN);
        for i in 0..self.circuit.model.parts.len() {
            let r = self.circuit.model.body_rect(i);
            min = min.min(r.min);
            max = max.max(r.max);
        }
        let bw = (max.x - min.x + 160.0).max(320.0);
        let bh = (max.y - min.y + 160.0).max(240.0);
        let zoom = (rect.width() / bw).min(rect.height() / bh).clamp(0.35, 1.6);
        self.circuit.view.zoom = zoom;
        let center = Pos2::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5);
        self.circuit.view.pan = center.to_vec2() - (rect.size() * 0.5) / zoom;
    }

    fn draw_circuit_grid(&self, painter: &egui::Painter, rect: Rect) {
        let step = 20.0 * self.circuit.view.zoom;
        if step < 6.0 {
            return;
        }
        let start_x =
            rect.min.x - ((self.circuit.view.pan.x * self.circuit.view.zoom).rem_euclid(step));
        let start_y =
            rect.min.y - ((self.circuit.view.pan.y * self.circuit.view.zoom).rem_euclid(step));
        let mut x = start_x;
        while x < rect.right() {
            let mut y = start_y;
            while y < rect.bottom() {
                painter.circle_filled(egui::pos2(x, y), 1.0, GRID);
                y += step;
            }
            x += step;
        }
    }

    fn draw_part(&self, painter: &egui::Painter, rect: Rect, index: usize, selected: bool) {
        let Some(part) = self.circuit.model.parts.get(index) else {
            return;
        };
        let part = part.clone();
        let body = self.circuit.model.body_rect(index);
        let screen = Rect::from_min_max(
            self.circuit.view.w2s(rect, body.min),
            self.circuit.view.w2s(rect, body.max),
        );
        let fill = if selected { PART_BODY_SEL } else { PART_BODY };
        let border = if selected {
            PART_BORDER_SEL
        } else {
            PART_BORDER
        };
        let stroke = Stroke::new(if selected { 2.0 } else { 1.2 }, border);
        let zoom = self.circuit.view.zoom;

        match part.kind {
            PartKind::Ground => {
                let top = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "0"));
                let w = 14.0 * zoom;
                painter.line_segment(
                    [top, top + Vec2::new(0.0, 6.0 * zoom)],
                    Stroke::new(1.8, PIN),
                );
                painter.line_segment(
                    [
                        top + Vec2::new(-w, 6.0 * zoom),
                        top + Vec2::new(w, 6.0 * zoom),
                    ],
                    Stroke::new(1.8, PIN),
                );
                painter.line_segment(
                    [
                        top + Vec2::new(-w * 0.6, 11.0 * zoom),
                        top + Vec2::new(w * 0.6, 11.0 * zoom),
                    ],
                    Stroke::new(1.8, PIN),
                );
                painter.line_segment(
                    [
                        top + Vec2::new(-w * 0.3, 16.0 * zoom),
                        top + Vec2::new(w * 0.3, 16.0 * zoom),
                    ],
                    Stroke::new(1.8, PIN),
                );
            }
            PartKind::Diode | PartKind::Led => {
                let a = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "1"));
                let b = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "2"));
                let mid = a + (b - a) * 0.5;
                let h = 9.0 * zoom;
                let w = (b.x - a.x) * 0.22;
                painter.add(egui::Shape::convex_polygon(
                    vec![
                        mid + Vec2::new(-w, -h),
                        mid + Vec2::new(-w, h),
                        mid + Vec2::new(w * 0.4, 0.0),
                    ],
                    fill,
                    stroke,
                ));
                let bar = mid + Vec2::new(w * 0.5, 0.0);
                painter.line_segment(
                    [bar + Vec2::new(0.0, -h), bar + Vec2::new(0.0, h)],
                    Stroke::new(1.8, PIN),
                );
                painter.line_segment([a, mid + Vec2::new(-w, 0.0)], Stroke::new(1.6, PIN));
                painter.line_segment([bar, b], Stroke::new(1.6, PIN));
                if part.kind == PartKind::Led {
                    let tip = mid + Vec2::new(w * 0.2, 0.0);
                    let arrow = Stroke::new(1.4, Color32::from_rgb(250, 210, 120));
                    painter.line_segment(
                        [
                            tip + Vec2::new(0.0, -12.0 * zoom),
                            tip + Vec2::new(9.0 * zoom, -19.0 * zoom),
                        ],
                        arrow,
                    );
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            tip + Vec2::new(9.0 * zoom, -19.0 * zoom),
                            tip + Vec2::new(8.0 * zoom, -11.0 * zoom),
                            tip + Vec2::new(2.0 * zoom, -13.0 * zoom),
                        ],
                        Color32::from_rgb(250, 210, 120),
                        Stroke::NONE,
                    ));
                }
            }
            PartKind::Capacitor => {
                let a = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "1"));
                let b = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "2"));
                let mid = a + (b - a) * 0.5;
                let h = 10.0 * zoom;
                painter.line_segment([a, mid], Stroke::new(1.6, PIN));
                painter.line_segment([b, mid], Stroke::new(1.6, PIN));
                painter.line_segment(
                    [mid + Vec2::new(0.0, -h), mid + Vec2::new(0.0, h)],
                    Stroke::new(2.2, PIN),
                );
                painter.line_segment(
                    [
                        mid + Vec2::new(5.0 * zoom, -h),
                        mid + Vec2::new(5.0 * zoom, h),
                    ],
                    Stroke::new(2.2, PIN),
                );
            }
            PartKind::Inductor => {
                let a = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "1"));
                let b = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "2"));
                painter.line_segment([a, a + (b - a) * 0.2], Stroke::new(1.6, PIN));
                painter.line_segment([b, b - (b - a) * 0.2], Stroke::new(1.6, PIN));
                let r = 7.0 * zoom;
                for i in 0..3 {
                    let c = a + (b - a) * (0.3 + 0.2 * i as f32);
                    painter.circle_stroke(c, r, Stroke::new(1.6, PIN));
                }
            }
            PartKind::Resistor => {
                let a = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "1"));
                let b = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "2"));
                painter.line_segment([a, a + (b - a) * 0.22], Stroke::new(1.6, PIN));
                painter.line_segment([b, b - (b - a) * 0.22], Stroke::new(1.6, PIN));
                let r = Rect::from_min_max(a + (b - a) * 0.22, b - (b - a) * 0.22);
                painter.rect_filled(r, 3.0, fill);
                painter.rect_stroke(r, 3.0, stroke, StrokeKind::Middle);
            }
            PartKind::Vsource | PartKind::Isource => {
                let a = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "1"));
                let b = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "2"));
                let mid = a + (b - a) * 0.5;
                let r = 11.0 * zoom;
                painter.circle_filled(mid, r, fill);
                painter.circle_stroke(mid, r, stroke);
                painter.line_segment([a, mid - Vec2::new(r, 0.0)], Stroke::new(1.6, PIN));
                painter.line_segment([b, mid + Vec2::new(r, 0.0)], Stroke::new(1.6, PIN));
                if part.kind == PartKind::Vsource {
                    painter.line_segment(
                        [
                            mid + Vec2::new(-4.0, -6.0) * zoom,
                            mid + Vec2::new(4.0, -6.0) * zoom,
                        ],
                        Stroke::new(1.6, TEXT),
                    );
                    painter.line_segment(
                        [
                            mid + Vec2::new(0.0, -10.0) * zoom,
                            mid + Vec2::new(0.0, -2.0) * zoom,
                        ],
                        Stroke::new(1.6, TEXT),
                    );
                    painter.line_segment(
                        [
                            mid + Vec2::new(0.0, 2.0) * zoom,
                            mid + Vec2::new(0.0, 10.0) * zoom,
                        ],
                        Stroke::new(1.6, TEXT),
                    );
                    painter.line_segment(
                        [
                            mid + Vec2::new(-4.0, 6.0) * zoom,
                            mid + Vec2::new(4.0, 6.0) * zoom,
                        ],
                        Stroke::new(1.6, TEXT),
                    );
                } else {
                    painter.line_segment(
                        [
                            mid + Vec2::new(0.0, -9.0) * zoom,
                            mid + Vec2::new(0.0, 9.0) * zoom,
                        ],
                        Stroke::new(1.6, TEXT),
                    );
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            mid + Vec2::new(0.0, 11.0 * zoom),
                            mid + Vec2::new(-4.0, 3.0) * zoom,
                            mid + Vec2::new(4.0, 3.0) * zoom,
                        ],
                        TEXT,
                        Stroke::NONE,
                    ));
                }
            }
            PartKind::Switch => {
                let a = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "1"));
                let b = self
                    .circuit
                    .view
                    .w2s(rect, self.circuit.model.pin_pos(index, "2"));
                painter.line_segment([a, a + (b - a) * 0.25], Stroke::new(1.6, PIN));
                painter.line_segment([b, b - (b - a) * 0.25], Stroke::new(1.6, PIN));
                let from = a + (b - a) * 0.25;
                let to = if part.on {
                    b - (b - a) * 0.25
                } else {
                    b - (b - a) * 0.25 + Vec2::new(0.0, -11.0 * zoom)
                };
                painter.line_segment([from, to], Stroke::new(1.8, PIN));
                painter.circle_filled(from, 2.6 * zoom, PIN);
                painter.circle_filled(b - (b - a) * 0.25, 2.6 * zoom, PIN);
            }
            PartKind::Gate | PartKind::McuPin => {
                painter.rect_filled(screen, 3.0, fill);
                painter.rect_stroke(screen, 3.0, stroke, StrokeKind::Middle);
            }
        }

        let anchor = self
            .circuit
            .view
            .w2s(rect, Pos2::new(part.x, part.y - PART_H * 0.5 - 4.0));
        painter.text(
            anchor,
            egui::Align2::CENTER_BOTTOM,
            &part.id,
            FontId::monospace(11.0),
            MUTED,
        );
        if !matches!(part.kind, PartKind::Ground) {
            let caption = part.caption();
            // Parts whose symbol fills the body get the caption underneath.
            let below = matches!(
                part.kind,
                PartKind::Diode | PartKind::Led | PartKind::Vsource | PartKind::Isource
            );
            let anchor = self.circuit.view.w2s(
                rect,
                Pos2::new(
                    part.x,
                    part.y + if below { PART_H * 0.5 + 12.0 } else { 0.0 },
                ),
            );
            let size = Vec2::new(caption.chars().count() as f32 * 6.6 + 8.0, 15.0);
            let plate = Rect::from_center_size(anchor, size);
            painter.rect_filled(plate, 3.0, Color32::from_rgb(20, 22, 30));
            painter.text(
                anchor,
                egui::Align2::CENTER_CENTER,
                caption,
                FontId::monospace(11.0),
                TEXT,
            );
        }
    }

    fn circuit_input(&mut self, ui: &mut egui::Ui, rect: Rect) {
        let (
            pos,
            delta,
            primary_pressed,
            primary_down,
            primary_released,
            middle_down,
            secondary_down,
        ) = ui.input(|i| {
            (
                i.pointer.hover_pos(),
                i.pointer.delta(),
                i.pointer.primary_pressed(),
                i.pointer.primary_down(),
                i.pointer.primary_released(),
                i.pointer.button_down(egui::PointerButton::Middle),
                i.pointer.button_down(egui::PointerButton::Secondary),
            )
        });
        let scroll = ui.input(|i| i.smooth_scroll_delta);
        let zoom_delta = ui.input(|i| i.zoom_delta());
        let command = ui.input(|i| i.modifiers.command);

        if let Some(p) = pos {
            if rect.contains(p) {
                let mut factor = zoom_delta;
                if command && scroll.y != 0.0 {
                    factor *= (scroll.y * 0.002).exp();
                } else if scroll.length_sq() > 0.0 {
                    self.circuit.view.pan -= scroll / self.circuit.view.zoom;
                }
                if (factor - 1.0).abs() > 0.001 {
                    self.circuit.view.zoom_at(rect, p, factor);
                }
            }
        }
        if (middle_down || secondary_down) && delta != Vec2::ZERO {
            self.circuit.view.pan -= delta / self.circuit.view.zoom;
        }

        if primary_pressed {
            if let Some(p) = pos {
                if rect.contains(p) {
                    let world = self.circuit.view.s2w(rect, p);
                    if let Some(pin) = self.circuit.model.hit_pin(world) {
                        match self.circuit.pending.clone() {
                            Some(pending) if pending != pin => {
                                self.circuit.model.wire(
                                    pending.part,
                                    &pending.pin,
                                    pin.part,
                                    &pin.pin,
                                );
                                self.circuit.dirty = true;
                                self.circuit.pending = None;
                            }
                            Some(_) => {
                                self.circuit.pending = None;
                            }
                            None => {
                                self.circuit.pending = Some(pin);
                            }
                        }
                        self.circuit.selected = None;
                    } else if let Some(idx) = self.circuit.model.hit_part(world) {
                        self.circuit.selected = Some(Sel::Part(idx));
                        let part = self.circuit.model.parts[idx].clone();
                        let grab = world.to_vec2() - Vec2::new(part.x, part.y);
                        self.circuit.drag = Some(DragPart {
                            index: idx,
                            grab,
                            moved: false,
                        });
                    } else if let Some(w) = self.circuit.model.hit_wire(world) {
                        self.circuit.selected = Some(Sel::Wire(w));
                        self.circuit.drag = None;
                    } else {
                        self.circuit.selected = None;
                        self.circuit.pending = None;
                        self.circuit.panning = true;
                    }
                }
            }
        }

        if primary_down {
            if let Some(mut drag) = self.circuit.drag {
                if let Some(p) = pos {
                    let world = self.circuit.view.s2w(rect, p);
                    let target = world.to_vec2() - drag.grab;
                    let nx = (target.x / 10.0).round() * 10.0;
                    let ny = (target.y / 10.0).round() * 10.0;
                    if let Some(part) = self.circuit.model.parts.get_mut(drag.index) {
                        if (part.x - nx).abs() > 0.01 || (part.y - ny).abs() > 0.01 {
                            part.x = nx;
                            part.y = ny;
                            self.circuit.dirty = true;
                            drag.moved = true;
                        }
                    }
                    self.circuit.drag = Some(drag);
                }
            } else if self.circuit.panning && delta != Vec2::ZERO {
                self.circuit.view.pan -= delta / self.circuit.view.zoom;
            }
        }

        if primary_released {
            self.circuit.drag = None;
            self.circuit.panning = false;
        }
    }

    /// Load a blinking firmware example that matches the current target, so
    /// co-simulation has something to run.
    pub fn load_blink_example(&mut self) {
        let target = Target::parse(&self.graph.target).unwrap_or(Target::Stm32F411);
        let idx = match target {
            Target::AvrAtmega328p => 5,
            Target::Esp32 => 9,
            Target::Esp8266 => 10,
            _ => 7,
        };
        self.load_example(idx);
        self.status = self.tr("status.mcu_example").to_string();
    }

    /// Delete the current selection in the schematic.
    pub fn circuit_delete_selected(&mut self) {
        match self.circuit.selected {
            Some(Sel::Part(idx)) => {
                self.circuit.model.remove_part(idx);
                self.circuit.selected = None;
                self.circuit.dirty = true;
            }
            Some(Sel::Wire(idx)) if idx < self.circuit.model.wires.len() => {
                self.circuit.model.wires.remove(idx);
                self.circuit.selected = None;
                self.circuit.dirty = true;
            }
            _ => {}
        }
    }

    /// Run the simulation and report the outcome.
    pub fn run_circuit(&mut self) {
        let graph = self.graph.clone();
        match run_simulation(&self.circuit.model, &graph) {
            Ok((result, uart, elapsed)) => {
                self.circuit.result = Some(result);
                self.circuit.uart = uart;
                self.circuit.status = format!("{elapsed:.1} ms");
                self.circuit.error.clear();
                self.status = self.tr("status.circuit_ok").to_string();
            }
            Err(e) => {
                self.circuit.error = e.clone();
                self.status = e;
            }
        }
    }
}

/// Run an analysis described by a schematic.
pub fn run_simulation(
    model: &Schematic,
    graph: &Graph,
) -> Result<(SimResult, String, f64), String> {
    let started = std::time::Instant::now();
    let circuit = model.netlist()?;
    let mut sim = Simulator::new(&circuit, model.opts.clone())?;
    if model.mcu_on {
        let bindings: Vec<McuBinding> = model.mcu_bindings()?;
        if bindings.is_empty() {
            return Err("no MCU pins on the schematic".to_string());
        }
        let mut mcu = VirtualMcu::new(graph.clone())?;
        mcu.run_until(0.0)?;
        sim.attach_mcu(mcu, bindings, model.mcu_vdd, 30.0);
    }
    let result = match model.analysis {
        Analysis::Dc => SimResult::Dc(sim.dc()?),
        Analysis::Tran => SimResult::Tran(sim.transient()?),
        Analysis::Ac => SimResult::Ac(sim.ac(model.f_start, model.f_stop, model.ac_points)?),
    };
    let uart = sim.mcu().map(|m| m.uart().to_string()).unwrap_or_default();
    Ok((result, uart, started.elapsed().as_secs_f64() * 1000.0))
}

fn draw_dc_table(ui: &mut egui::Ui, rect: Rect, sol: &Solution) {
    let painter = ui.painter();
    let mut y = rect.top() + 6.0;
    let row_h = 16.0;
    let max_abs = sol
        .nodes
        .values()
        .fold(0.0f64, |acc, v| acc.max(v.abs()))
        .max(1e-9);
    for (name, value) in &sol.nodes {
        if y > rect.bottom() {
            break;
        }
        painter.text(
            egui::pos2(rect.left() + 4.0, y),
            egui::Align2::LEFT_TOP,
            name,
            FontId::monospace(12.0),
            TEXT,
        );
        painter.text(
            egui::pos2(rect.left() + 140.0, y),
            egui::Align2::LEFT_TOP,
            format!("{value:>12.5} V"),
            FontId::monospace(12.0),
            MUTED,
        );
        let bar_w = (rect.width() - 260.0).max(40.0);
        let frac = (value.abs() / max_abs).clamp(0.0, 1.0) as f32;
        let bar = Rect::from_min_size(
            egui::pos2(rect.left() + 250.0, y + 2.0),
            Vec2::new(bar_w * frac, row_h - 6.0),
        );
        painter.rect_filled(bar, 2.0, Color32::from_rgb(96, 165, 250));
        y += row_h;
    }
}

/// Draw a set of series against a shared x axis.
fn draw_plot(
    ui: &mut egui::Ui,
    rect: Rect,
    x: &[f64],
    series: &[(String, Vec<f64>)],
    log_x: bool,
    x_label: &str,
    y_label: &str,
) {
    let painter = ui.painter();
    let left = rect.left() + 52.0;
    let bottom = rect.bottom() - 26.0;
    let right = rect.right() - 12.0;
    let top = rect.top() + 10.0;
    if right <= left || bottom <= top || x.is_empty() {
        return;
    }

    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;
    for (_, values) in series {
        for v in values {
            if v.is_finite() {
                y_min = y_min.min(*v);
                y_max = y_max.max(*v);
            }
        }
    }
    if !y_min.is_finite() || !y_max.is_finite() {
        return;
    }
    if (y_max - y_min).abs() < 1e-12 {
        y_max += 1.0;
        y_min -= 1.0;
    }
    let span = y_max - y_min;
    y_min -= span * 0.08;
    y_max += span * 0.08;

    let x0 = x[0];
    let x1 = x[x.len() - 1];
    let (lx0, lx1) = if log_x {
        (x0.max(1e-12).log10(), x1.max(1e-12).log10())
    } else {
        (x0, x1)
    };
    let lx_span = (lx1 - lx0).max(1e-12);

    let to_screen = |xv: f64, yv: f64| -> Pos2 {
        let t = if log_x {
            (xv.max(1e-12).log10() - lx0) / lx_span
        } else {
            (xv - x0) / (x1 - x0).max(1e-12)
        };
        let ty = (yv - y_min) / (y_max - y_min);
        Pos2::new(
            left + t as f32 * (right - left),
            bottom - ty as f32 * (bottom - top),
        )
    };

    for i in 0..=4 {
        let t = i as f32 / 4.0;
        let gy = bottom - t * (bottom - top);
        painter.line_segment(
            [egui::pos2(left, gy), egui::pos2(right, gy)],
            Stroke::new(0.6, GRID_STRONG),
        );
        let value = y_min + (y_max - y_min) * (i as f64 / 4.0);
        painter.text(
            egui::pos2(left - 6.0, gy),
            egui::Align2::RIGHT_CENTER,
            format!("{value:.2}"),
            FontId::monospace(10.0),
            MUTED,
        );
    }
    for i in 0..=4 {
        let t = i as f32 / 4.0;
        let gx = left + t * (right - left);
        painter.line_segment(
            [egui::pos2(gx, top), egui::pos2(gx, bottom)],
            Stroke::new(0.6, GRID_STRONG),
        );
        let lxv = lx0 + lx_span * (i as f64 / 4.0);
        let xv = if log_x { 10f64.powf(lxv) } else { lxv };
        painter.text(
            egui::pos2(gx, bottom + 4.0),
            egui::Align2::CENTER_TOP,
            if log_x {
                format!("{xv:.0}")
            } else {
                format!("{xv:.4}")
            },
            FontId::monospace(10.0),
            MUTED,
        );
    }
    painter.line_segment(
        [egui::pos2(left, top), egui::pos2(left, bottom)],
        Stroke::new(1.0, PART_BORDER),
    );
    painter.line_segment(
        [egui::pos2(left, bottom), egui::pos2(right, bottom)],
        Stroke::new(1.0, PART_BORDER),
    );
    painter.text(
        egui::pos2(right, bottom + 4.0),
        egui::Align2::RIGHT_TOP,
        x_label,
        FontId::monospace(10.0),
        MUTED,
    );
    painter.text(
        egui::pos2(left - 4.0, top - 2.0),
        egui::Align2::RIGHT_BOTTOM,
        y_label,
        FontId::monospace(10.0),
        MUTED,
    );

    let step = (x.len() / 1200).max(1);
    for (i, (_, values)) in series.iter().enumerate() {
        let color = SERIES_COLORS[i % SERIES_COLORS.len()];
        let mut points: Vec<Pos2> = Vec::new();
        for (k, v) in values.iter().enumerate() {
            if k % step != 0 && k + 1 != values.len() {
                continue;
            }
            let Some(xv) = x.get(k) else { continue };
            if !v.is_finite() {
                continue;
            }
            points.push(to_screen(*xv, *v));
        }
        if points.len() < 2 {
            continue;
        }
        painter.add(egui::Shape::line(points, Stroke::new(1.6, color)));
    }
}

/// Geometry helpers that the editor needs from the model.
pub trait CircuitGeometry {
    fn pin_pos(&self, part: usize, pin: &str) -> Pos2;
    fn body_rect(&self, part: usize) -> Rect;
    fn hit_pin(&self, world: Pos2) -> Option<PinAddr>;
    fn hit_part(&self, world: Pos2) -> Option<usize>;
    fn hit_wire(&self, world: Pos2) -> Option<usize>;
}

impl CircuitGeometry for Schematic {
    fn pin_pos(&self, part: usize, pin: &str) -> Pos2 {
        let base = self
            .parts
            .get(part)
            .map(|p| Pos2::new(p.x, p.y))
            .unwrap_or(Pos2::ZERO);
        let (dx, dy) = self.pin_offset(part, pin);
        base + Vec2::new(dx, dy)
    }

    fn body_rect(&self, part: usize) -> Rect {
        let Some(p) = self.parts.get(part) else {
            return Rect::from_min_size(Pos2::ZERO, Vec2::ZERO);
        };
        let half = match p.kind {
            PartKind::Ground => Vec2::new(18.0, 14.0),
            PartKind::Gate => Vec2::new(PART_W * 0.5, 20.0),
            _ => Vec2::new(PART_W * 0.5, PART_H * 0.5),
        };
        Rect::from_center_size(Pos2::new(p.x, p.y), half * 2.0)
    }

    fn hit_pin(&self, world: Pos2) -> Option<PinAddr> {
        let mut best: Option<(f32, PinAddr)> = None;
        for (i, part) in self.parts.iter().enumerate() {
            for pin in part.pins() {
                let d = (self.pin_pos(i, pin) - world).length();
                if d < 12.0 && best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                    best = Some((d, PinAddr::new(i, pin)));
                }
            }
        }
        best.map(|(_, a)| a)
    }

    fn hit_part(&self, world: Pos2) -> Option<usize> {
        (0..self.parts.len()).find(|i| self.body_rect(*i).contains(world))
    }

    fn hit_wire(&self, world: Pos2) -> Option<usize> {
        let mut best: Option<(f32, usize)> = None;
        for (i, w) in self.wires.iter().enumerate() {
            let a = self.pin_pos(w.a.part, &w.a.pin);
            let b = self.pin_pos(w.b.part, &w.b.pin);
            let d = distance_to_segment(world, a, b);
            if d < 8.0 && best.as_ref().map(|(bd, _)| d < *bd).unwrap_or(true) {
                best = Some((d, i));
            }
        }
        best.map(|(_, i)| i)
    }
}

/// Distance from a point to a line segment.
pub fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    wireasm_sim::schematic::distance_to_segment(p.x, p.y, a.x, a.y, b.x, b.y)
}

/// Route a wire as three straight segments, the way schematics are drawn:
/// out of one pin, across, and into the other.
fn elbow(a: Pos2, b: Pos2) -> Vec<Pos2> {
    if (a.x - b.x).abs() < 0.5 || (a.y - b.y).abs() < 0.5 {
        return vec![a, b];
    }
    let mid = (a.x + b.x) * 0.5;
    vec![a, Pos2::new(mid, a.y), Pos2::new(mid, b.y), b]
}

#[cfg(test)]
mod tests {
    use super::*;
    use wireasm_sim::Simulator;

    #[test]
    fn example_schemes_run() {
        for (name, model) in Schematic::examples() {
            let circuit = model.netlist().unwrap_or_else(|e| panic!("{name}: {e}"));
            assert!(!circuit.components.is_empty(), "{name} has no components");
        }
    }

    #[test]
    fn geometry_finds_parts() {
        let model = Schematic::divider();
        let pin = model.pin_pos(1, "1");
        assert!(model.hit_pin(pin).is_some());
        let body = model.body_rect(1);
        assert!(model.hit_part(body.center()).is_some());
    }

    #[test]
    fn divider_matches_the_analytic_value() {
        let model = Schematic::divider();
        let circuit = model.netlist().unwrap();
        let mut sim = Simulator::new(&circuit, model.opts.clone()).unwrap();
        let sol = sim.dc().unwrap();
        let expected = 12.0 * 4700.0 / 14700.0;
        assert!((sol.node("out") - expected).abs() < 1e-6);
    }

    #[test]
    fn mcu_firmware_drives_the_schematic() {
        // The blink graph must toggle the C13 pin of the MCU LED schematic.
        let graph = wireasm_core::model::Graph::from_json_value(
            &wireasm_core::json::Json::parse(include_str!("../../../examples/stm32_blink.json"))
                .unwrap(),
        )
        .unwrap();
        let mut model = Schematic::mcu_led();
        model.opts.dt = 1e-1;
        model.opts.t_stop = 2.0;
        let (result, _uart, _ms) = run_simulation(&model, &graph).unwrap();
        let Some(SimResult::Tran(trace)) = Some(result) else {
            panic!("expected a transient result");
        };
        let pin = trace.series.get("mcu").expect("probe mcu");
        assert!(pin.iter().any(|v| *v > 2.0), "pin never went high");
        assert!(pin.iter().any(|v| *v < 0.5), "pin never went low");
    }

    #[test]
    fn saved_schematic_reopens_with_the_same_nets() {
        let model = Schematic::rc_lowpass();
        let text = model.to_json_value().stringify_pretty();
        let back =
            Schematic::from_json_value(&wireasm_core::json::Json::parse(&text).unwrap()).unwrap();
        assert_eq!(back.nets(), model.nets());
    }
}
