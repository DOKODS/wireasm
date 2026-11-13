//! The look of the schematic sheet.
//!
//! Two styles are available. `Multisim` is the light sheet with red wires and
//! blue labels that Multisim draws; `Dark` is the darker sheet the editor
//! used before.

use eframe::egui::{self, Color32, FontId, Pos2, Stroke, Vec2};

use crate::settings::SheetStyle;

/// The sheet background of the light style.
pub const SHEET: Color32 = Color32::from_rgb(244, 244, 240);
/// The dotted grid of the light style.
pub const GRID: Color32 = Color32::from_rgb(198, 200, 196);
/// The stronger grid every fifth dot.
pub const GRID_STRONG: Color32 = Color32::from_rgb(168, 172, 168);
/// A wire of the light style.
pub const WIRE: Color32 = Color32::from_rgb(176, 32, 32);
/// A selected wire.
pub const WIRE_SEL: Color32 = Color32::from_rgb(24, 92, 196);
/// The body of a component.
pub const PART_FILL: Color32 = Color32::from_rgb(250, 250, 248);
/// The outline of a component.
pub const PART_LINE: Color32 = Color32::from_rgb(24, 24, 24);
/// The label of a component: its designator and value.
pub const PART_LABEL: Color32 = Color32::from_rgb(20, 60, 150);
/// The pins and their stubs.
pub const PIN: Color32 = Color32::from_rgb(24, 24, 24);
/// The pin that is under the pointer.
pub const PIN_HOT: Color32 = Color32::from_rgb(220, 120, 20);
/// A node number.
pub const NODE_NUMBER: Color32 = Color32::from_rgb(70, 70, 70);
/// A probe on the sheet.
pub const PROBE: Color32 = Color32::from_rgb(196, 96, 32);
/// The frame of an instrument.
pub const INSTRUMENT_FILL: Color32 = Color32::from_rgb(226, 232, 240);
/// The outline of an instrument.
pub const INSTRUMENT_LINE: Color32 = Color32::from_rgb(40, 64, 120);
/// The selection frame that Multisim draws around a picked component.
pub const SELECT: Color32 = Color32::from_rgb(40, 96, 220);
/// The text inside an instrument.
pub const INSTRUMENT_TEXT: Color32 = Color32::from_rgb(20, 30, 60);
/// The background of a display inside a panel.
pub const PANEL_SCREEN: Color32 = Color32::from_rgb(12, 20, 16);
/// The grid of a display inside a panel.
pub const PANEL_GRID: Color32 = Color32::from_rgb(38, 62, 50);
/// The trace of channel A in a display.
pub const TRACE_A: Color32 = Color32::from_rgb(120, 230, 120);
/// The trace of channel B in a display.
pub const TRACE_B: Color32 = Color32::from_rgb(240, 200, 80);
/// The trace of channel C in a display.
pub const TRACE_C: Color32 = Color32::from_rgb(130, 190, 250);
/// The trace of channel D in a display.
pub const TRACE_D: Color32 = Color32::from_rgb(240, 130, 200);
/// The colours of the four scope channels.
pub const CHANNEL_COLORS: [Color32; 4] = [TRACE_A, TRACE_B, TRACE_C, TRACE_D];

// The dark style.
const DARK_SHEET: Color32 = Color32::from_rgb(16, 18, 24);
const DARK_GRID: Color32 = Color32::from_rgb(36, 40, 52);
const DARK_GRID_STRONG: Color32 = Color32::from_rgb(52, 58, 74);
const DARK_WIRE: Color32 = Color32::from_rgb(104, 196, 172);
const DARK_PART_LINE: Color32 = Color32::from_rgb(206, 214, 228);
const DARK_PART_LABEL: Color32 = Color32::from_rgb(150, 190, 250);
const DARK_PIN: Color32 = Color32::from_rgb(206, 214, 228);
const DARK_NODE_NUMBER: Color32 = Color32::from_rgb(150, 158, 176);

impl SheetStyle {
    /// The background of the sheet.
    pub fn sheet(self) -> Color32 {
        match self {
            SheetStyle::Multisim => SHEET,
            SheetStyle::Dark => DARK_SHEET,
        }
    }

    /// The colour of a wire.
    pub fn wire(self) -> Color32 {
        match self {
            SheetStyle::Multisim => WIRE,
            SheetStyle::Dark => DARK_WIRE,
        }
    }

