//! The front panels of the virtual instruments.
//!
//! Multisim shows an instrument as a small box on the sheet and opens a front
//! panel when it is double-clicked. The panel holds a display and the knobs of
//! the instrument. This module draws those panels.

use eframe::egui::{self, Color32, FontId, Pos2, Rect, Stroke};

use wireasm_sim::trace::{AcSweep, Trace};

use crate::sheet;

/// The kind of an instrument on the sheet.
pub type Instrument = wireasm_sim::netlist::InstrumentKind;

/// What a panel shows. The data is owned, so a panel can outlive the run
/// that produced it.
#[derive(Clone, Default)]
pub enum Reading {
    /// Nothing has been measured yet.
    #[default]
    Empty,
    /// The result of a transient analysis.
    Transient(Trace),
    /// The result of an AC sweep.
    Ac(AcSweep),
    /// The result of a DC operating point.
    Dc(wireasm_sim::solver::Solution),
}

/// The channels of an instrument: the nets they watch and what they measured.
#[derive(Clone, Default)]
pub struct Channels {
    /// The net of every channel.
    pub nets: Vec<String>,
    /// The measured values of every channel.
    pub values: Vec<Vec<f64>>,
}

impl Channels {
    pub fn new(nets: Vec<String>) -> Channels {
        Channels {
            nets,
            values: Vec::new(),
        }
    }

    /// The values of one channel.
    pub fn series(&self, index: usize) -> Option<&Vec<f64>> {
        self.values.get(index)
    }
}

/// Draw the display of an instrument: a dark screen with a grid, the way the
/// real faceplates look.
pub fn display(
    ui: &mut egui::Ui,
    rect: Rect,
    title: &str,
    channels: &Channels,
    reading: &Reading,
    kind: Instrument,
) {
    let painter = ui.painter();
    // The screen.
    painter.rect_filled(rect, 2.0, sheet::PANEL_SCREEN);
    painter.rect_stroke(
        rect,
        2.0,
        Stroke::new(1.0, sheet::INSTRUMENT_LINE),
        egui::StrokeKind::Middle,
    );

    // The grid of the screen: ten divisions each way, as on a real scope.
    let cols = 10;
    let rows = 8;
    for i in 1..cols {
        let x = rect.left() + rect.width() * i as f32 / cols as f32;
        painter.line_segment(
            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
            Stroke::new(0.5, sheet::PANEL_GRID),
        );
    }
    for i in 1..rows {
        let y = rect.top() + rect.height() * i as f32 / rows as f32;
        painter.line_segment(
            [Pos2::new(rect.left(), y), Pos2::new(rect.right(), y)],
            Stroke::new(0.5, sheet::PANEL_GRID),
        );
    }
    // The centre line.
    painter.line_segment(
        [
            Pos2::new(rect.left(), rect.center().y),
            Pos2::new(rect.right(), rect.center().y),
        ],
        Stroke::new(0.8, sheet::PANEL_GRID),
    );

    painter.text(
        Pos2::new(rect.right() - 6.0, rect.top() + 4.0),
        egui::Align2::RIGHT_TOP,
        title,
        FontId::monospace(11.0),
        Color32::from_rgb(150, 200, 150),
    );

    match (kind, reading) {
        (Instrument::Scope, Reading::Transient(trace)) => {
            draw_scope(painter, rect, channels, trace);
        }
        (Instrument::Scope, Reading::Ac(sweep)) => {
            draw_bode(painter, rect, channels, sweep);
        }
        (Instrument::BodePlotter, Reading::Ac(sweep)) => {
            draw_bode(painter, rect, channels, sweep);
        }
        (Instrument::LogicAnalyzer, Reading::Transient(trace)) => {
            draw_logic(painter, rect, channels, trace);
        }
        (Instrument::Multimeter, Reading::Dc(sol)) => {
            let value = channels.nets.first().map(|n| sol.node(n)).unwrap_or(0.0);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{value:.4} V"),
                FontId::monospace(28.0),
                Color32::from_rgb(140, 240, 140),
            );
        }
        (Instrument::Multimeter, Reading::Transient(trace)) => {
            let value = channels
                .nets
                .first()
                .and_then(|n| trace.series.get(n))
                .and_then(|s| s.last())
                .copied()
                .unwrap_or(0.0);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{value:.4} V"),
                FontId::monospace(28.0),
                Color32::from_rgb(140, 240, 140),
            );
        }
        (Instrument::Wattmeter, Reading::Dc(sol)) => {
            let value = channels.nets.first().map(|n| sol.node(n)).unwrap_or(0.0);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{value:.4} W"),
                FontId::monospace(24.0),
                Color32::from_rgb(140, 240, 140),
            );
        }
        (Instrument::FrequencyCounter, Reading::Transient(trace)) => {
            let freq = measure_frequency(trace);
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                match freq {
                    Some(f) => format!("{f:.1} Hz"),
                    None => "—".to_string(),
                },
                FontId::monospace(24.0),
                Color32::from_rgb(140, 240, 140),
            );
        }
        _ => {
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "no signal",
                FontId::monospace(12.0),
                Color32::from_rgb(90, 130, 100),
            );
        }
    }
}

