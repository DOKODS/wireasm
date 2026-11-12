//! The look of the schematic editor, borrowed from Multisim.
//!
//! Multisim draws a light sheet with a dotted grid, wires in red with
//! right-angle bends, black component bodies with blue labels, node numbers
//! next to every net, and the instruments as small blue boxes on the sheet.
//! This module holds that palette and the helpers that draw it.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke, Vec2};

/// The sheet background: light, like a sheet of paper.
pub const SHEET: Color32 = Color32::from_rgb(244, 244, 240);
/// The dotted grid on the sheet.
pub const GRID: Color32 = Color32::from_rgb(198, 200, 196);
/// The stronger grid every fifth dot.
pub const GRID_STRONG: Color32 = Color32::from_rgb(168, 172, 168);
/// A wire on the sheet.
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
/// The text inside an instrument.
pub const INSTRUMENT_TEXT: Color32 = Color32::from_rgb(20, 30, 60);
/// The selection frame that Multisim draws around a picked component.
pub const SELECT: Color32 = Color32::from_rgb(40, 96, 220);
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

/// Draw the dotted grid of the sheet.
pub fn draw_grid(painter: &egui::Painter, rect: Rect, zoom: f32, pan: Vec2) {
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
                if strong { GRID_STRONG } else { GRID },
            );
            y += step;
        }
        x += step;
    }
}

/// Draw a label next to a component: the designator above, the value below.
pub fn draw_label(
    painter: &egui::Painter,
    center: Pos2,
    designator: &str,
    value: &str,
    zoom: f32,
    half_height: f32,
) {
    let size = 11.0;
    let scale = zoom.max(0.8);
    painter.text(
        center + Vec2::new(0.0, -half_height - 3.0 * scale),
        egui::Align2::CENTER_BOTTOM,
        designator,
        FontId::proportional(size * scale),
        PART_LABEL,
    );
    if !value.is_empty() {
        painter.text(
            center + Vec2::new(0.0, half_height + 3.0 * scale),
            egui::Align2::CENTER_TOP,
            value,
            FontId::proportional(size * scale),
            PART_LABEL,
        );
    }
}

/// Draw the number of a node next to the wire that carries it.
pub fn draw_node_number(painter: &egui::Painter, at: Pos2, number: &str, zoom: f32) {
    painter.text(
        at,
        egui::Align2::LEFT_BOTTOM,
        number,
        FontId::proportional(10.0 * zoom.max(0.8)),
        NODE_NUMBER,
    );
}

/// Draw a junction dot where wires meet.
pub fn draw_junction(painter: &egui::Painter, at: Pos2, zoom: f32) {
    painter.circle_filled(at, 2.4 * zoom.max(0.6), WIRE);
}

/// The frame of an instrument on the sheet.
pub fn instrument_frame(zoom: f32) -> Stroke {
    Stroke::new(1.4 * zoom.max(0.7), INSTRUMENT_LINE)
}