    /// The outline of a component body.
    pub fn part_line(self) -> Color32 {
        match self {
            SheetStyle::Multisim => PART_LINE,
            SheetStyle::Dark => DARK_PART_LINE,
        }
    }

    /// The fill of a component body.
    pub fn part_fill(self) -> Color32 {
        match self {
            SheetStyle::Multisim => PART_FILL,
            SheetStyle::Dark => Color32::from_rgb(32, 36, 48),
        }
    }

    /// The colour of the designator and the value.
    pub fn part_label(self) -> Color32 {
        match self {
            SheetStyle::Multisim => PART_LABEL,
            SheetStyle::Dark => DARK_PART_LABEL,
        }
    }

    /// The colour of the pins.
    pub fn pin(self) -> Color32 {
        match self {
            SheetStyle::Multisim => PIN,
            SheetStyle::Dark => DARK_PIN,
        }
    }

    /// The colour of the node numbers.
    pub fn node_number(self) -> Color32 {
        match self {
            SheetStyle::Multisim => NODE_NUMBER,
            SheetStyle::Dark => DARK_NODE_NUMBER,
        }
    }

    fn grid(self) -> Color32 {
        match self {
            SheetStyle::Multisim => GRID,
            SheetStyle::Dark => DARK_GRID,
        }
    }

    fn grid_strong(self) -> Color32 {
        match self {
            SheetStyle::Multisim => GRID_STRONG,
            SheetStyle::Dark => DARK_GRID_STRONG,
        }
    }
}

/// Draw the dotted grid of the sheet.
pub fn draw_grid(
    painter: &egui::Painter,
    rect: egui::Rect,
    zoom: f32,
    pan: Vec2,
    style: SheetStyle,
) {
    let step = 10.0 * zoom;
    if step < 5.0 {
        return;
    }
    let start_x = rect.min.x - ((pan.x * zoom).rem_euclid(step));
    let start_y = rect.min.y - ((pan.y * zoom).rem_euclid(step));
    let mut x = start_x;
    while x < rect.right() {
        let mut y = start_y;
        while y < rect.bottom() {
            let strong = ((x - start_x) / step).round() % 5.0 == 0.0
                && ((y - start_y) / step).round() % 5.0 == 0.0;
            painter.circle_filled(
                egui::pos2(x, y),
                if strong { 1.2 } else { 0.9 },
                if strong {
                    style.grid_strong()
                } else {
                    style.grid()
                },
            );
            y += step;
        }
        x += step;
    }
}

/// Draw a label next to a component.
///
/// Multisim prints the designator above the symbol and the value under it,
/// both centred on the body.
pub fn draw_label(
    painter: &egui::Painter,
    center: Pos2,
    designator: &str,
    value: &str,
    zoom: f32,
    half_height: f32,
    style: SheetStyle,
) {
    let size = 11.0;
    let scale = zoom.max(0.8);
    painter.text(
        center + Vec2::new(0.0, -half_height - 3.0 * scale),
        egui::Align2::CENTER_BOTTOM,
        designator,
        FontId::proportional(size * scale),
        style.part_label(),
    );
    if !value.is_empty() {
        painter.text(
            center + Vec2::new(0.0, half_height + 3.0 * scale),
            egui::Align2::CENTER_TOP,
            value,
            FontId::proportional(size * scale),
            style.part_label(),
        );
    }
}

/// Draw the number of a node next to the wire that carries it.
pub fn draw_node_number(
    painter: &egui::Painter,
    at: Pos2,
    number: &str,
    zoom: f32,
    style: SheetStyle,
) {
    painter.text(
        at,
        egui::Align2::LEFT_BOTTOM,
        number,
        FontId::proportional(10.0 * zoom.max(0.8)),
        style.node_number(),
    );
}

/// Draw a junction dot where wires meet.
pub fn draw_junction(painter: &egui::Painter, at: Pos2, zoom: f32, style: SheetStyle) {
    painter.circle_filled(at, 2.4 * zoom.max(0.6), style.wire());
}

/// The frame of an instrument on the sheet.
pub fn instrument_frame(zoom: f32) -> Stroke {
    Stroke::new(1.4 * zoom.max(0.7), INSTRUMENT_LINE)
}
