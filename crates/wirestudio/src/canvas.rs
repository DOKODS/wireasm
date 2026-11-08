//! Graph canvas: drawing, pan/zoom, node dragging, pin linking.

use std::collections::BTreeMap;

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Sense, Stroke, StrokeKind, Vec2,
};
use wireasm_core::model::{pins_for, Edge, Node, Target, TY_EXEC};

use crate::app::{sanitize_kind, DragNode, PendingLink, WireStudio};

const BG: Color32 = Color32::from_rgb(18, 20, 26);
const GRID: Color32 = Color32::from_rgb(42, 46, 58);
const BODY: Color32 = Color32::from_rgb(34, 38, 50);
const BODY_HOVER: Color32 = Color32::from_rgb(40, 45, 60);
const BORDER: Color32 = Color32::from_rgb(58, 64, 82);
const TEXT: Color32 = Color32::from_rgb(226, 231, 240);
const MUTED: Color32 = Color32::from_rgb(150, 158, 176);
const ACCENT: Color32 = Color32::from_rgb(96, 165, 250);
const EXEC_PIN: Color32 = Color32::from_rgb(232, 236, 244);
const U64_PIN: Color32 = Color32::from_rgb(79, 209, 197);
const EXEC_WIRE: Color32 = Color32::from_rgb(206, 214, 228);
const DATA_WIRE: Color32 = Color32::from_rgb(66, 190, 178);

const HEADER: f32 = 26.0;
const ROW: f32 = 20.0;

struct PinVis {
    name: String,
    ty: String,
    pos: Pos2,
}

struct NodeVis {
    id: String,
    kind: String,
    rect: Rect,
    title: String,
    preview: Option<String>,
    inputs: Vec<PinVis>,
    outputs: Vec<PinVis>,
}

impl WireStudio {
    pub fn canvas(&mut self, ui: &mut egui::Ui) {
        let (resp, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());
        let rect = resp.rect;
        self.last_canvas = rect;
        if self.fit_requested {
            self.fit_view(rect);
            self.fit_requested = false;
        }

        painter.rect_filled(rect, 0.0, BG);
        self.draw_grid(&painter, rect);

        let layouts = self.compute_layout(rect);
        self.draw_wires(&painter, &layouts, rect);
        for vis in &layouts {
            let hovered = resp
                .hover_pos()
                .map(|p| vis.rect.contains(p))
                .unwrap_or(false);
            let selected = self.selected.as_deref() == Some(vis.id.as_str());
            draw_node(&painter, self, vis, hovered, selected);
        }
        self.draw_pending(&painter, &layouts, ui);
        self.canvas_input(ui, &resp, rect, &layouts);
    }

    fn fit_view(&mut self, rect: Rect) {
        if self.graph.nodes.is_empty() {
            self.view.pan = Vec2::new(-60.0, -120.0);
            self.view.zoom = 1.0;
            return;
        }
        let mut min = Pos2::new(f32::MAX, f32::MAX);
        let mut max = Pos2::new(f32::MIN, f32::MIN);
        for node in &self.graph.nodes {
            min.x = min.x.min(node.x as f32);
            min.y = min.y.min(node.y as f32);
            max.x = max.x.max(node.x as f32 + 230.0);
            max.y = max.y.max(node.y as f32 + 150.0);
        }
        let bw = (max.x - min.x + 120.0).max(320.0);
        let bh = (max.y - min.y + 120.0).max(240.0);
        let zoom = (rect.width() / bw)
            .min(rect.height() / bh)
            .clamp(0.3, 1.5);
        self.view.zoom = zoom;
        let world_center = Pos2::new((min.x + max.x) * 0.5, (min.y + max.y) * 0.5);
        self.view.pan = world_center.to_vec2() - (rect.size() * 0.5) / zoom;
    }