/// Draw the traces of the scope channels.
fn draw_scope(painter: &egui::Painter, rect: Rect, channels: &Channels, trace: &Trace) {
    if trace.time.is_empty() {
        return;
    }
    let t0 = trace.time[0];
    let t1 = trace.time[trace.time.len() - 1];
    let span = (t1 - t0).max(1e-12);
    let mut v_min = f64::INFINITY;
    let mut v_max = f64::NEG_INFINITY;
    for index in 0..channels.nets.len() {
        let Some(series) = channels.series(index) else {
            continue;
        };
        for v in series {
            v_min = v_min.min(*v);
            v_max = v_max.max(*v);
        }
    }
    if !v_min.is_finite() {
        return;
    }
    if (v_max - v_min).abs() < 1e-9 {
        v_max += 0.5;
        v_min -= 0.5;
    }
    let pad = (v_max - v_min) * 0.1;
    v_min -= pad;
    v_max += pad;
    let span_v = v_max - v_min;

    for index in 0..channels.nets.len() {
        let Some(series) = channels.series(index) else {
            continue;
        };
        let color = sheet::CHANNEL_COLORS[index % sheet::CHANNEL_COLORS.len()];
        let mut points: Vec<Pos2> = Vec::new();
        for (k, v) in series.iter().enumerate() {
            let Some(t) = trace.time.get(k) else { continue };
            let x = rect.left() + rect.width() * ((t - t0) / span) as f32;
            let y = rect.bottom() - rect.height() * ((v - v_min) / span_v) as f32;
            points.push(Pos2::new(x, y));
        }
        if points.len() > 1 {
            painter.add(egui::Shape::line(points, Stroke::new(1.6, color)));
        }
        // Only the channels that carry a signal are named, so the legend does
        // not pile up.
        if channels
            .series(index)
            .map(|s| !s.is_empty())
            .unwrap_or(false)
        {
            painter.text(
                Pos2::new(rect.left() + 6.0, rect.top() + 4.0 + index as f32 * 13.0),
                egui::Align2::LEFT_TOP,
                channels.nets[index].clone(),
                FontId::monospace(10.0),
                color,
            );
        }
    }
}

/// Draw the magnitude and phase of a Bode plot.
fn draw_bode(painter: &egui::Painter, rect: Rect, channels: &Channels, sweep: &AcSweep) {
    let Some(out) = channels.nets.get(1).or_else(|| channels.nets.first()) else {
        return;
    };
    let mag = sweep.magnitude(out);
    if mag.is_empty() {
        return;
    }
    let db: Vec<f64> = mag.iter().map(|m| 20.0 * m.max(1e-18).log10()).collect();
    let mut lo = db.iter().cloned().fold(f64::INFINITY, f64::min);
    let mut hi = db.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if (hi - lo).abs() < 1e-9 {
        hi += 1.0;
        lo -= 1.0;
    }
    let f0 = sweep.frequency[0].max(1e-12).log10();
    let f1 = sweep.frequency[sweep.frequency.len() - 1]
        .max(1e-12)
        .log10();
    let span = (f1 - f0).max(1e-9);
    let mut points = Vec::new();
    for (k, v) in db.iter().enumerate() {
        let f = sweep.frequency[k].max(1e-12).log10();
        let x = rect.left() + rect.width() * ((f - f0) / span) as f32;
        let y = rect.bottom() - rect.height() * ((v - lo) / (hi - lo)) as f32;
        points.push(Pos2::new(x, y));
    }
    if points.len() > 1 {
        painter.add(egui::Shape::line(points, Stroke::new(1.6, sheet::TRACE_B)));
    }
    painter.text(
        Pos2::new(rect.right() - 6.0, rect.top() + 4.0),
        egui::Align2::RIGHT_TOP,
        format!("{hi:.1} dB … {lo:.1} dB"),
        FontId::monospace(10.0),
        Color32::from_rgb(150, 200, 150),
    );
}

