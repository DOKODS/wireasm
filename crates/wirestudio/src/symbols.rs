//! Circuit symbols drawn to GOST.
//!
//! The shapes follow the Russian standards for graphical symbols:
//!
//! * ГОСТ 2.722-68 — resistors, drawn as a rectangle;
//! * ГОСТ 2.723-68 — inductors, three arcs;
//! * ГОСТ 2.725-68 — capacitors, two plates;
//! * ГОСТ 2.727-68 — diodes and light-emitting diodes;
//! * ГОСТ 2.730-68, 2.735-68, 2.736-68 — sources of current and voltage;
//! * ГОСТ 2.741-68 — bipolar transistors;
//! * ГОСТ 2.742-68 — the earth symbol;
//! * ГОСТ 2.759-68 — switches;
//! * ГОСТ 2.610-68 and ГОСТ 2.318-81 — logic elements and integrated
//!   circuits, drawn as a box with the function inside.
//!
//! Every symbol is drawn in world units and scaled by the canvas zoom, so a
//! zoomed-in schematic keeps the same proportions.

use eframe::egui::{self, Color32, FontId, Pos2, Stroke, Vec2};

use wireasm_sim::schematic::{Part, PartKind};

use crate::settings::SheetStyle;
use crate::sheet;

/// Line colour of a symbol.
pub const LINE: Color32 = sheet::PART_LINE;
/// Colour of the pins and their stubs.
pub const PIN: Color32 = sheet::PIN;
/// Colour of the letters inside a symbol.
pub const TEXT: Color32 = sheet::PART_LABEL;
/// Colour of the muted captions.
pub const MUTED: Color32 = sheet::NODE_NUMBER;
/// Fill of a symbol body.
pub const FILL: Color32 = sheet::PART_FILL;

/// Everything the drawing helpers need.
pub struct Ctx<'a> {
    /// The part to draw.
    pub part: &'a Part,
    /// The canvas painter.
    pub painter: &'a egui::Painter,
    /// Where the part sits, in world units.
    pub center: Pos2,
    /// The pin positions, in world units.
    pub pins: &'a [(String, Pos2)],
    /// How to turn world coordinates into screen coordinates.
    pub w2s: &'a dyn Fn(Pos2) -> Pos2,
    /// The zoom factor.
    pub zoom: f32,
    /// Whether the part is selected.
    pub selected: bool,
    /// The look of the sheet.
    pub style: SheetStyle,
}

impl Ctx<'_> {
    fn screen(&self, p: Vec2) -> Pos2 {
        (self.w2s)(p.to_pos2())
    }

    fn line(&self, a: Vec2, b: Vec2, width: f32, color: Color32) {
        self.painter
            .line_segment([self.screen(a), self.screen(b)], Stroke::new(width, color));
    }

    fn stroke(&self) -> Stroke {
        if self.selected {
            Stroke::new(2.0 * self.zoom.max(0.5), sheet::SELECT)
        } else {
            Stroke::new(1.4 * self.zoom.max(0.5), self.style.part_line())
        }
    }

    /// The fill of a symbol body.
    fn fill(&self) -> Color32 {
        self.style.part_fill()
    }

    /// The colour of the pins.
    fn pin(&self) -> Color32 {
        self.style.pin()
    }

    /// The colour of the letters inside a symbol.
    fn label_color(&self) -> Color32 {
        self.style.part_label()
    }

    fn text(&self, at: Vec2, text: &str, size: f32) {
        self.text_at(at, text, size, egui::Align2::CENTER_CENTER);
    }

    /// Write a text with a chosen alignment.
    fn text_at(&self, at: Vec2, text: &str, size: f32, align: egui::Align2) {
        self.painter.text(
            self.screen(at),
            align,
            text,
            FontId::monospace(size),
            self.label_color(),
        );
    }

    /// The position of a pin, in world units.
    pub fn pin_pos(&self, name: &str) -> Option<Vec2> {
        self.pins
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, p)| p.to_vec2())
    }

    /// Fill a polygon given in world coordinates.
    fn polygon(&self, points: &[Vec2]) {
        let screen: Vec<Pos2> = points.iter().map(|p| self.screen(*p)).collect();
        self.painter
            .add(egui::Shape::convex_polygon(screen, FILL, self.stroke()));
    }
}

