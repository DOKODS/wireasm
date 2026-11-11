//! Inspector, variables editor and the custom-node editor window.

use std::collections::BTreeSet;

use eframe::egui::{self, Color32, DragValue, RichText};

use wireasm_core::model::{CustomNodeDef, CustomPin, Target, TY_EXEC, TY_U64};

use crate::app::{CustomEditor, WireStudio};

const BIN_OPS: &[&str] = &["add", "sub", "mul", "div", "and", "or", "xor", "shl", "shr"];
const CMP_OPS: &[&str] = &["eq", "ne", "lt", "le", "gt", "ge"];
const A64_REGS: &[&str] = &[
    "x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9", "x10", "x11", "x12", "x13", "x14",
    "x15", "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27", "x28", "fp", "lr",
];
const X64_REGS: &[&str] = &[
    "rax", "rbx", "rcx", "rdx", "rsi", "rdi", "r8", "r9", "r10", "r11", "r12", "r13", "r14", "r15",
];
const AVR_REGS: &[&str] = &[
    "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13", "r14",
    "r15", "r16", "r17", "r18", "r19", "r20", "r21", "r22", "r23", "r24", "r25", "r26", "r27",
    "r28", "r29", "r30", "r31",
];

impl WireStudio {
    // ------------------------------------------------------------- inspector