/// Draw the channels of a logic analyzer as square waves.
fn draw_logic(painter: &egui::Painter, rect: Rect, channels: &Channels, trace: &Trace) {
    if trace.time.is_empty() {
        return;
    }
    let t0 = trace.time[0];
    let t1 = trace.time[trace.time.len() - 1];
    let span = (t1 - t0).max(1e-12);
    let visible = channels.nets.len().min(8);
    if visible == 0 {
        return;
    }
    let row = rect.height() / visible as f32;
    for (index, net) in channels.nets.iter().take(visible).enumerate() {
        let Some(series) = trace.series.get(net) else {
            continue;
        };
        let color = sheet::CHANNEL_COLORS[index % sheet::CHANNEL_COLORS.len()];
        let top = rect.top() + row * index as f32 + row * 0.2;
        let bottom = top + row * 0.6;
        let mut points: Vec<Pos2> = Vec::new();
        for (k, v) in series.iter().enumerate() {
            let Some(t) = trace.time.get(k) else { continue };
            let x = rect.left() + rect.width() * ((t - t0) / span) as f32;
            let high = *v > 1.5;
            let y = if high { top } else { bottom };
            if let Some(last) = points.last() {
                if (last.y - y).abs() > 0.5 {
                    points.push(Pos2::new(x, last.y));
                }
            }
            points.push(Pos2::new(x, y));
        }
        if points.len() > 1 {
            painter.add(egui::Shape::line(points, Stroke::new(1.4, color)));
        }
        painter.text(
            Pos2::new(rect.left() + 4.0, top - 1.0),
            egui::Align2::LEFT_BOTTOM,
            format!("{}", index),
            FontId::monospace(10.0),
            color,
        );
    }
}

/// The frequency of the last few periods of a signal.
fn measure_frequency(trace: &Trace) -> Option<f64> {
    let series = trace.series.values().next()?;
    if series.len() < 4 {
        return None;
    }
    let v_max = series.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let v_min = series.iter().cloned().fold(f64::INFINITY, f64::min);
    let threshold = (v_max + v_min) * 0.5;
    let mut edges: Vec<f64> = Vec::new();
    for k in 1..series.len() {
        let before = series[k - 1] < threshold;
        let after = series[k] < threshold;
        if before != after {
            edges.push(trace.time[k]);
        }
    }
    if edges.len() < 3 {
        return None;
    }
    let first = edges[edges.len() - 3];
    let last = edges[edges.len() - 1];
    let periods = (edges.len() - 3) as f64;
    let span = last - first;
    if span <= 0.0 {
        return None;
    }
    Some(periods / span)
}

/// The icon of an instrument on the sheet: a small box with its terminals.
pub fn draw_icon(
    painter: &egui::Painter,
    rect: Rect,
    label: &str,
    terminals: &[(&str, Pos2)],
    zoom: f32,
    selected: bool,
) {
    painter.rect_filled(rect, 2.0, sheet::INSTRUMENT_FILL);
    painter.rect_stroke(
        rect,
        2.0,
        if selected {
            Stroke::new(2.0, sheet::SELECT)
        } else {
            sheet::instrument_frame(zoom)
        },
        egui::StrokeKind::Middle,
    );
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        FontId::proportional(11.0 * zoom.max(0.8)),
        sheet::INSTRUMENT_TEXT,
    );
    // The terminals stick out of the box.
    for (name, at) in terminals {
        painter.line_segment(
            [*at, Pos2::new(at.x, rect.center().y)],
            Stroke::new(1.2, sheet::PIN),
        );
        painter.text(
            Pos2::new(at.x, rect.center().y - 2.0),
            egui::Align2::CENTER_BOTTOM,
            *name,
            FontId::proportional(9.0 * zoom.max(0.8)),
            sheet::INSTRUMENT_TEXT,
        );
    }
}