/// Draw the symbol of the part.
pub fn draw(ctx: &Ctx) {
    let part = ctx.part;
    if let Some(device) = part.catalog_device {
        device_symbol(ctx, device);
        return;
    }
    if part.is_catalog() {
        integrated_circuit(ctx);
        return;
    }
    match part.kind {
        PartKind::Resistor => resistor(ctx),
        PartKind::Capacitor => capacitor(ctx),
        PartKind::Inductor => inductor(ctx),
        PartKind::Diode => diode(ctx, false),
        PartKind::Led => diode(ctx, true),
        PartKind::Vsource => source_voltage(ctx),
        PartKind::Isource => source_current(ctx),
        PartKind::Switch => switch(ctx),
        PartKind::Ground => ground(ctx),
        PartKind::McuPin => mcu_pin(ctx),
        PartKind::Gate => logic_gate(ctx),
    }
}

/// The symbol of a device part: a diode, a transistor, an amplifier.
///
/// The shapes follow the Russian standards:
///
/// * ГОСТ 2.727-68 — diodes and light-emitting diodes;
/// * ГОСТ 2.741-68 — bipolar transistors;
/// * ГОСТ 2.742-68 — the field-effect transistor.
fn device_symbol(ctx: &Ctx, device: wireasm_sim::library::Device) {
    use wireasm_sim::library::Device;
    match device {
        Device::Diode => diode(ctx, false),
        Device::Led => diode(ctx, true),
        Device::Zener => zener(ctx),
        Device::Bjt { npn } => bipolar(ctx, npn),
        Device::Mosfet { nch } => field_effect(ctx, nch),
        Device::OpAmp => amplifier(ctx, false),
        Device::Comparator => amplifier(ctx, true),
        Device::Timer555 => timer555(ctx),
    }
}

/// A diode with a bent cathode: the zener.
fn zener(ctx: &Ctx) {
    let (a, b) = ends(ctx);
    let mid = a * 0.5 + b * 0.5;
    let dir = (b - a).normalized();
    let normal = Vec2::new(-dir.y, dir.x);
    let w = 11.0;
    ctx.polygon(&[
        mid - dir * w - normal * 8.0,
        mid - dir * w + normal * 8.0,
        mid + dir * w * 0.6,
    ]);
    let bar = mid + dir * w * 0.6;
    ctx.line(bar - normal * 8.0, bar - normal * 3.0, 2.0, ctx.style.part_line());
    ctx.line(bar - normal * 3.0, bar + normal * 8.0, 2.0, ctx.style.part_line());
    ctx.line(a, mid - dir * w, 1.6, ctx.pin());
    ctx.line(bar, b, 1.6, ctx.pin());
}

/// A circle with the base bar and the arrow: a bipolar transistor.
///
/// The shape follows ГОСТ 2.741-68: the base bar stands on the left, the
/// collector and the emitter leave it at an angle, and the arrow of the
/// emitter points away from the base for an NPN transistor.
fn bipolar(ctx: &Ctx, npn: bool) {
    let base = ctx.pin_pos("B").unwrap_or(ctx.center.to_vec2());
    let collector = ctx.pin_pos("C").unwrap_or(ctx.center.to_vec2());
    let emitter = ctx.pin_pos("E").unwrap_or(ctx.center.to_vec2());
    let mid = ctx.center.to_vec2();
    let r = 22.0;
    ctx.painter
        .circle_stroke(ctx.screen(mid), r * ctx.zoom, ctx.stroke());

    // The base bar and the lead that comes to it.
    let bar_x = mid.x - 12.0;
    ctx.line(
        Vec2::new(bar_x, mid.y - 11.0),
        Vec2::new(bar_x, mid.y + 11.0),
        2.0,
        ctx.style.part_line(),
    );
    ctx.line(base, Vec2::new(bar_x, mid.y), 1.6, ctx.pin());

    // The collector and the emitter leave the bar at an angle.
    let knee_c = Vec2::new(mid.x + 8.0, mid.y - 15.0);
    let knee_e = Vec2::new(mid.x + 8.0, mid.y + 15.0);
    ctx.line(
        Vec2::new(bar_x, mid.y - 5.0),
        knee_c,
        1.6,
        ctx.style.part_line(),
    );
    ctx.line(knee_c, collector, 1.6, ctx.pin());
    ctx.line(
        Vec2::new(bar_x, mid.y + 5.0),
        knee_e,
        1.6,
        ctx.style.part_line(),
    );
    ctx.line(knee_e, emitter, 1.6, ctx.pin());

    // The arrow of the emitter: out of the base for an NPN, into it for a
    // PNP.
    let dir = if npn {
        (knee_e - Vec2::new(bar_x, mid.y + 5.0)).normalized()
    } else {
        (Vec2::new(bar_x, mid.y + 5.0) - knee_e).normalized()
    };
    let tip = knee_e - dir * 4.0;
    let wing = Vec2::new(-dir.y, dir.x) * 4.5;
    ctx.line(tip, tip - dir * 9.0 + wing, 1.6, ctx.style.part_line());
    ctx.line(tip, tip - dir * 9.0 - wing, 1.6, ctx.style.part_line());
}