    fn draw_grid(&self, painter: &egui::Painter, rect: Rect) {
        let step = 40.0 * self.view.zoom;
        if step < 8.0 {
            return;
        }
        let start_x = rect.min.x - ((self.view.pan.x * self.view.zoom).rem_euclid(step));
        let start_y = rect.min.y - ((self.view.pan.y * self.view.zoom).rem_euclid(step));
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

    fn compute_layout(&self, rect: Rect) -> Vec<NodeVis> {
        let mut out = Vec::new();
        for node in &self.graph.nodes {
            let (inputs, outputs) = self
                .graph
                .pins(&node.kind)
                .unwrap_or_else(|| (Vec::new(), Vec::new()));
            let title = self.node_title(&node.kind);
            let preview = preview_of(node);
            let rows = inputs.len().max(outputs.len());

            let mut width = 24.0 + title.chars().count() as f32 * 7.4;
            for p in &inputs {
                width = width.max(40.0 + p.name.chars().count() as f32 * 6.6);
            }
            for p in &outputs {
                width = width.max(40.0 + p.name.chars().count() as f32 * 6.6);
            }
            if let Some(view) = &preview {
                width = width.max(24.0 + view.chars().count() as f32 * 6.4);
            }
            let width = width.clamp(150.0, 320.0);
            let height = HEADER
                + rows as f32 * ROW
                + if preview.is_some() { 20.0 } else { 0.0 }
                + 12.0;

            let tl = self
                .view
                .w2s(rect, egui::pos2(node.x as f32, node.y as f32));
            let br = self.view.w2s(
                rect,
                egui::pos2(node.x as f32 + width, node.y as f32 + height),
            );
            let node_rect = Rect::from_min_max(tl, br);

            let mut in_vis = Vec::new();
            for (i, p) in inputs.iter().enumerate() {
                let pos = self.view.w2s(
                    rect,
                    egui::pos2(
                        node.x as f32,
                        node.y as f32 + HEADER + 10.0 + i as f32 * ROW,
                    ),
                );
                in_vis.push(PinVis {
                    name: p.name.clone(),
                    ty: p.ty.clone(),
                    pos,
                });
            }
            let mut out_vis = Vec::new();
            for (i, p) in outputs.iter().enumerate() {
                let pos = self.view.w2s(
                    rect,
                    egui::pos2(
                        node.x as f32 + width,
                        node.y as f32 + HEADER + 10.0 + i as f32 * ROW,
                    ),
                );
                out_vis.push(PinVis {
                    name: p.name.clone(),
                    ty: p.ty.clone(),
                    pos,
                });
            }
            out.push(NodeVis {
                id: node.id.clone(),
                kind: node.kind.clone(),
                rect: node_rect,
                title,
                preview,
                inputs: in_vis,
                outputs: out_vis,
            });
        }
        out
    }

    fn draw_wires(&self, painter: &egui::Painter, layouts: &[NodeVis], rect: Rect) {
        for edge in &self.graph.edges {
            let Some(from) = find_pin(layouts, &edge.from_node, &edge.from_pin, false) else {
                continue;
            };
            let Some(to) = find_pin(layouts, &edge.to_node, &edge.to_pin, true) else {
                continue;
            };
            let color = if from.ty == TY_EXEC {
                EXEC_WIRE
            } else {
                DATA_WIRE
            };
            let width = if from.ty == TY_EXEC { 2.2 } else { 1.7 };
            bezier(painter, from.pos, to.pos, Stroke::new(width, color));
            let _ = rect;
        }
    }

    fn draw_pending(&self, painter: &egui::Painter, layouts: &[NodeVis], ui: &egui::Ui) {
        let Some(link) = &self.pending_link else { return };
        let Some(from) = find_pin(layouts, &link.from_node, &link.from_pin, false) else {
            return;
        };
        let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) else {
            return;
        };
        let color = if link.ty == TY_EXEC { EXEC_WIRE } else { DATA_WIRE };
        bezier(painter, from.pos, cursor, Stroke::new(2.0, color));
        painter.circle_filled(cursor, 3.5, color);
    }

    fn canvas_input(
        &mut self,
        ui: &mut egui::Ui,
        resp: &egui::Response,
        rect: Rect,
        layouts: &[NodeVis],
    ) {
        let (pos, delta, primary_pressed, primary_down, primary_released, middle_down, secondary_down) =
            ui.input(|i| {
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
                    self.view.pan -= scroll / self.view.zoom;
                }
                if (factor - 1.0).abs() > 0.001 {
                    self.view.zoom_at(rect, p, factor);
                }
            }
        }

        if (middle_down || secondary_down)
            && delta != Vec2::ZERO {
                self.view.pan -= delta / self.view.zoom;
            }