    pub fn inspector_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::right("wire_inspector")
            .resizable(true)
            .default_size(286.0)
            .min_size(220.0)
            .show(ui, |ui| {
                ui.add_space(4.0);
                ui.heading(self.tr("inspector.title"));
                ui.separator();
                let Some(id) = self.selected.clone() else {
                    ui.label(RichText::new(self.tr("inspector.empty")).weak());
                    return;
                };
                let Some(node) = self.graph.node(&id).cloned() else {
                    ui.label(RichText::new(self.tr("inspector.empty")).weak());
                    return;
                };

                ui.label(RichText::new(self.node_title(&node.kind)).strong());
                egui::Grid::new("wire_node_grid")
                    .num_columns(2)
                    .spacing([8.0, 3.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new(self.tr("inspector.id")).small().weak());
                        ui.label(RichText::new(&node.id).monospace());
                        ui.end_row();
                        ui.label(RichText::new(self.tr("inspector.kind")).small().weak());
                        ui.label(RichText::new(&node.kind).monospace());
                        ui.end_row();
                        ui.label(RichText::new(self.tr("inspector.position")).small().weak());
                        ui.label(
                            RichText::new(format!("{:.0}, {:.0}", node.x, node.y)).monospace(),
                        );
                        ui.end_row();
                    });

                let custom = self.graph.custom(&node.kind).cloned();
                if let Some(def) = &custom {
                    ui.add_space(2.0);
                    if ui.button(self.tr("inspector.edit_definition")).clicked() {
                        self.custom_editor = Some(CustomEditor::new(Some(def.clone())));
                    }
                }

                ui.add_space(6.0);
                ui.separator();
                ui.label(RichText::new(self.tr("inspector.properties")).strong());
                ui.add_space(2.0);

                match node.kind.as_str() {
                    "Const" => self.prop_value(&node.id, "value", ui),
                    "VarGet" | "VarSet" => self.prop_variable(&node.id, ui),
                    "BinOp" => self.prop_choice(&node.id, "op", "inspector.operator", BIN_OPS, ui),
                    "Cmp" => self.prop_choice(&node.id, "op", "inspector.operator", CMP_OPS, ui),
                    "PrintStr" | "RawAsm" | "Comment" => {
                        let multiline = node.kind != "Comment";
                        self.prop_text(&node.id, "text", "inspector.text", multiline, ui)
                    }
                    "Call" => self.prop_text(&node.id, "func", "inspector.function", false, ui),
                    "Syscall" => {
                        self.prop_number(&node.id, "number", "inspector.syscall_number", ui)
                    }
                    "RegGet" | "RegSet" => self.prop_register(&node.id, ui),
                    "McuPinMode" => {
                        self.prop_range(&node.id, "pin", "inspector.pin", 0, 19, ui);
                        self.prop_choice(
                            &node.id,
                            "mode",
                            "inspector.pin_mode",
                            &["out", "in", "pullup"],
                            ui,
                        );
                        ui.label(RichText::new(self.tr("inspector.pin_hint")).small().weak());
                    }
                    "McuDigitalWrite" | "McuDigitalRead" => {
                        self.prop_range(&node.id, "pin", "inspector.pin", 0, 19, ui);
                        ui.label(RichText::new(self.tr("inspector.pin_hint")).small().weak());
                    }
                    "McuUartInit" => {
                        self.prop_range(&node.id, "baud", "inspector.baud", 300, 1_000_000, ui)
                    }
                    "McuUartWriteStr" => {
                        self.prop_text(&node.id, "text", "inspector.text", true, ui)
                    }
                    "McuAdcRead" => {
                        self.prop_range(&node.id, "channel", "inspector.channel", 0, 7, ui)
                    }
                    "McuRegisterWrite" | "McuRegisterRead" => {
                        self.prop_text(&node.id, "address", "inspector.address", false, ui);
                        ui.label(RichText::new(self.tr("inspector.reg_hint")).small().weak());
                    }
                    other if custom.is_some() => {
                        let def = custom.as_ref().expect("checked");
                        let keys = prop_keys(def);
                        let mut any = false;
                        for key in keys {
                            any = true;
                            self.prop_text(&node.id, &key, &key, false, ui);
                        }
                        if !any {
                            ui.label(RichText::new(self.tr("inspector.no_props")).small().weak());
                        }
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            let hint = self.tr("inspector.prop_key");
                            ui.add(
                                egui::TextEdit::singleline(&mut self.new_prop_key)
                                    .hint_text(hint)
                                    .desired_width(130.0),
                            );
                            if ui.button(self.tr("inspector.add")).clicked() {
                                let key = self.new_prop_key.trim().to_string();
                                if !key.is_empty() {
                                    self.snapshot();
                                    if let Some(n) = self.graph.node_mut(&node.id) {
                                        n.props.entry(key).or_default();
                                    }
                                    self.new_prop_key.clear();
                                    self.touch();
                                }
                            }
                        });
                        let _ = other;
                    }
                    _ => {
                        ui.label(RichText::new(self.tr("inspector.no_props")).small().weak());
                    }
                }

                ui.add_space(10.0);
                ui.separator();
                if ui.button(self.tr("inspector.delete")).clicked() {
                    self.delete_selected();
                }
            });
    }

    fn prop_text(&mut self, id: &str, key: &str, label: &str, multiline: bool, ui: &mut egui::Ui) {
        let mut value = self
            .graph
            .node(id)
            .map(|n| n.prop(key).to_string())
            .unwrap_or_default();
        ui.label(RichText::new(label).small().weak());
        let edit = if multiline {
            egui::TextEdit::multiline(&mut value)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .code_editor()
        } else {
            egui::TextEdit::singleline(&mut value).desired_width(f32::INFINITY)
        };
        let resp = ui.add(edit);
        if resp.gained_focus() {
            self.snapshot();
        }
        if resp.changed() {
            if let Some(node) = self.graph.node_mut(id) {
                node.props.insert(key.to_string(), value);
            }
            self.touch();
        }
        ui.add_space(3.0);
    }

    fn prop_value(&mut self, id: &str, key: &str, ui: &mut egui::Ui) {
        let mut value = self
            .graph
            .node(id)
            .map(|n| n.prop_u64(key))
            .unwrap_or_default();
        ui.label(RichText::new(self.tr("inspector.value")).small().weak());
        let resp = ui.add(DragValue::new(&mut value).speed(1.0));
        if resp.drag_started() || resp.gained_focus() {
            self.snapshot();
        }
        if resp.changed() {
            if let Some(node) = self.graph.node_mut(id) {
                node.props.insert(key.to_string(), value.to_string());
            }
            self.touch();
        }
        ui.add_space(3.0);
    }

    fn prop_range(
        &mut self,
        id: &str,
        key: &str,
        label: &str,
        min: u64,
        max: u64,
        ui: &mut egui::Ui,
    ) {
        let mut value = self
            .graph
            .node(id)
            .map(|n| n.prop_u64(key))
            .unwrap_or_default()
            .clamp(min, max);
        ui.label(RichText::new(self.tr(label)).small().weak());
        let resp = ui.add(DragValue::new(&mut value).speed(1.0).range(min..=max));
        if resp.drag_started() || resp.gained_focus() {
            self.snapshot();
        }
        if resp.changed() {
            if let Some(node) = self.graph.node_mut(id) {
                node.props.insert(key.to_string(), value.to_string());
            }
            self.touch();
        }
        ui.add_space(3.0);
    }

    fn prop_number(&mut self, id: &str, key: &str, label: &str, ui: &mut egui::Ui) {
        let mut value = self
            .graph
            .node(id)
            .map(|n| n.prop_u64(key))
            .unwrap_or_default();
        ui.label(RichText::new(self.tr(label)).small().weak());
        let resp = ui.add(DragValue::new(&mut value).speed(1.0));
        if resp.drag_started() || resp.gained_focus() {
            self.snapshot();
        }
        if resp.changed() {
            if let Some(node) = self.graph.node_mut(id) {
                node.props.insert(key.to_string(), value.to_string());
            }
            self.touch();
        }
        ui.add_space(3.0);
    }

    fn prop_choice(
        &mut self,
        id: &str,
        key: &str,
        label: &str,
        choices: &[&str],
        ui: &mut egui::Ui,
    ) {
        let current = self
            .graph
            .node(id)
            .map(|n| n.prop(key).to_string())
            .unwrap_or_default();
        ui.label(RichText::new(self.tr(label)).small().weak());
        let mut pick = None;
        egui::ComboBox::from_id_salt(format!("prop_{id}_{key}"))
            .selected_text(current.clone())
            .width(ui.available_width().min(200.0))
            .show_ui(ui, |ui| {
                for choice in choices {
                    if ui.selectable_label(current == *choice, *choice).clicked() {
                        pick = Some(choice.to_string());
                    }
                }
            });
        if let Some(choice) = pick {
            self.snapshot();
            if let Some(node) = self.graph.node_mut(id) {
                node.props.insert(key.to_string(), choice);
            }
            self.touch();
        }
        ui.add_space(3.0);
    }

    fn prop_variable(&mut self, id: &str, ui: &mut egui::Ui) {
        let names = self.graph.all_variable_names();
        let current = self
            .graph
            .node(id)
            .map(|n| n.prop("name").to_string())
            .unwrap_or_default();
        ui.label(RichText::new(self.tr("inspector.var_name")).small().weak());
        if names.is_empty() {
            ui.label(RichText::new(self.tr("vars.empty")).small().weak());
        } else {
            let mut pick = None;
            egui::ComboBox::from_id_salt(format!("var_{id}"))
                .selected_text(current.clone())
                .width(ui.available_width().min(200.0))
                .show_ui(ui, |ui| {
                    for name in &names {
                        if ui.selectable_label(&current == name, name).clicked() {
                            pick = Some(name.clone());
                        }
                    }
                });
            if let Some(name) = pick {
                self.snapshot();
                if let Some(node) = self.graph.node_mut(id) {
                    node.props.insert("name".to_string(), name);
                }
                self.touch();
            }
        }
        ui.add_space(3.0);
    }

    fn prop_register(&mut self, id: &str, ui: &mut egui::Ui) {
        let target = Target::parse(&self.graph.target).unwrap_or(Target::Aarch64Macos);
        let regs = if target.is_avr() {
            AVR_REGS
        } else if target.is_aarch64() {
            A64_REGS
        } else {
            X64_REGS
        };
        let current = self
            .graph
            .node(id)
            .map(|n| n.prop("reg").to_string())
            .unwrap_or_default();
        ui.label(RichText::new(self.tr("inspector.register")).small().weak());
        let mut pick = None;
        egui::ComboBox::from_id_salt(format!("reg_{id}"))
            .selected_text(current.clone())
            .width(ui.available_width().min(200.0))
            .show_ui(ui, |ui| {
                for reg in regs {
                    if ui.selectable_label(current == *reg, *reg).clicked() {
                        pick = Some(reg.to_string());
                    }
                }
            });
        if let Some(reg) = pick {
            self.snapshot();
            if let Some(node) = self.graph.node_mut(id) {
                node.props.insert("reg".to_string(), reg);
            }
            self.touch();
        }
        ui.add_space(3.0);
    }

    // ------------------------------------------------------------- variables

    pub fn variables_window(&mut self, ctx: &egui::Context) {
        if !self.variables_open {
            return;
        }
        let mut open = true;
        let mut close = false;
        egui::Window::new(self.tr("vars.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(410.0, 320.0))
            .show(ctx, |ui| {
                if self.graph.variables.is_empty() {
                    ui.label(RichText::new(self.tr("vars.empty")).weak());
                }
                let mut remove: Option<usize> = None;
                for i in 0..self.graph.variables.len() {
                    let mut name = self.graph.variables[i].name.clone();
                    let mut init = self.graph.variables[i].init;
                    ui.horizontal(|ui| {
                        ui.label(self.tr("vars.name"));
                        let resp =
                            ui.add(egui::TextEdit::singleline(&mut name).desired_width(150.0));
                        if resp.gained_focus() {
                            self.snapshot();
                        }
                        if resp.changed() {
                            let old = self.graph.variables[i].name.clone();
                            self.rename_variable(&old, &name);
                            self.graph.variables[i].name = name.clone();
                            self.touch();
                        }
                        ui.label(self.tr("vars.init"));
                        let init_resp = ui.add(DragValue::new(&mut init).speed(1.0));
                        if init_resp.drag_started() || init_resp.gained_focus() {
                            self.snapshot();
                        }
                        if init_resp.changed() {
                            self.graph.variables[i].init = init;
                            self.touch();
                        }
                        if ui.button(self.tr("vars.remove")).clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if let Some(i) = remove {
                    self.snapshot();
                    self.graph.variables.remove(i);
                    self.touch();
                }
                ui.add_space(6.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(self.tr("vars.add")).clicked() {
                        self.snapshot();
                        let name = self.unique_variable_name();
                        self.graph
                            .variables
                            .push(wireasm_core::model::Variable { name, init: 0 });
                        self.touch();
                    }
                    if ui.button(self.tr("vars.close")).clicked() {
                        close = true;
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new(self.tr("vars.hint")).small().weak());
            });
        if close || !open {
            self.variables_open = false;
        }
    }

    fn unique_variable_name(&self) -> String {
        let mut n = 1;
        loop {
            let name = format!("var_{n}");
            if !self.graph.variables.iter().any(|v| v.name == name) {
                return name;
            }
            n += 1;
        }
    }

    fn rename_variable(&mut self, old: &str, new: &str) {
        if old == new || old.is_empty() {
            return;
        }
        for node in &mut self.graph.nodes {
            if node.kind != "VarGet" && node.kind != "VarSet" {
                continue;
            }
            if node.prop("name") == old {
                node.props.insert("name".to_string(), new.to_string());
            }
        }
    }

    // ---------------------------------------------------------- custom nodes

    pub fn custom_editor_window(&mut self, ctx: &egui::Context) {
        let Some(editor) = self.custom_editor.as_ref() else {
            return;
        };
        let original = editor.original.clone();
        let mut def = editor.def.clone();
        let mut error = editor.error.clone();
        let mut open = true;
        let mut close = false;
        let mut save = false;
        let mut delete = false;

        let title = if original.is_some() {
            self.tr("custom.edit")
        } else {
            self.tr("custom.new")
        };
        egui::Window::new(title)
            .open(&mut open)
            .collapsible(false)
            .resizable(true)
            .default_size(egui::vec2(560.0, 560.0))
            .show(ctx, |ui| {
                egui::Grid::new("custom_meta")
                    .num_columns(2)
                    .spacing([8.0, 4.0])
                    .show(ui, |ui| {
                        ui.label(self.tr("custom.kind"));
                        ui.add(egui::TextEdit::singleline(&mut def.kind).desired_width(220.0));
                        ui.end_row();
                        ui.label(self.tr("custom.title"));
                        ui.add(egui::TextEdit::singleline(&mut def.title).desired_width(300.0));
                        ui.end_row();
                        ui.label(self.tr("custom.category"));
                        ui.add(egui::TextEdit::singleline(&mut def.category).desired_width(220.0));
                        ui.end_row();
                        ui.label(self.tr("custom.description"));
                        ui.add(
                            egui::TextEdit::singleline(&mut def.description).desired_width(360.0),
                        );
                        ui.end_row();
                    });

                ui.add_space(6.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.label(RichText::new(self.tr("custom.inputs")).strong());
                        pin_rows(ui, &mut def.inputs, true);
                    });
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new(self.tr("custom.outputs")).strong());
                        pin_rows(ui, &mut def.outputs, false);
                    });
                });

                ui.add_space(6.0);
                ui.label(RichText::new(self.tr("custom.aarch64")).small().weak());
                ui.add(
                    egui::TextEdit::multiline(&mut def.asm_aarch64)
                        .code_editor()
                        .desired_rows(7)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(4.0);
                ui.label(RichText::new(self.tr("custom.x86_64")).small().weak());
                ui.add(
                    egui::TextEdit::multiline(&mut def.asm_x86_64)
                        .code_editor()
                        .desired_rows(7)
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                ui.label(RichText::new(self.tr("custom.contract")).small().weak());

                if !error.is_empty() {
                    ui.add_space(4.0);
                    ui.label(RichText::new(&error).color(Color32::from_rgb(235, 105, 105)));
                }

                ui.add_space(6.0);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button(self.tr("custom.save")).clicked() {
                        save = true;
                    }
                    if ui.button(self.tr("custom.cancel")).clicked() {
                        close = true;
                    }
                    if original.is_some() && ui.button(self.tr("custom.delete")).clicked() {
                        delete = true;
                    }
                });
            });

        if save {
            match self.save_custom_def(original.clone(), def.clone()) {
                Ok(()) => {
                    self.status = self.tr("status.custom_saved").replace("{}", &def.title);
                    close = true;
                }
                Err(e) => error = e,
            }
        }
        if delete {
            if let Some(kind) = original.as_deref() {
                match self.delete_custom_def(kind) {
                    Ok(()) => close = true,
                    Err(e) => error = e,
                }
            }
        }

        if close || !open {
            self.custom_editor = None;
        } else if let Some(editor) = self.custom_editor.as_mut() {
            editor.def = def;
            editor.error = error;
        }
    }
}