/// The field-effect transistor: a gate bar, a channel bar and the arrow.
///
/// The shape follows ГОСТ 2.742-68.
fn field_effect(ctx: &Ctx, nch: bool) {
    let gate = ctx.pin_pos("G").unwrap_or(ctx.center.to_vec2());
    let drain = ctx.pin_pos("D").unwrap_or(ctx.center.to_vec2());
    let source = ctx.pin_pos("S").unwrap_or(ctx.center.to_vec2());
    let mid = ctx.center.to_vec2();

    // The gate bar and the lead that comes to it.
    let gate_bar = Vec2::new(mid.x - 14.0, mid.y);
    ctx.line(
        gate_bar + Vec2::new(0.0, -12.0),
        gate_bar + Vec2::new(0.0, 12.0),
        2.0,
        ctx.style.part_line(),
    );
    ctx.line(gate, gate_bar, 1.6, ctx.pin());

    // The channel bar, drawn in three pieces the way the standard shows it.
    let channel = Vec2::new(mid.x + 2.0, mid.y);
    ctx.line(
        channel + Vec2::new(0.0, -15.0),
        channel + Vec2::new(0.0, 15.0),
        2.0,
        ctx.style.part_line(),
    );

    // The drain and the source leave the channel at an angle.
    let knee_d = Vec2::new(mid.x + 12.0, mid.y - 15.0);
    let knee_s = Vec2::new(mid.x + 12.0, mid.y + 15.0);
    ctx.line(
        channel + Vec2::new(0.0, -7.0),
        knee_d,
        1.6,
        ctx.style.part_line(),
    );
    ctx.line(knee_d, drain, 1.6, ctx.pin());
    ctx.line(
        channel + Vec2::new(0.0, 7.0),
        knee_s,
        1.6,
        ctx.style.part_line(),
    );
    ctx.line(knee_s, source, 1.6, ctx.pin());

    // The arrow of the source points into the channel for an N-channel.
    let dir = if nch {
        (channel - knee_s).normalized()
    } else {
        (knee_s - channel).normalized()
    };
    let tip = knee_s + dir * 4.0;
    let wing = Vec2::new(-dir.y, dir.x) * 4.5;
    ctx.line(tip, tip - dir * 9.0 + wing, 1.6, ctx.style.part_line());
    ctx.line(tip, tip - dir * 9.0 - wing, 1.6, ctx.style.part_line());
}