        // Drop a node dragged out of the palette.
        if let Some(kind) = self.palette_drag.clone() {
            if primary_released || (!primary_down && pos.is_some()) {
                if let Some(p) = pos {
                    if rect.contains(p) {
                        let world = self.view.s2w(rect, p);
                        self.add_node_at(&kind, world);
                    }
                }
                self.palette_drag = None;
            }
        }

        if primary_pressed {
            let hit = pos.and_then(|p| hit_pin(layouts, p));
            if let Some(pin) = hit {
                if let Some(pending) = self.pending_link.clone() {
                    if !pin.is_output && pin.ty == pending.ty {
                        self.connect_link(
                            &pending.from_node,
                            &pending.from_pin,
                            &pin.node,
                            &pin.name,
                            &pin.ty,
                        );
                        self.pending_link = None;
                    } else if pin.is_output {
                        self.pending_link = Some(PendingLink {
                            from_node: pin.node,
                            from_pin: pin.name,
                            ty: pin.ty,
                        });
                    } else {
                        self.pending_link = None;
                    }
                } else if pin.is_output {
                    self.pending_link = Some(PendingLink {
                        from_node: pin.node,
                        from_pin: pin.name,
                        ty: pin.ty,
                    });
                }
            } else if let Some(p) = pos {
                if rect.contains(p) {
                    if let Some(vis) = hit_node(layouts, p) {
                        self.selected = Some(vis.id.clone());
                        let node = self.graph.node(&vis.id);
                        if let Some(node) = node {
                            let world = self.view.s2w(rect, p);
                            self.drag_node = Some(DragNode {
                                id: vis.id.clone(),
                                grab: world.to_vec2()
                                    - egui::pos2(node.x as f32, node.y as f32).to_vec2(),
                                moved: false,
                            });
                        }
                    } else {
                        self.selected = None;
                        self.pending_link = None;
                        self.panning = true;
                    }
                } else {
                    self.pending_link = None;
                }
            }
        }

        if primary_down {
            if let Some(mut drag) = self.drag_node.clone() {
                if let Some(p) = pos {
                    let world = self.view.s2w(rect, p);
                    let target = world - drag.grab;
                    let mut changed = false;
                    let nx = target.x as f64;
                    let ny = target.y as f64;
                    if let Some(i) = self.graph.nodes.iter().position(|n| n.id == drag.id) {
                        let node = &self.graph.nodes[i];
                        if (node.x - nx).abs() > 0.01 || (node.y - ny).abs() > 0.01 {
                            if !drag.moved {
                                self.snapshot();
                                drag.moved = true;
                            }
                            self.graph.nodes[i].x = nx;
                            self.graph.nodes[i].y = ny;
                            changed = true;
                        }
                    }
                    if changed {
                        self.drag_node = Some(drag);
                        self.touch();
                    }
                }
            } else if self.panning && delta != Vec2::ZERO {
                self.view.pan -= delta / self.view.zoom;
            }
        }

        if primary_released {
            self.drag_node = None;
            self.panning = false;
        }