/// Renders one pin list inside the custom-node editor.
fn pin_rows(ui: &mut egui::Ui, pins: &mut Vec<CustomPin>, inputs: bool) {
    let mut remove = None;
    for (i, pin) in pins.iter_mut().enumerate() {
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut pin.name).desired_width(110.0));
            let mut ty = pin.ty.clone();
            egui::ComboBox::from_id_salt(format!("pin_type_{inputs}_{i}"))
                .selected_text(if ty == TY_EXEC { "exec" } else { "u64" })
                .width(72.0)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(ty == TY_EXEC, "exec").clicked() {
                        ty = TY_EXEC.to_string();
                    }
                    if ui.selectable_label(ty == TY_U64, "u64").clicked() {
                        ty = TY_U64.to_string();
                    }
                });
            if ty != pin.ty {
                if ty == TY_EXEC {
                    pin.name = "exec".to_string();
                } else if pin.name == "exec" {
                    pin.name = if inputs { "value" } else { "result" }.to_string();
                }
                pin.ty = ty;
            }
            if ui.button("×").clicked() {
                remove = Some(i);
            }
        });
    }
    if let Some(i) = remove {
        pins.remove(i);
    }
    if ui.button("+").clicked() {
        let mut n = 1;
        loop {
            let name = if inputs {
                format!("value_{n}")
            } else {
                format!("result_{n}")
            };
            if !pins.iter().any(|p| p.name == name) {
                pins.push(CustomPin::new(name, TY_U64));
                break;
            }
            n += 1;
        }
    }
}

/// Property keys referenced by `{prop:key}` placeholders in either template.
fn prop_keys(def: &CustomNodeDef) -> BTreeSet<String> {
    let mut keys = BTreeSet::new();
    for text in [&def.asm_aarch64, &def.asm_x86_64] {
        let mut rest = text.as_str();
        while let Some(start) = rest.find("{prop:") {
            rest = &rest[start + 6..];
            let Some(end) = rest.find('}') else { break };
            let key = rest[..end].trim();
            if !key.is_empty() {
                keys.insert(key.to_string());
            }
            rest = &rest[end + 1..];
        }
    }
    keys
}