/// A triangle: an operational amplifier, or a comparator.
fn amplifier(ctx: &Ctx, comparator: bool) {
    let in_plus = ctx.pin_pos("IN+").unwrap_or(ctx.center.to_vec2());
    let in_minus = ctx.pin_pos("IN-").unwrap_or(ctx.center.to_vec2());
    let out = ctx.pin_pos("OUT").unwrap_or(ctx.center.to_vec2());
    let mid = ctx.center.to_vec2();
    let half_w = 30.0;
    let half_h = 18.0;
    let apex = Vec2::new(mid.x + half_w, mid.y);
    ctx.polygon(&[
        Vec2::new(mid.x - half_w, mid.y - half_h),
        Vec2::new(mid.x - half_w, mid.y + half_h),
        apex,
    ]);
    ctx.line(in_plus, Vec2::new(mid.x - half_w, mid.y - half_h * 0.5), 1.6, ctx.pin());
    ctx.line(in_minus, Vec2::new(mid.x - half_w, mid.y + half_h * 0.5), 1.6, ctx.pin());
    ctx.line(apex, out, 1.6, ctx.pin());
    // The signs of the inputs.
    ctx.text(Vec2::new(mid.x - half_w + 10.0, mid.y - half_h * 0.5), "+", 11.0);
    ctx.text(Vec2::new(mid.x - half_w + 10.0, mid.y + half_h * 0.5), "-", 11.0);
    if comparator {
        // The comparator carries the sign of the output.
        ctx.text(Vec2::new(mid.x + 2.0, mid.y), ">", 13.0);
    }
}

/// The 555 timer: a box with the names of its pins.
///
/// The box is a little wider than the pins, so the names sit inside it and
/// do not touch the border.
fn timer555(ctx: &Ctx) {
    let mut points: Vec<Vec2> = ctx.pins.iter().map(|(_, p)| p.to_vec2()).collect();
    points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    let left = points.first().copied().unwrap_or(ctx.center.to_vec2()).x;
    let right = points.last().copied().unwrap_or(ctx.center.to_vec2()).x;
    let top = points.iter().map(|p| p.y).fold(f32::MAX, f32::min) - 4.0;
    let bottom = points.iter().map(|p| p.y).fold(f32::MIN, f32::max) + 4.0;
    ctx.polygon(&[
        Vec2::new(left, top),
        Vec2::new(right, top),
        Vec2::new(right, bottom),
        Vec2::new(left, bottom),
    ]);
    ctx.text(ctx.center.to_vec2() + Vec2::new(0.0, 4.0), "555", 13.0);
    for (name, at) in ctx.pins {
        // The names of the pins sit inside the box, next to their pin.
        if at.y < ctx.center.y - 10.0 {
            // A pin on the top edge: the name goes under it.
            ctx.text_at(
                Vec2::new(at.x, at.y + 3.0),
                name,
                8.0,
                egui::Align2::CENTER_TOP,
            );
        } else if at.y > ctx.center.y + 10.0 {
            ctx.text_at(
                Vec2::new(at.x, at.y - 3.0),
                name,
                8.0,
                egui::Align2::CENTER_BOTTOM,
            );
        } else if at.x < ctx.center.x {
            ctx.text_at(
                Vec2::new(at.x + 4.0, at.y),
                name,
                8.0,
                egui::Align2::LEFT_CENTER,
            );
        } else {
            ctx.text_at(
                Vec2::new(at.x - 4.0, at.y),
                name,
                8.0,
                egui::Align2::RIGHT_CENTER,
            );
        }
    }
}

/// A rectangle, the GOST form of a resistor.
fn resistor(ctx: &Ctx) {
    let (a, b) = ends(ctx);
    let dir = (b - a).normalized();
    let len = 22.0;
    let p1 = a + dir * len;
    let p2 = b - dir * len;
    let normal = Vec2::new(-dir.y, dir.x) * 9.0;
    ctx.polygon(&[p1 + normal, p2 + normal, p2 - normal, p1 - normal]);
    ctx.line(a, p1, 1.6, ctx.pin());
    ctx.line(p2, b, 1.6, ctx.pin());
}

/// Two plates, the GOST form of a capacitor.
fn capacitor(ctx: &Ctx) {
    let (a, b) = ends(ctx);
    let mid = a * 0.5 + b * 0.5;
    let dir = (b - a).normalized();
    let normal = Vec2::new(-dir.y, dir.x);
    let gap = 11.0;
    ctx.line(a, mid - dir * gap, 1.6, ctx.pin());
    ctx.line(b, mid + dir * gap, 1.6, ctx.pin());
    ctx.line(
        mid - dir * gap - normal * 10.0,
        mid - dir * gap + normal * 10.0,
        2.0,
        ctx.style.part_line(),
    );
    ctx.line(
        mid + dir * gap - normal * 10.0,
        mid + dir * gap + normal * 10.0,
        2.0,
        ctx.style.part_line(),
    );
}