        let _ = resp;
    }

    pub fn add_node_centered(&mut self, kind: &str) {
        let center = self
            .view
            .s2w(self.last_canvas, self.last_canvas.center());
        self.add_node_at(kind, center);
    }

    pub fn add_node_at(&mut self, kind: &str, center: Pos2) {
        if kind == "Entry" && self.graph.nodes.iter().any(|n| n.kind == "Entry") {
            self.status = self.tr("status.one_entry").to_string();
            return;
        }
        if pins_for(kind).is_none() && self.graph.custom(kind).is_none() {
            return;
        }
        self.snapshot();
        let id = self.unique_id(kind);
        let props = self.default_props(kind);
        self.graph.nodes.push(Node {
            id: id.clone(),
            kind: kind.to_string(),
            x: (center.x - 90.0) as f64,
            y: (center.y - 40.0) as f64,
            props,
        });
        self.selected = Some(id);
        self.touch();
    }

    fn unique_id(&self, kind: &str) -> String {
        let base = sanitize_kind(kind).to_ascii_lowercase();
        let mut n = 1;
        loop {
            let id = format!("{base}_{n}");
            if self.graph.node(&id).is_none() {
                return id;
            }
            n += 1;
        }
    }

    fn default_props(&self, kind: &str) -> BTreeMap<String, String> {
        let mut props = BTreeMap::new();
        let target = Target::parse(&self.graph.target).unwrap_or(Target::Aarch64Macos);
        let text = match kind {
            "Const" => Some(("value", "0".to_string())),
            "VarGet" | "VarSet" => Some((
                "name",
                self.graph
                    .all_variable_names()
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| "value".to_string()),
            )),
            "BinOp" => Some(("op", "add".to_string())),
            "Cmp" => Some(("op", "eq".to_string())),
            "PrintStr" => Some(("text", "Hello from WireASM!\\n".to_string())),
            "Call" => Some(("func", "print_u64".to_string())),
            "Syscall" => Some((
                "number",
                match (target.is_macos(), target.is_aarch64()) {
                    (true, _) => "4",
                    (false, true) => "64",
                    (false, false) => "1",
                }
                .to_string(),
            )),
            "RegGet" | "RegSet" => Some((
                "reg",
                if target.is_aarch64() { "x0" } else { "rax" }.to_string(),
            )),
            "RawAsm" => Some((
                "text",
                if target.is_aarch64() {
                    "add {a}, {a}, {b}"
                } else {
                    "add {a}, {b}"
                }
                .to_string(),
            )),
            "Comment" => Some(("text", String::new())),
            _ => None,
        };
        if let Some((k, v)) = text {
            props.insert(k.to_string(), v);
        }
        props
    }

    fn connect_link(
        &mut self,
        from_node: &str,
        from_pin: &str,
        to_node: &str,
        to_pin: &str,
        ty: &str,
    ) {
        if from_node == to_node {
            return;
        }
        self.snapshot();
        self.graph
            .edges
            .retain(|e| !(e.to_node == to_node && e.to_pin == to_pin));
        if ty == TY_EXEC {
            self.graph
                .edges
                .retain(|e| !(e.from_node == from_node && e.from_pin == from_pin));
        }
        self.graph.edges.push(Edge {
            from_node: from_node.to_string(),
            from_pin: from_pin.to_string(),
            to_node: to_node.to_string(),
            to_pin: to_pin.to_string(),
        });
        self.touch();
    }
}

fn preview_of(node: &Node) -> Option<String> {
    let text = match node.kind.as_str() {
        "Const" => format!("= {}", node.prop("value")),
        "VarGet" | "VarSet" => node.prop("name").to_string(),
        "BinOp" | "Cmp" => node.prop("op").to_string(),
        "Call" => node.prop("func").to_string(),
        "Syscall" => format!("#{}", node.prop("number")),
        "RegGet" | "RegSet" => node.prop("reg").to_string(),
        "PrintStr" | "RawAsm" | "Comment" => node.prop("text").replace('\n', " "),
        _ => return None,
    };
    if text.trim().is_empty() {
        return None;
    }
    Some(clip(&text, 34))
}

fn clip(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn find_pin<'a>(
    layouts: &'a [NodeVis],
    node: &str,
    pin: &str,
    input: bool,
) -> Option<&'a PinVis> {
    let vis = layouts.iter().find(|v| v.id == node)?;
    let list = if input { &vis.inputs } else { &vis.outputs };
    list.iter().find(|p| p.name == pin)
}

struct HitPin {
    node: String,
    name: String,
    ty: String,
    is_output: bool,
}

fn hit_pin(layouts: &[NodeVis], p: Pos2) -> Option<HitPin> {
    let radius = 10.0;
    for vis in layouts.iter().rev() {
        for pin in &vis.inputs {
            if pin.pos.distance(p) <= radius {
                return Some(HitPin {
                    node: vis.id.clone(),
                    name: pin.name.clone(),
                    ty: pin.ty.clone(),
                    is_output: false,
                });
            }
        }
        for pin in &vis.outputs {
            if pin.pos.distance(p) <= radius {
                return Some(HitPin {
                    node: vis.id.clone(),
                    name: pin.name.clone(),
                    ty: pin.ty.clone(),
                    is_output: true,
                });
            }
        }
    }
    None
}

fn hit_node(layouts: &[NodeVis], p: Pos2) -> Option<&NodeVis> {
    layouts.iter().rev().find(|v| v.rect.contains(p))
}