/// Three arcs, the GOST form of an inductor.
fn inductor(ctx: &Ctx) {
    let (a, b) = ends(ctx);
    let dir = (b - a).normalized();
    let len = (b - a).length();
    let step = len / 3.0;
    ctx.line(a, a + dir * step * 0.5, 1.6, ctx.pin());
    for i in 0..3 {
        let c = a + dir * (step * (i as f32 + 0.5));
        let r = step * 0.5;
        let mut points = Vec::new();
        for k in 0..=16 {
            let t = std::f32::consts::PI * (k as f32 / 16.0);
            // The arc bulges away from the wire.
            let offset = Vec2::new(-dir.y, dir.x) * (r * t.sin());
            points.push(ctx.screen(c + dir * (r * t.cos()) + offset));
        }
        ctx.painter
            .add(egui::Shape::line(points, Stroke::new(1.6, LINE)));
    }
    ctx.line(b - dir * step * 0.5, b, 1.6, ctx.pin());
}

/// A triangle and a bar: a diode, with arrows for a light-emitting one.
fn diode(ctx: &Ctx, lit: bool) {
    let (a, b) = ends(ctx);
    let mid = a * 0.5 + b * 0.5;
    let dir = (b - a).normalized();
    let normal = Vec2::new(-dir.y, dir.x);
    let w = 11.0;
    ctx.polygon(&[
        mid - dir * w - normal * 8.0,
        mid - dir * w + normal * 8.0,
        mid + dir * w * 0.6,
    ]);
    let bar = mid + dir * w * 0.6;
    ctx.line(bar - normal * 8.0, bar + normal * 8.0, 2.0, ctx.style.part_line());
    ctx.line(a, mid - dir * w, 1.6, ctx.pin());
    ctx.line(bar, b, 1.6, ctx.pin());
    if lit {
        // Two arrows pointing away from the junction.
        let tip = mid + dir * w * 0.6 + normal * 4.0;
        let color = Color32::from_rgb(250, 210, 120);
        for k in 0..2 {
            let start = tip + normal * (4.0 + k as f32 * 6.0) - dir * 6.0;
            let end = tip + normal * (1.0 + k as f32 * 6.0);
            ctx.line(start, end, 1.3, color);
            // The arrowhead.
            let head = Vec2::new(-dir.x, -dir.y);
            ctx.line(end, end - head * 4.0 + normal * 2.5, 1.3, color);
            ctx.line(end, end - head * 4.0 - normal * 2.5, 1.3, color);
        }
    }
}

/// A circle with the signs: a voltage source.
fn source_voltage(ctx: &Ctx) {
    let (a, b) = ends(ctx);
    let mid = a * 0.5 + b * 0.5;
    let r = 13.0;
    ctx.painter
        .circle_filled(ctx.screen(mid), r * ctx.zoom, ctx.fill());
    ctx.painter
        .circle_stroke(ctx.screen(mid), r * ctx.zoom, ctx.stroke());
    ctx.line(a, mid - Vec2::new(0.0, r), 1.6, ctx.pin());
    ctx.line(b, mid + Vec2::new(0.0, r), 1.6, ctx.pin());
    // The plus on top and the minus below.
    ctx.line(
        mid + Vec2::new(-4.0, -8.0),
        mid + Vec2::new(4.0, -8.0),
        1.5,
        TEXT,
    );
    ctx.line(
        mid + Vec2::new(0.0, -12.0),
        mid + Vec2::new(0.0, -4.0),
        1.5,
        TEXT,
    );
    ctx.line(
        mid + Vec2::new(-4.0, 8.0),
        mid + Vec2::new(4.0, 8.0),
        1.5,
        TEXT,
    );
}

/// A circle with an arrow: a current source.
fn source_current(ctx: &Ctx) {
    let (a, b) = ends(ctx);
    let mid = a * 0.5 + b * 0.5;
    let r = 13.0;
    ctx.painter
        .circle_filled(ctx.screen(mid), r * ctx.zoom, ctx.fill());
    ctx.painter
        .circle_stroke(ctx.screen(mid), r * ctx.zoom, ctx.stroke());
    ctx.line(a, mid - Vec2::new(0.0, r), 1.6, ctx.pin());
    ctx.line(b, mid + Vec2::new(0.0, r), 1.6, ctx.pin());
    // The arrow follows the current direction.
    ctx.line(
        mid + Vec2::new(0.0, 8.0),
        mid + Vec2::new(0.0, -8.0),
        1.5,
        TEXT,
    );
    let tip = mid + Vec2::new(0.0, -9.0);
    ctx.painter.add(egui::Shape::convex_polygon(
        vec![
            ctx.screen(tip),
            ctx.screen(tip + Vec2::new(-3.5, 6.0)),
            ctx.screen(tip + Vec2::new(3.5, 6.0)),
        ],
        TEXT,
        Stroke::NONE,
    ));
}

/// A lever, the GOST form of a switch.
fn switch(ctx: &Ctx) {
    let (a, b) = ends(ctx);
    let on = ctx.part.on;
    let from = a + (b - a) * 0.25;
    let to = if on {
        b - (b - a) * 0.25
    } else {
        b - (b - a) * 0.25 + Vec2::new(0.0, -13.0)
    };
    ctx.line(a, from, 1.6, ctx.pin());
    ctx.line(b, b - (b - a) * 0.25, 1.6, ctx.pin());
    ctx.line(from, to, 1.8, ctx.style.part_line());
    ctx.painter
        .circle_filled(ctx.screen(from), 2.6 * ctx.zoom, ctx.pin());
    ctx.painter
        .circle_filled(ctx.screen(b - (b - a) * 0.25), 2.6 * ctx.zoom, ctx.pin());
}

/// The earth symbol: three shrinking lines.
fn ground(ctx: &Ctx) {
    let top = ctx.pin_pos("0").unwrap_or(ctx.center.to_vec2());
    let w = 15.0;
    ctx.line(top, top + Vec2::new(0.0, 7.0), 1.8, ctx.pin());
    for (half, dy) in [(1.0, 7.0), (0.62, 12.0), (0.3, 17.0)] {
        ctx.line(
            top + Vec2::new(-w * half, dy),
            top + Vec2::new(w * half, dy),
            1.8,
            ctx.style.part_line(),
        );
    }
}

/// A box with the pin name: a microcontroller pin.
fn mcu_pin(ctx: &Ctx) {
    let center = ctx.center.to_vec2();
    let half = Vec2::new(26.0, 15.0);
    ctx.polygon(&[
        center + Vec2::new(-half.x, -half.y),
        center + Vec2::new(half.x, -half.y),
        center + Vec2::new(half.x, half.y),
        center + Vec2::new(-half.x, half.y),
    ]);
    let name = ctx.part.pin_name.clone();
    ctx.text(center, &name, 11.0);
}

/// A box with the function inside: a logic element per ГОСТ 2.610-68.
fn logic_gate(ctx: &Ctx) {
    let inputs: Vec<Vec2> = ctx
        .pins
        .iter()
        .filter(|(name, _)| name == "a" || name == "b")
        .map(|(_, p)| p.to_vec2())
        .collect();
    let output = ctx
        .pins
        .iter()
        .find(|(name, _)| name == "y")
        .map(|(_, p)| p.to_vec2());
    let Some(output) = output else { return };
    let left = inputs
        .iter()
        .copied()
        .min_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal))
        .unwrap_or(ctx.center.to_vec2());
    let half = Vec2::new(22.0, 16.0);
    let top = left.y - half.y;
    let bottom = left.y + half.y;
    ctx.polygon(&[
        Vec2::new(left.x, top),
        Vec2::new(output.x, top),
        Vec2::new(output.x, bottom),
        Vec2::new(left.x, bottom),
    ]);
    for p in &inputs {
        ctx.line(*p, Vec2::new(left.x, p.y), 1.6, ctx.pin());
    }
    ctx.line(output, Vec2::new(output.x + 6.0, output.y), 1.6, ctx.pin());
    // The function mark, as the standard writes it.
    let mark = match ctx.part.gate {
        wireasm_sim::GateKind::And => "≥1",
        wireasm_sim::GateKind::Or => "=1",
        wireasm_sim::GateKind::Nand => "≥1",
        wireasm_sim::GateKind::Nor => "=1",
        wireasm_sim::GateKind::Xor => "=1",
        wireasm_sim::GateKind::Xnor => "=1",
        wireasm_sim::GateKind::Not => "1",
        wireasm_sim::GateKind::Buffer => "1",
    };
    ctx.text(Vec2::new((left.x + output.x) * 0.5, left.y), mark, 12.0);
}