fn bezier(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    let dx = ((to.x - from.x).abs() * 0.5).max(40.0);
    let points = [
        from,
        from + egui::vec2(dx, 0.0),
        to - egui::vec2(dx, 0.0),
        to,
    ];
    painter.add(egui::epaint::CubicBezierShape::from_points_stroke(
        points,
        false,
        Color32::TRANSPARENT,
        stroke,
    ));
}

fn header_color(kind: &str, custom: bool) -> Color32 {
    if custom {
        return Color32::from_rgb(96, 76, 44);
    }
    match kind {
        "Entry" => Color32::from_rgb(40, 96, 74),
        "Branch" | "Anchor" => Color32::from_rgb(92, 70, 40),
        "Return" => Color32::from_rgb(105, 55, 60),
        "Const" | "VarGet" | "VarSet" | "BinOp" | "Cmp" | "RegGet" | "RegSet" => {
            Color32::from_rgb(44, 74, 100)
        }
        "PrintU64" | "PrintStr" => Color32::from_rgb(80, 58, 104),
        "Call" | "Syscall" | "RawAsm" => Color32::from_rgb(48, 80, 80),
        "Comment" => Color32::from_rgb(58, 58, 62),
        _ => Color32::from_rgb(58, 64, 82),
    }
}

fn draw_node(
    painter: &egui::Painter,
    app: &WireStudio,
    vis: &NodeVis,
    hovered: bool,
    selected: bool,
) {
    let zoom = app.view.zoom;
    let radius = CornerRadius::same((7.0 * zoom).clamp(3.0, 12.0) as u8);
    let body = if hovered { BODY_HOVER } else { BODY };
    painter.rect_filled(vis.rect, radius, body);
    let stroke = if selected {
        Stroke::new(2.0, ACCENT)
    } else {
        Stroke::new(1.0, BORDER)
    };
    painter.rect_stroke(vis.rect, radius, stroke, StrokeKind::Outside);

    let header = Rect::from_min_size(
        vis.rect.min,
        egui::vec2(vis.rect.width(), HEADER * zoom),
    );
    let header_radius = CornerRadius {
        nw: radius.nw,
        ne: radius.ne,
        sw: 0,
        se: 0,
    };
    let custom = app.graph.custom(&vis.kind).is_some();
    painter.rect_filled(header, header_radius, header_color(&vis.kind, custom));

    let title = clip(&vis.title, 26);
    painter.text(
        header.left_center() + egui::vec2(9.0 * zoom, 0.0),
        Align2::LEFT_CENTER,
        title,
        FontId::proportional((13.0 * zoom).clamp(8.0, 20.0)),
        TEXT,
    );

    let pin_font = FontId::monospace((10.5 * zoom).clamp(7.0, 16.0));
    let label_font = FontId::monospace((10.0 * zoom).clamp(7.0, 15.0));
    for pin in &vis.inputs {
        let color = if pin.ty == TY_EXEC { EXEC_PIN } else { U64_PIN };
        painter.circle_filled(pin.pos, (4.6 * zoom).clamp(2.5, 7.0), color);
        painter.circle_stroke(pin.pos, (4.6 * zoom).clamp(2.5, 7.0), Stroke::new(1.0, BG));
        painter.text(
            pin.pos + egui::vec2(10.0 * zoom, 0.0),
            Align2::LEFT_CENTER,
            clip(&pin.name, 20),
            label_font.clone(),
            MUTED,
        );
    }
    for pin in &vis.outputs {
        let color = if pin.ty == TY_EXEC { EXEC_PIN } else { U64_PIN };
        painter.circle_filled(pin.pos, (4.6 * zoom).clamp(2.5, 7.0), color);
        painter.circle_stroke(pin.pos, (4.6 * zoom).clamp(2.5, 7.0), Stroke::new(1.0, BG));
        painter.text(
            pin.pos - egui::vec2(10.0 * zoom, 0.0),
            Align2::RIGHT_CENTER,
            clip(&pin.name, 20),
            label_font.clone(),
            MUTED,
        );
    }
    let _ = pin_font;

    if let Some(preview) = &vis.preview {
        painter.text(
            vis.rect.center_bottom() - egui::vec2(0.0, 8.0 * zoom),
            Align2::CENTER_BOTTOM,
            clip(preview, 38),
            FontId::monospace((10.0 * zoom).clamp(7.0, 15.0)),
            Color32::from_rgb(176, 186, 204),
        );
    }
}