/// A catalogue part: a box per ГОСТ 2.318-81, with the pins on the sides and
/// the type inside.
fn integrated_circuit(ctx: &Ctx) {
    let part = ctx.part;
    let entry = part.catalog_entry();
    let title = entry
        .as_ref()
        .map(|e| e.title.clone())
        .unwrap_or_else(|| part.catalog.clone().unwrap_or_default());
    let key = part.catalog.clone().unwrap_or_else(|| part.id.clone());

    // Split the pins into inputs, outputs and the supply.
    let mut inputs: Vec<(String, Vec2)> = Vec::new();
    let mut outputs: Vec<(String, Vec2)> = Vec::new();
    let mut supply: Vec<(String, Vec2)> = Vec::new();
    for unit in &part.catalog_units {
        for name in &unit.ins {
            if let Some(p) = ctx.pin_pos(name) {
                if !inputs.iter().any(|(n, _)| n == name) {
                    inputs.push((name.clone(), p));
                }
            }
        }
        for name in &unit.outs {
            if let Some(p) = ctx.pin_pos(name) {
                if !outputs.iter().any(|(n, _)| n == name) {
                    outputs.push((name.clone(), p));
                }
            }
        }
    }
    if let Some(power) = &part.catalog_power {
        for name in [&power.vcc, &power.gnd] {
            if let Some(p) = ctx.pin_pos(name) {
                supply.push((name.clone(), p));
            }
        }
    }

    let left = inputs.iter().map(|(_, p)| p.x).fold(f32::MAX, f32::min);
    let right = outputs.iter().map(|(_, p)| p.x).fold(f32::MIN, f32::max);
    let (left, right) = if left > right {
        (ctx.center.x - 34.0, ctx.center.x + 34.0)
    } else {
        (left, right)
    };
    let top = inputs
        .iter()
        .chain(outputs.iter())
        .map(|(_, p)| p.y)
        .fold(f32::MAX, f32::min)
        .min(ctx.center.y - 18.0);
    let bottom = inputs
        .iter()
        .chain(outputs.iter())
        .map(|(_, p)| p.y)
        .fold(f32::MIN, f32::max)
        .max(ctx.center.y + 18.0);

    ctx.polygon(&[
        Vec2::new(left, top),
        Vec2::new(right, top),
        Vec2::new(right, bottom),
        Vec2::new(left, bottom),
    ]);

    // The pins are drawn as stubs into the box.
    for (name, p) in inputs.iter().chain(outputs.iter()) {
        let edge_x = if inputs.iter().any(|(n, _)| n == name) {
            left
        } else {
            right
        };
        ctx.line(*p, Vec2::new(edge_x, p.y), 1.4, PIN);
    }
    for (name, p) in &supply {
        ctx.line(*p, *p, 1.4, PIN);
        let _ = name;
    }

    // The type and the title inside the box.
    ctx.text(
        Vec2::new((left + right) * 0.5, (top + bottom) * 0.5 - 7.0),
        &key,
        11.0,
    );
    let caption: String = title.chars().take(18).collect();
    ctx.painter.text(
        ctx.screen(Vec2::new((left + right) * 0.5, (top + bottom) * 0.5 + 7.0)),
        egui::Align2::CENTER_CENTER,
        caption,
        FontId::monospace(9.0),
        MUTED,
    );
}

/// The two ends of a two-terminal part.
fn ends(ctx: &Ctx) -> (Vec2, Vec2) {
    let mut points: Vec<Vec2> = ctx.pins.iter().map(|(_, p)| p.to_vec2()).collect();
    points.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap_or(std::cmp::Ordering::Equal));
    (
        points.first().copied().unwrap_or(ctx.center.to_vec2()),
        points.last().copied().unwrap_or(ctx.center.to_vec2()),
    )
}
