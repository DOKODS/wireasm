//! Application state, menus, panels, files, compilation and the run job.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError};
use std::time::{Duration, Instant};

use eframe::egui::{self, Align, Align2, Color32, Layout, RichText, Vec2};
use wireasm_core::json::Json;
use wireasm_core::model::{
    pins_for, CustomNodeDef, CustomPin, Edge, Graph, Node, Target, TY_EXEC, TY_U64,
};
use wireasm_core::{artifact_stem, compile, toolchain, CompileOutput, Severity};

use crate::circuit::CircuitDoc;
use crate::editor::{CodeEditor, Syntax, Workspace};
use crate::i18n::{t, Lang};
use crate::settings::Settings;
use wireasm_sim::schematic::Schematic;

pub const EXAMPLES: &[(&str, &str, &str)] = &[
    (
        "hello",
        "example.hello",
        include_str!("../../../examples/hello.json"),
    ),
    (
        "sum_loop",
        "example.sum_loop",
        include_str!("../../../examples/sum_loop.json"),
    ),
    (
        "fibonacci",
        "example.fibonacci",
        include_str!("../../../examples/fibonacci.json"),
    ),
    (
        "raw_asm",
        "example.raw_asm",
        include_str!("../../../examples/raw_asm.json"),
    ),
    (
        "custom_nodes",
        "example.custom_nodes",
        include_str!("../../../examples/custom_nodes.json"),
    ),
    (
        "avr_blink",
        "example.avr_blink",
        include_str!("../../../examples/avr_blink.json"),
    ),
    (
        "avr_uart",
        "example.avr_uart",
        include_str!("../../../examples/avr_uart_hello.json"),
    ),
    (
        "stm32_blink",
        "example.stm32_blink",
        include_str!("../../../examples/stm32_blink.json"),
    ),
    (
        "stm32_uart",
        "example.stm32_uart",
        include_str!("../../../examples/stm32_uart_hello.json"),
    ),
    (
        "esp32_blink",
        "example.esp32_blink",
        include_str!("../../../examples/esp32_blink.json"),
    ),
    (
        "esp32_uart",
        "example.esp32_uart",
        include_str!("../../../examples/esp32_uart_hello.json"),
    ),
    (
        "c_hello",
        "example.c_hello",
        include_str!("../../../examples/c_hello.json"),
    ),
    (
        "cpp_hello",
        "example.cpp_hello",
        include_str!("../../../examples/cpp_hello.json"),
    ),
    (
        "csharp_hello",
        "example.csharp_hello",
        include_str!("../../../examples/csharp_hello.json"),
    ),
];

#[derive(Clone, Copy, Debug)]
pub struct View {
    /// World coordinate shown at the top-left corner of the canvas.
    pub pan: Vec2,
    pub zoom: f32,
}

impl Default for View {
    fn default() -> Self {
        View {
            pan: Vec2::new(-60.0, -120.0),
            zoom: 1.0,
        }
    }
}

impl View {
    pub fn w2s(&self, rect: egui::Rect, p: egui::Pos2) -> egui::Pos2 {
        rect.min + (p.to_vec2() - self.pan) * self.zoom
    }

    pub fn s2w(&self, rect: egui::Rect, p: egui::Pos2) -> egui::Pos2 {
        ((p - rect.min) / self.zoom + self.pan).to_pos2()
    }

    pub fn zoom_at(&mut self, rect: egui::Rect, screen: egui::Pos2, factor: f32) {
        let before = self.s2w(rect, screen);
        self.zoom = (self.zoom * factor).clamp(0.3, 2.6);
        let after = self.s2w(rect, screen);
        self.pan += before - after;
    }
}

#[derive(Clone, Debug)]
pub struct PendingLink {
    pub from_node: String,
    pub from_pin: String,
    pub ty: String,
}

#[derive(Clone, Debug)]
pub struct DragNode {
    pub id: String,
    pub grab: Vec2,
    pub moved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BottomTab {
    Asm,
    Problems,
    Console,
}

pub struct Job {
    pub rx: Receiver<Result<String, String>>,
    pub started: Instant,
    pub kind: JobKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum JobKind {
    Run,
    Build,
    Flash,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum After {
    New,
    Open,
    Example(usize),
    Quit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Confirm {
    Unsaved(After),
    UnsavedCircuit(AfterCircuit),
}

/// What to do once the circuit is saved or discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AfterCircuit {
    New,
    Open,
    Example(usize),
    Quit,
}

pub struct CustomEditor {
    pub original: Option<String>,
    pub def: CustomNodeDef,
    pub error: String,
}

impl CustomEditor {
    pub fn new(def: Option<CustomNodeDef>) -> Self {
        match def {
            Some(def) => CustomEditor {
                original: Some(def.kind.clone()),
                def,
                error: String::new(),
            },
            None => CustomEditor {
                original: None,
                def: CustomNodeDef {
                    kind: "my_node".to_string(),
                    title: "My node".to_string(),
                    category: "Custom".to_string(),
                    description: String::new(),
                    inputs: vec![
                        CustomPin::new("exec", TY_EXEC),
                        CustomPin::new("value", TY_U64),
                    ],
                    outputs: vec![
                        CustomPin::new("exec", TY_EXEC),
                        CustomPin::new("result", TY_U64),
                    ],
                    asm_aarch64: "add {out}, {in0}, #1".to_string(),
                    asm_x86_64: "mov rax, rdi\nadd rax, 1".to_string(),
                },
                error: String::new(),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum Action {
    New,
    Open,
    Save,
    SaveAs,
    ExportAsm,
    ImportLibrary,
    ExportLibrary,
    ImportAsm,
    Example(usize),
    Quit,
    Undo,
    Redo,
    Delete,
    Fit,
    ZoomIn,
    ZoomOut,
    Compile,
    BuildRun,
    BuildFirmware,
    FlashFirmware,
    BoardSettings,
    Variables,
    NewCustom,
    Shortcuts,
    About,
    SetLang(Option<Lang>),
    SetTarget(Target),
    CodeOpen,
    CodeNew,
    CircuitNew,
    CircuitOpen,
    CircuitSave,
    CircuitSaveAs,
    CircuitExample(usize),
    CircuitRun,
    CircuitFit,
    CircuitExport,
    LibraryExport,
    LibraryImport,
}

pub struct WireStudio {
    pub settings: Settings,
    pub lang: Lang,
    pub graph: Graph,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub needs_compile: bool,
    pub out: CompileOutput,
    pub selected: Option<String>,
    pub view: View,
    pub pending_link: Option<PendingLink>,
    pub drag_node: Option<DragNode>,
    pub panning: bool,
    pub palette_drag: Option<String>,
    pub palette_filter: String,
    pub new_prop_key: String,
    pub tab: BottomTab,
    pub console: Vec<String>,
    pub variables_open: bool,
    pub custom_editor: Option<CustomEditor>,
    pub confirm: Option<Confirm>,
    pub show_about: bool,
    pub show_shortcuts: bool,
    pub board_open: bool,
    pub avr_ports: Vec<String>,
    pub workspace: Workspace,
    pub editor: CodeEditor,
    pub circuit: CircuitDoc,
    pub allow_close: bool,
    pub job: Option<Job>,
    pub undo: Vec<String>,
    pub redo: Vec<String>,
    pub status: String,
    pub fit_requested: bool,
    pub last_canvas: egui::Rect,
    pub window_title: String,
}

impl WireStudio {
    pub fn new(initial: Option<PathBuf>) -> Self {
        let settings = Settings::load();
        let lang = settings
            .lang
            .as_deref()
            .and_then(Lang::from_code)
            .unwrap_or_else(Lang::detect);
        let graph = default_graph();
        let out = compile(&graph);
        let mut app = WireStudio {
            settings,
            lang,
            graph,
            path: None,
            dirty: false,
            needs_compile: false,
            out,
            selected: None,
            view: View::default(),
            pending_link: None,
            drag_node: None,
            panning: false,
            palette_drag: None,
            palette_filter: String::new(),
            new_prop_key: String::new(),
            tab: BottomTab::Asm,
            console: vec![t(lang, "status.console_ready").to_string()],
            variables_open: false,
            custom_editor: None,
            confirm: None,
            show_about: false,
            show_shortcuts: false,
            board_open: false,
            avr_ports: Vec::new(),
            workspace: Workspace::Graph,
            editor: CodeEditor::new(),
            circuit: CircuitDoc::default(),
            allow_close: false,
            job: None,
            undo: Vec::new(),
            redo: Vec::new(),
            status: String::new(),
            fit_requested: true,
            last_canvas: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            window_title: String::new(),
        };
        crate::editor::set_code_font_size(app.code_font_size() as f32);
        app.status = app.tr("status.ready").to_string();
        if let Some(path) = initial {
            app.open_path(&path);
        }
        // Headless check: `WIRESTUDIO_AUTOSIM=1 wirestudio circuit.json` runs
        // the analysis at startup, which is handy in scripts and CI.
        if std::env::var("WIRESTUDIO_AUTOSIM").is_ok() {
            app.run_circuit();
        }
        app
    }

    pub fn tr(&self, key: &str) -> &'static str {
        t(self.lang, key)
    }

    /// Push the theme and the zoom of the interface into the context.
    fn apply_appearance(&mut self, ctx: &egui::Context) {
        let scale = self.ui_scale() as f32;
        if (ctx.zoom_factor() - scale).abs() > 0.001 {
            ctx.set_zoom_factor(scale);
        }
        let theme = self.theme();
        let wanted = match theme {
            crate::settings::Theme::Dark => egui::Theme::Dark,
            crate::settings::Theme::Light => egui::Theme::Light,
        };
        if ctx.theme() != wanted {
            ctx.set_theme(wanted);
            // The visuals go with the theme, so that the panels and the
            // background follow it too.
            ctx.set_visuals(match wanted {
                egui::Theme::Dark => egui::Visuals::dark(),
                egui::Theme::Light => egui::Visuals::light(),
            });
        }
    }

    // ------------------------------------------------------ parameters

    /// The theme of the interface.
    pub fn theme(&self) -> crate::settings::Theme {
        self.settings
            .theme
            .as_deref()
            .and_then(crate::settings::Theme::from_key)
            .unwrap_or(crate::settings::Theme::Dark)
    }

    pub fn set_theme(&mut self, theme: crate::settings::Theme) {
        self.settings.theme = Some(theme.key().to_string());
        self.settings.save();
    }

    /// The look of the schematic sheet.
    pub fn sheet_style(&self) -> crate::settings::SheetStyle {
        self.settings
            .sheet_style
            .as_deref()
            .and_then(crate::settings::SheetStyle::from_key)
            .unwrap_or(crate::settings::SheetStyle::Multisim)
    }

    pub fn set_sheet_style(&mut self, style: crate::settings::SheetStyle) {
        self.settings.sheet_style = Some(style.key().to_string());
        self.settings.save();
    }

    /// The zoom of the whole interface.
    pub fn ui_scale(&self) -> f64 {
        self.settings.ui_scale.unwrap_or(1.0).clamp(0.75, 1.75)
    }

    pub fn set_ui_scale(&mut self, scale: f64) {
        self.settings.ui_scale = Some(scale.clamp(0.75, 1.75));
        self.settings.save();
    }

    /// The size of the code font, in points.
    pub fn code_font_size(&self) -> f64 {
        self.settings.code_font_size.unwrap_or(14.0).clamp(10.0, 22.0)
    }

    pub fn set_code_font_size(&mut self, size: f64) {
        self.settings.code_font_size = Some(size.clamp(10.0, 22.0));
        crate::editor::set_code_font_size(size.clamp(10.0, 22.0) as f32);
        self.settings.save();
    }

    /// Whether the sheet shows the grid.
    pub fn show_grid(&self) -> bool {
        self.settings.show_grid.unwrap_or(true)
    }

    pub fn set_show_grid(&mut self, show: bool) {
        self.settings.show_grid = Some(show);
        self.settings.save();
    }

    /// Whether the sheet shows the node numbers.
    pub fn show_node_numbers(&self) -> bool {
        self.settings.show_node_numbers.unwrap_or(true)
    }

    pub fn set_show_node_numbers(&mut self, show: bool) {
        self.settings.show_node_numbers = Some(show);
        self.settings.save();
    }

    /// Whether the instrument panels open after a run.
    pub fn auto_open_panels(&self) -> bool {
        self.settings.auto_open_panels.unwrap_or(true)
    }

    pub fn set_auto_open_panels(&mut self, open: bool) {
        self.settings.auto_open_panels = Some(open);
        self.settings.save();
    }

    /// The step of the transient analysis, in seconds.
    pub fn sim_dt(&self) -> f64 {
        self.settings.sim_dt.unwrap_or(1e-4).clamp(1e-6, 1e-2)
    }

    pub fn set_sim_dt(&mut self, dt: f64) {
        self.settings.sim_dt = Some(dt.clamp(1e-6, 1e-2));
        self.circuit.model.opts.dt = self.sim_dt();
        self.settings.save();
    }

    /// The length of the transient analysis, in seconds.
    pub fn sim_stop(&self) -> f64 {
        self.settings.sim_stop.unwrap_or(2e-3).clamp(1e-4, 1.0)
    }

    pub fn set_sim_stop(&mut self, stop: f64) {
        self.settings.sim_stop = Some(stop.clamp(1e-4, 1.0));
        self.circuit.model.opts.t_stop = self.sim_stop();
        self.settings.save();
    }

    /// Put every parameter back to its default.
    pub fn reset_settings(&mut self) {
        self.settings.theme = None;
        self.settings.sheet_style = None;
        self.settings.ui_scale = None;
        self.settings.code_font_size = None;
        self.settings.line_numbers = None;
        self.settings.show_grid = None;
        self.settings.show_node_numbers = None;
        self.settings.auto_open_panels = None;
        self.settings.sim_dt = None;
        self.settings.sim_stop = None;
        self.settings.save();
        self.editor.line_numbers = true;
        self.circuit.model.opts.dt = 1e-4;
        self.circuit.model.opts.t_stop = 2e-3;
        self.status = self.tr("settings.reset_done").to_string();
    }

    pub fn node_title(&self, kind: &str) -> String {
        if let Some(def) = self.graph.custom(kind) {
            return if def.title.is_empty() {
                def.kind.clone()
            } else {
                def.title.clone()
            };
        }
        match node_key(kind) {
            Some(key) => self.tr(key).to_string(),
            None => kind.to_string(),
        }
    }

    // ---------------------------------------------------------------- state

    pub fn touch(&mut self) {
        self.dirty = true;
        self.needs_compile = true;
    }

    pub fn snapshot(&mut self) {
        self.undo
            .push(self.graph.to_json_value().stringify_pretty());
        if self.undo.len() > 200 {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    pub fn compile_now(&mut self, announce: bool) {
        self.out = compile(&self.graph);
        self.needs_compile = false;
        if announce {
            if self.out.ok {
                self.status = self.tr("status.compiled").replace("{}", &self.graph.target);
                if let Some(path) = self.path.clone() {
                    let stem = artifact_stem(&self.graph);
                    let asm = path
                        .parent()
                        .unwrap_or_else(|| Path::new("."))
                        .join(format!("{stem}.s"));
                    if std::fs::write(&asm, &self.out.asm).is_ok() {
                        self.status = self
                            .tr("status.auto_saved")
                            .replace("{}", &asm.display().to_string());
                    }
                }
            } else {
                let errors = self
                    .out
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == Severity::Error)
                    .count();
                self.status = self
                    .tr("status.compile_failed")
                    .replace("{}", &errors.to_string());
                self.tab = BottomTab::Problems;
            }
        }
        if self.out.ok {
            self.refresh_generated_tab();
        }
    }

    pub(crate) fn error(&mut self, msg: impl Into<String>) {
        let msg = msg.into();
        self.console.push(msg.clone());
        self.status = msg;
    }

    // ------------------------------------------------------------- file i/o

    fn dialog_dir(&self) -> Option<PathBuf> {
        self.settings
            .last_dir
            .as_ref()
            .map(PathBuf::from)
            .or_else(|| {
                self.path
                    .as_ref()
                    .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            })
    }

    pub(crate) fn remember_dir(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            self.settings.last_dir = Some(parent.display().to_string());
            self.settings.save();
        }
    }

    fn default_file_name(&self, ext: &str) -> String {
        format!("{}.{}", artifact_stem(&self.graph), ext)
    }

    pub(crate) fn pick_open(
        &mut self,
        title: &str,
        filters: &[(&str, &[&str])],
    ) -> Option<PathBuf> {
        let mut dlg = rfd::FileDialog::new().set_title(title);
        if let Some(dir) = self.dialog_dir() {
            dlg = dlg.set_directory(dir);
        }
        for (name, exts) in filters {
            dlg = dlg.add_filter(*name, exts);
        }
        dlg.pick_file()
    }

    pub(crate) fn pick_save(
        &mut self,
        title: &str,
        filters: &[(&str, &[&str])],
        file_name: &str,
    ) -> Option<PathBuf> {
        let mut dlg = rfd::FileDialog::new()
            .set_title(title)
            .set_file_name(file_name);
        let dir = self
            .settings
            .out_dir
            .clone()
            .map(PathBuf::from)
            .or_else(|| self.dialog_dir());
        if let Some(dir) = dir {
            dlg = dlg.set_directory(dir);
        }
        for (name, exts) in filters {
            dlg = dlg.add_filter(*name, exts);
        }
        dlg.save_file()
    }

    pub fn save(&mut self, save_as: bool) -> bool {
        let mut path = self.path.clone();
        if save_as || path.is_none() {
            let name = self.default_file_name("json");
            let filter = self.tr("dialog.graph");
            match self.pick_save(self.tr("file.save_as"), &[(filter, &["json"])], &name) {
                Some(p) => path = Some(with_extension(p, "json")),
                None => {
                    self.status = self.tr("status.cancelled").to_string();
                    return false;
                }
            }
        }
        let Some(path) = path else { return false };
        let text = self.graph.to_json_value().stringify_pretty();
        if let Err(e) = std::fs::write(&path, text) {
            self.error(format!("{}: {e}", path.display()));
            return false;
        }
        self.remember_dir(&path);
        self.path = Some(path.clone());
        self.dirty = false;
        self.status = self
            .tr("status.saved")
            .replace("{}", &path.display().to_string());
        self.console.push(self.status.clone());
        true
    }

    pub fn open_path(&mut self, path: &Path) {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                self.error(format!("{}: {e}", path.display()));
                return;
            }
        };
        // A schematic file has its own marker; everything else is a graph.
        if let Ok(value) = Json::parse(&src) {
            if value.get("kind").and_then(|x| x.as_str()) == Some("wireasm-circuit") {
                self.circuit_open_path(path);
                return;
            }
        }
        // A source file belongs to the code editor, not to the graph.
        if is_source_file(path) {
            match self.editor.open(path) {
                Ok(()) => {
                    self.workspace = Workspace::Code;
                    self.status = self
                        .tr("status.code_opened")
                        .replace("{}", &path.display().to_string());
                }
                Err(e) => self.error(e),
            }
            return;
        }
        let parsed = Json::parse(&src)
            .map_err(|e| format!("JSON: {e}"))
            .and_then(|v| Graph::from_json_value(&v));
        match parsed {
            Ok(graph) => {
                self.graph = graph;
                self.path = Some(path.to_path_buf());
                self.dirty = false;
                self.selected = None;
                self.pending_link = None;
                self.drag_node = None;
                self.undo.clear();
                self.redo.clear();
                self.needs_compile = true;
                self.fit_requested = true;
                self.remember_dir(path);
                self.compile_now(false);
                self.tab = if self.out.ok {
                    BottomTab::Asm
                } else {
                    BottomTab::Problems
                };
                self.status = self
                    .tr("status.opened")
                    .replace("{}", &path.display().to_string());
                self.console.push(self.status.clone());
            }
            Err(e) => self.error(format!("{}: {e}", path.display())),
        }
    }

    fn open_dialog(&mut self) {
        let title = self.tr("file.open").to_string();
        let filter = self.tr("dialog.graph");
        if let Some(path) = self.pick_open(&title, &[(filter, &["json"])]) {
            self.open_path(&path);
        }
    }

    /// Open a schematic file into the circuit workspace.
    pub fn circuit_open_path(&mut self, path: &Path) {
        let src = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                self.error(format!("{}: {e}", path.display()));
                return;
            }
        };
        let parsed = Json::parse(&src)
            .map_err(|e| format!("JSON: {e}"))
            .and_then(|v| Schematic::from_json_value(&v));
        match parsed {
            Ok(schematic) => {
                self.circuit.model = schematic;
                self.circuit.path = Some(path.to_path_buf());
                self.circuit.dirty = false;
                self.circuit.result = None;
                self.workspace = Workspace::Circuit;
                self.remember_dir(path);
                self.status = self
                    .tr("status.circuit_opened")
                    .replace("{}", &path.display().to_string());
                self.console.push(self.status.clone());
            }
            Err(e) => self.error(format!("{}: {e}", path.display())),
        }
    }

    /// Write the schematic to a file.
    pub fn circuit_save(&mut self, save_as: bool) -> bool {
        let mut path = self.circuit.path.clone();
        if save_as || path.is_none() {
            let name = format!("{}.circuit.json", self.circuit.model.name);
            let filter = self.tr("dialog.circuit");
            match self.pick_save(self.tr("circuit.save_as"), &[(filter, &["json"])], &name) {
                Some(p) => path = Some(p),
                None => {
                    self.status = self.tr("status.cancelled").to_string();
                    return false;
                }
            }
        }
        let Some(path) = path else { return false };
        let text = self.circuit.model.to_json_value().stringify_pretty();
        if let Err(e) = std::fs::write(&path, text) {
            self.error(format!("{}: {e}", path.display()));
            return false;
        }
        self.remember_dir(&path);
        self.circuit.path = Some(path.clone());
        self.circuit.dirty = false;
        self.status = self
            .tr("status.circuit_saved")
            .replace("{}", &path.display().to_string());
        self.console.push(self.status.clone());
        true
    }

    /// Ask before throwing away an unsaved schematic.
    pub fn circuit_new(&mut self) {
        if self.circuit.dirty {
            self.confirm = Some(Confirm::UnsavedCircuit(AfterCircuit::New));
        } else {
            self.circuit.model = Schematic::empty();
            self.circuit.result = None;
            self.circuit.dirty = false;
            self.workspace = Workspace::Circuit;
            self.status = self.tr("status.circuit_new").to_string();
        }
    }

    /// Show the open dialog for schematic files.
    pub fn circuit_open(&mut self) {
        let title = self.tr("circuit.open").to_string();
        let filter = self.tr("dialog.circuit");
        if let Some(path) = self.pick_open(&title, &[(filter, &["json"])]) {
            self.circuit_open_path(&path);
        }
    }

    /// Ask before replacing a schematic that has unsaved changes.
    pub fn circuit_ask_or(&mut self, after: AfterCircuit) {
        if self.circuit.dirty {
            self.confirm = Some(Confirm::UnsavedCircuit(after));
        } else {
            self.after_circuit_unsaved(after, None);
        }
    }

    /// Load one of the built-in example schematics.
    pub fn circuit_load_example(&mut self, idx: usize) {
        let examples = Schematic::examples();
        let Some((_, schematic)) = examples.get(idx) else {
            return;
        };
        let name = schematic.name.clone();
        self.circuit.model = schematic.clone();
        self.circuit.path = None;
        self.circuit.dirty = false;
        self.circuit.result = None;
        self.workspace = Workspace::Circuit;
        self.status = self.tr("status.circuit_example").replace("{}", &name);
    }

    /// Export the current schematic as a simulation netlist.
    pub fn circuit_export_netlist(&mut self) {
        let circuit = match self.circuit.model.netlist() {
            Ok(c) => c,
            Err(e) => {
                self.error(e);
                return;
            }
        };
        let filter = self.tr("dialog.netlist");
        let name = format!("{}.netlist.json", self.circuit.model.name);
        let Some(path) = self.pick_save(
            self.tr("circuit.export_netlist"),
            &[(filter, &["json"])],
            &name,
        ) else {
            self.status = self.tr("status.cancelled").to_string();
            return;
        };
        let text = circuit.to_json_value().stringify_pretty();
        if let Err(e) = std::fs::write(&path, text) {
            self.error(format!("{}: {e}", path.display()));
            return;
        }
        self.remember_dir(&path);
        self.status = self
            .tr("status.circuit_exported")
            .replace("{}", &path.display().to_string());
    }

    /// Write the component library to a file.
    pub fn library_export(&mut self) {
        if self.circuit.library.entries.is_empty() {
            self.status = self.tr("lib.empty").to_string();
            return;
        }
        let filter = self.tr("dialog.component_library");
        let name = format!("{}.wirelib.json", self.circuit.library.name);
        let Some(path) = self.pick_save(self.tr("lib.export"), &[(filter, &["json"])], &name)
        else {
            self.status = self.tr("status.cancelled").to_string();
            return;
        };
        let text = self.circuit.library.to_json_value().stringify_pretty();
        if let Err(e) = std::fs::write(&path, text) {
            self.error(format!("{}: {e}", path.display()));
            return;
        }
        self.remember_dir(&path);
        self.status = self
            .tr("lib.exported")
            .replace("{}", &path.display().to_string());
    }

    /// Read a component library and add its parts to the palette.
    pub fn library_import(&mut self) {
        let filter = self.tr("dialog.component_library");
        let Some(path) = self.pick_open(self.tr("lib.import"), &[(filter, &["json"])]) else {
            self.status = self.tr("status.cancelled").to_string();
            return;
        };
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                self.error(format!("{}: {e}", path.display()));
                return;
            }
        };
        let parsed = Json::parse(&src)
            .map_err(|e| format!("JSON: {e}"))
            .and_then(|v| wireasm_sim::Library::from_json_value(&v));
        match parsed {
            Ok(lib) => {
                let count = lib.entries.len();
                for entry in lib.entries {
                    if self.circuit.library.find(&entry.key).is_some() {
                        continue;
                    }
                    self.circuit.library.entries.push(entry);
                }
                self.status = self
                    .tr("lib.imported")
                    .replace("{}", &count.to_string())
                    .replace("{}", &path.display().to_string());
                self.remember_dir(&path);
            }
            Err(e) => self.error(self.tr("lib.import_failed").replace("{}", &e)),
        }
    }

    pub fn load_example(&mut self, idx: usize) {
        let Some((_, _, src)) = EXAMPLES.get(idx) else {
            return;
        };
        let parsed = Json::parse(src).and_then(|v| Graph::from_json_value(&v));
        match parsed {
            Ok(mut graph) => {
                // Host assembly examples follow the machine; MCU and source
                // examples keep the target they were written for.
                if Target::parse(&graph.target)
                    .map(|t| t.is_host_asm())
                    .unwrap_or(true)
                {
                    if let Some(host) = toolchain::host_target() {
                        graph.target = host.as_str().to_string();
                    }
                }
                self.graph = graph;
                self.path = None;
                self.dirty = true;
                self.selected = None;
                self.pending_link = None;
                self.needs_compile = true;
                self.fit_requested = true;
                self.undo.clear();
                self.redo.clear();
                self.status = self.tr("status.new").to_string();
            }
            Err(e) => self.error(format!("{}: {e}", self.tr("file.examples"))),
        }
    }

    fn export_asm(&mut self) {
        self.compile_now(false);
        if !self.out.ok {
            self.status = self
                .tr("status.compile_failed")
                .replace("{}", &self.out.diagnostics.len().to_string());
            self.tab = BottomTab::Problems;
            return;
        }
        let name = self.default_file_name("s");
        let filter = self.tr("dialog.asm");
        if let Some(path) = self.pick_save(
            self.tr("file.export_asm"),
            &[(filter, &["s", "asm"])],
            &name,
        ) {
            let path = with_extension(path, "s");
            match std::fs::write(&path, &self.out.asm) {
                Ok(()) => {
                    self.settings.out_dir = path.parent().map(|p| p.display().to_string());
                    self.settings.save();
                    self.remember_dir(&path);
                    self.status = self
                        .tr("status.saved_asm")
                        .replace("{}", &path.display().to_string());
                    self.console.push(self.status.clone());
                }
                Err(e) => self.error(format!("{}: {e}", path.display())),
            }
        }
    }

    fn import_library(&mut self) {
        let title = self.tr("file.import_library").to_string();
        let filter = self.tr("dialog.library");
        let Some(path) = self.pick_open(&title, &[(filter, &["json"])]) else {
            return;
        };
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                self.error(format!("{}: {e}", path.display()));
                return;
            }
        };
        match wireasm_core::model::Library::parse(&src) {
            Ok(lib) => {
                self.snapshot();
                let count = self.graph.merge_library(&lib);
                self.touch();
                self.status = self
                    .tr("status.imported_lib")
                    .replacen("{}", &count.to_string(), 1)
                    .replacen("{}", &path.display().to_string(), 1);
                self.console.push(self.status.clone());
            }
            Err(e) => self.error(format!("{}: {e}", path.display())),
        }
    }

    fn export_library(&mut self) {
        if self.graph.custom_nodes.is_empty() {
            self.status = self.tr("status.no_custom").to_string();
            return;
        }
        let name = format!("{}.wirelib.json", artifact_stem(&self.graph));
        let filter = self.tr("dialog.library");
        if let Some(path) = self.pick_save(
            self.tr("file.export_library"),
            &[(filter, &["json"])],
            &name,
        ) {
            let path = if path.extension().is_none() {
                path.with_extension("wirelib.json")
            } else {
                path
            };
            let lib = self.graph.as_library();
            match std::fs::write(&path, lib.to_json_value().stringify_pretty()) {
                Ok(()) => {
                    self.status = self
                        .tr("status.exported_lib")
                        .replace("{}", &path.display().to_string());
                    self.console.push(self.status.clone());
                }
                Err(e) => self.error(format!("{}: {e}", path.display())),
            }
        }
    }

    fn import_asm(&mut self) {
        let title = self.tr("file.import_asm").to_string();
        let filter = self.tr("dialog.asm_source");
        let Some(path) = self.pick_open(&title, &[(filter, &["s", "asm", "S", "txt"])]) else {
            return;
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                self.error(format!("{}: {e}", path.display()));
                return;
            }
        };
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "imported".to_string());
        let kind = format!("import_{}", sanitize_kind(&stem));
        let def = CustomNodeDef {
            kind: kind.clone(),
            title: stem.clone(),
            category: "Imported".to_string(),
            description: format!("Imported from {}", path.display()),
            inputs: vec![
                CustomPin::new("exec", TY_EXEC),
                CustomPin::new("value", TY_U64),
            ],
            outputs: vec![
                CustomPin::new("exec", TY_EXEC),
                CustomPin::new("result", TY_U64),
            ],
            asm_aarch64: text.clone(),
            asm_x86_64: text,
        };
        self.custom_editor = Some(CustomEditor::new(Some(def)));
        self.status = self.tr("status.imported_asm").replace("{}", &stem);
        self.console.push(self.status.clone());
    }

    pub fn save_custom_def(
        &mut self,
        original: Option<String>,
        def: CustomNodeDef,
    ) -> Result<(), String> {
        if def.kind.trim().is_empty() || !valid_kind(&def.kind) {
            return Err(self.tr("custom.error_kind").to_string());
        }
        if pins_for(&def.kind).is_some() {
            return Err(self.tr("custom.error_builtin").to_string());
        }
        if original.as_deref() != Some(def.kind.as_str()) && self.graph.custom(&def.kind).is_some()
        {
            return Err(self.tr("custom.error_exists").to_string());
        }
        if def.asm_aarch64.trim().is_empty() || def.asm_x86_64.trim().is_empty() {
            return Err(self.tr("custom.error_asm").to_string());
        }
        if !pins_ok(&def.inputs) || !pins_ok(&def.outputs) {
            return Err(self.tr("custom.error_pin").to_string());
        }
        let u64_inputs = def.inputs.iter().filter(|p| p.ty == TY_U64).count();
        let u64_outputs = def.outputs.iter().filter(|p| p.ty == TY_U64).count();
        let exec_inputs = def.inputs.iter().filter(|p| p.ty == TY_EXEC).count();
        let exec_outputs = def.outputs.iter().filter(|p| p.ty == TY_EXEC).count();
        if u64_inputs > 8 || u64_outputs > 1 || (exec_outputs > 0 && exec_inputs == 0) {
            return Err(self.tr("custom.error_pin").to_string());
        }
        self.snapshot();
        if let Some(old) = original {
            if old != def.kind {
                self.graph.custom_nodes.retain(|d| d.kind != old);
                for node in &mut self.graph.nodes {
                    if node.kind == old {
                        node.kind = def.kind.clone();
                    }
                }
            } else {
                self.graph.custom_nodes.retain(|d| d.kind != old);
            }
        } else {
            self.graph.custom_nodes.retain(|d| d.kind != def.kind);
        }
        self.graph.custom_nodes.push(def);
        self.touch();
        Ok(())
    }

    pub fn delete_custom_def(&mut self, kind: &str) -> Result<(), String> {
        if self.graph.nodes.iter().any(|n| n.kind == kind) {
            return Err(self.tr("custom.error_used").to_string());
        }
        self.snapshot();
        self.graph.custom_nodes.retain(|d| d.kind != kind);
        self.touch();
        Ok(())
    }

    // -------------------------------------------------------------- actions

    fn ask_or(&mut self, after: After) {
        if self.dirty {
            self.confirm = Some(Confirm::Unsaved(after));
        } else {
            self.proceed(after);
        }
    }

    fn proceed(&mut self, after: After) {
        match after {
            After::New => {
                self.graph = default_graph();
                self.path = None;
                self.dirty = true;
                self.selected = None;
                self.pending_link = None;
                self.undo.clear();
                self.redo.clear();
                self.needs_compile = true;
                self.fit_requested = true;
                self.status = self.tr("status.new").to_string();
            }
            After::Open => self.open_dialog(),
            After::Example(idx) => self.load_example(idx),
            After::Quit => {}
        }
    }

    pub fn act(&mut self, action: Action, ctx: &egui::Context) {
        match action {
            Action::New => self.ask_or(After::New),
            Action::Open => self.ask_or(After::Open),
            Action::Save => match self.workspace {
                Workspace::Code => {
                    self.code_save(false);
                }
                Workspace::Circuit => {
                    self.circuit_save(false);
                }
                Workspace::Graph => {
                    self.save(false);
                }
                Workspace::Settings => {}
            },
            Action::SaveAs => match self.workspace {
                Workspace::Code => {
                    self.code_save(true);
                }
                Workspace::Circuit => {
                    self.circuit_save(true);
                }
                Workspace::Graph => {
                    self.save(true);
                }
                Workspace::Settings => {}
            },
            Action::CodeOpen => {
                self.workspace = Workspace::Code;
                self.code_open();
            }
            Action::CodeNew => {
                let target = Target::parse(&self.graph.target).unwrap_or(Target::Aarch64Macos);
                self.code_new(Syntax::from_target(target));
            }
            Action::CircuitNew => self.circuit_new(),
            Action::CircuitOpen => self.circuit_ask_or(AfterCircuit::Open),
            Action::CircuitSave => {
                self.circuit_save(false);
            }
            Action::CircuitSaveAs => {
                self.circuit_save(true);
            }
            Action::CircuitExample(idx) => self.circuit_ask_or(AfterCircuit::Example(idx)),
            Action::CircuitRun => self.run_circuit(),
            Action::CircuitFit => self.circuit.fit_requested = true,
            Action::CircuitExport => self.circuit_export_netlist(),
            Action::LibraryExport => self.library_export(),
            Action::LibraryImport => self.library_import(),
            Action::ExportAsm => self.export_asm(),
            Action::ImportLibrary => self.import_library(),
            Action::ExportLibrary => self.export_library(),
            Action::ImportAsm => self.import_asm(),
            Action::Example(idx) => self.ask_or(After::Example(idx)),
            Action::Quit => {
                if self.dirty {
                    self.confirm = Some(Confirm::Unsaved(After::Quit));
                } else {
                    self.allow_close = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            Action::Undo => self.undo_action(),
            Action::Redo => self.redo_action(),
            Action::Delete => {
                if self.workspace == Workspace::Circuit {
                    self.circuit_delete_selected();
                } else {
                    self.delete_selected();
                }
            }
            Action::Fit => self.fit_requested = true,
            Action::ZoomIn => {
                let c = self.last_canvas.center();
                self.view.zoom_at(self.last_canvas, c, 1.2);
            }
            Action::ZoomOut => {
                let c = self.last_canvas.center();
                self.view.zoom_at(self.last_canvas, c, 1.0 / 1.2);
            }
            Action::Compile => self.compile_now(true),
            Action::BuildRun => self.build_run(ctx),
            Action::BuildFirmware => self.firmware_job(ctx, false),
            Action::FlashFirmware => self.firmware_job(ctx, true),
            Action::BoardSettings => self.open_board_settings(),
            Action::Variables => self.variables_open = true,
            Action::NewCustom => self.custom_editor = Some(CustomEditor::new(None)),
            Action::Shortcuts => self.show_shortcuts = true,
            Action::About => self.show_about = true,
            Action::SetLang(lang) => {
                self.lang = lang.unwrap_or_else(Lang::detect);
                self.settings.lang = lang.map(|l| l.code().to_string());
                self.settings.save();
            }
            Action::SetTarget(target) => {
                self.graph.target = target.as_str().to_string();
                self.touch();
                self.status = self
                    .tr("status.target_changed")
                    .replace("{}", target.as_str());
            }
        }
    }

    fn undo_action(&mut self) {
        let Some(prev) = self.undo.pop() else { return };
        let cur = self.graph.to_json_value().stringify_pretty();
        self.redo.push(cur);
        if let Ok(v) = Json::parse(&prev) {
            if let Ok(g) = Graph::from_json_value(&v) {
                self.graph = g;
                self.dirty = true;
                self.needs_compile = true;
                self.selected = self
                    .selected
                    .clone()
                    .filter(|id| self.graph.node(id).is_some());
                self.pending_link = None;
                self.drag_node = None;
            }
        }
    }

    fn redo_action(&mut self) {
        let Some(next) = self.redo.pop() else { return };
        let cur = self.graph.to_json_value().stringify_pretty();
        self.undo.push(cur);
        if let Ok(v) = Json::parse(&next) {
            if let Ok(g) = Graph::from_json_value(&v) {
                self.graph = g;
                self.dirty = true;
                self.needs_compile = true;
                self.selected = self
                    .selected
                    .clone()
                    .filter(|id| self.graph.node(id).is_some());
                self.pending_link = None;
                self.drag_node = None;
            }
        }
    }

    pub fn delete_selected(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.snapshot();
        self.graph
            .edges
            .retain(|e| e.from_node != id && e.to_node != id);
        self.graph.nodes.retain(|n| n.id != id);
        self.selected = None;
        self.pending_link = None;
        self.touch();
        self.status = self.tr("status.deleted").replace("{}", &id);
    }

    // ------------------------------------------------------------ run a job

    fn build_run(&mut self, ctx: &egui::Context) {
        self.compile_now(false);
        if !self.out.ok {
            self.status = self
                .tr("status.compile_failed")
                .replace("{}", &self.out.diagnostics.len().to_string());
            self.tab = BottomTab::Problems;
            return;
        }
        let Some(target) = Target::parse(&self.graph.target) else {
            self.error(self.tr("status.unknown_target"));
            return;
        };
        if target.is_mcu() {
            self.firmware_job(ctx, false);
            return;
        }
        if !toolchain::can_build(target) {
            self.status = self.tr("status.host_only").replace("{}", target.as_str());
            return;
        }
        let source = self.out.asm.clone();
        let stem = artifact_stem(&self.graph);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let dir = std::env::temp_dir().join("wirestudio");
                std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
                let (program, label) = if target.is_source() {
                    let build = toolchain::build_source(&source, &dir, &stem, target)?;
                    (build.exe, build.source)
                } else {
                    let asm_path = dir.join(format!("{stem}.s"));
                    let exe_path = dir.join(format!("{stem}-{}", std::process::id()));
                    std::fs::write(&asm_path, source).map_err(|e| format!("write asm: {e}"))?;
                    toolchain::assemble(&asm_path, &exe_path, target)?;
                    (exe_path, asm_path)
                };
                let run = toolchain::run(&program, 5)?;
                let mut log = format!("$ {}:{stem} ({})\n", label.display(), target.as_str());
                log.push_str(&run.stdout);
                log.push_str(&run.stderr);
                if run.timed_out {
                    log.push_str("[killed after 5 s]\n");
                }
                log.push_str(&format!(
                    "[exit {}]\n",
                    run.status
                        .map(|c| c.to_string())
                        .unwrap_or_else(|| "signal".to_string())
                ));
                Ok(log)
            })();
            let _ = tx.send(result);
        });
        self.job = Some(Job {
            rx,
            started: Instant::now(),
            kind: JobKind::Run,
        });
        self.status = self.tr("status.running").to_string();
        self.tab = BottomTab::Console;
        ctx.request_repaint();
    }

    // ------------------------------------------------------- build firmware

    fn open_board_settings(&mut self) {
        self.avr_ports = toolchain::avr_ports();
        if self.settings.avr_port.is_none() {
            self.settings.avr_port = self.avr_ports.first().cloned();
        }
        if self.settings.avr_board.is_none() {
            self.settings.avr_board = Some("uno".to_string());
        }
        if self.settings.stm32_transport.is_none() {
            self.settings.stm32_transport = Some("stlink".to_string());
        }
        self.board_open = true;
    }

    pub(crate) fn firmware_dir(&self) -> PathBuf {
        if let Some(dir) = &self.settings.out_dir {
            return PathBuf::from(dir);
        }
        self.path
            .as_ref()
            .and_then(|p| p.parent().map(|p| p.join("out")))
            .unwrap_or_else(|| PathBuf::from("out"))
    }

    /// Compile the graph, then build firmware for the current target: AVR
    /// (`.elf/.hex/.bin` via avr-gcc), STM32 (via arm-none-eabi-gcc) or ESP
    /// (an Arduino sketch via arduino-cli). With `flash`, upload it as well.
    fn firmware_job(&mut self, ctx: &egui::Context, flash: bool) {
        self.compile_now(false);
        if !self.out.ok {
            self.status = self
                .tr("status.compile_failed")
                .replace("{}", &self.out.diagnostics.len().to_string());
            self.tab = BottomTab::Problems;
            return;
        }
        let Some(target) = Target::parse(&self.graph.target) else {
            self.error(self.tr("status.unknown_target"));
            return;
        };
        if !target.is_mcu() {
            self.status = self.tr("status.mcu_only").to_string();
            self.console.push(self.status.clone());
            return;
        }
        if !toolchain::can_build(target) {
            self.status = self.tr("status.no_toolchain").to_string();
            self.console.push(self.status.clone());
            match target {
                Target::Stm32F411 => self
                    .console
                    .push("install: brew install arm-none-eabi-gcc".to_string()),
                Target::Esp32 | Target::Esp8266 => self.console.push(format!(
                    "install: brew install arduino-cli && arduino-cli core install {}",
                    toolchain::esp_core_name(target).unwrap_or("")
                )),
                Target::AvrAtmega328p => self
                    .console
                    .push("install: Arduino IDE or avr-gcc/avrdude".to_string()),
                _ => {}
            }
            self.tab = BottomTab::Console;
            return;
        }
        let needs_port = target.is_avr() || target.is_esp();
        let port = self.settings.avr_port.clone();
        if flash && needs_port && port.is_none() {
            self.status = self.tr("status.need_port").to_string();
            self.open_board_settings();
            return;
        }
        let transport = self
            .settings
            .stm32_transport
            .clone()
            .unwrap_or_else(|| "stlink".to_string());
        let asm = self.out.asm.clone();
        let stem = artifact_stem(&self.graph);
        let out_dir = self.firmware_dir();
        let board = self
            .settings
            .avr_board
            .clone()
            .unwrap_or_else(|| "uno".to_string());
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let mut log = String::new();
                if target.is_avr() {
                    let build = toolchain::build_avr(&asm, &out_dir, &stem)?;
                    log.push_str(&format!("$ avr-gcc -mmcu=atmega328p -Os {stem}.s\n"));
                    if !build.size.trim().is_empty() {
                        log.push_str(build.size.trim_end());
                        log.push('\n');
                    }
                    log.push_str(&format!(
                        "[elf] {}\n[hex] {}\n[bin] {}\n",
                        build.elf.display(),
                        build.hex.display(),
                        build.bin.display()
                    ));
                    if let Some(port) = &port {
                        log.push_str(&format!(
                            "$ avrdude -p m328p -c arduino -P {port} ({board})\n"
                        ));
                        let out = toolchain::flash_avr(&build.hex, port, &board)?;
                        log.push_str(&out);
                        log.push_str("\n[upload ok]\n");
                    }
                } else if target.is_stm32() {
                    let build = toolchain::build_stm32(&asm, &out_dir, &stem)?;
                    log.push_str("$ arm-none-eabi-gcc -mcpu=cortex-m4 -mthumb -nostdlib\n");
                    if !build.size.trim().is_empty() {
                        log.push_str(build.size.trim_end());
                        log.push('\n');
                    }
                    log.push_str(&format!(
                        "[elf] {}\n[hex] {}\n[bin] {}\n",
                        build.elf.display(),
                        build.hex.display(),
                        build.bin.display()
                    ));
                    if flash {
                        log.push_str(&format!("$ {} {}\n", transport, build.bin.display()));
                        let out = toolchain::flash_stm32(&build.bin, &transport)?;
                        log.push_str(&out);
                        log.push_str("\n[upload ok]\n");
                    }
                } else if target.is_esp() {
                    if flash {
                        let port = port.as_deref().unwrap_or("");
                        log.push_str(&format!(
                            "$ arduino-cli upload -p {port} ({})\n",
                            target.as_str()
                        ));
                        let out = toolchain::flash_esp(&asm, &out_dir, &stem, target, port)?;
                        log.push_str(&out);
                        log.push_str("\n[upload ok]\n");
                    } else {
                        let build = toolchain::build_esp(&asm, &out_dir, &stem, target)?;
                        log.push_str(&format!(
                            "$ arduino-cli compile --fqbn {}\n",
                            toolchain::esp_fqbn(target).unwrap_or("")
                        ));
                        log.push_str(&format!("[sketch] {}\n", build.ino.display()));
                        log.push_str(&format!("[output] {}\n", build.output_dir.display()));
                        for binary in &build.binaries {
                            log.push_str(&format!("[bin] {}\n", binary.display()));
                        }
                    }
                }
                Ok(log)
            })();
            let _ = tx.send(result);
        });
        self.job = Some(Job {
            rx,
            started: Instant::now(),
            kind: if flash {
                JobKind::Flash
            } else {
                JobKind::Build
            },
        });
        self.status = self
            .tr(if flash {
                "status.flashing"
            } else {
                "status.building_fw"
            })
            .to_string();
        self.tab = BottomTab::Console;
        ctx.request_repaint();
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        let kind = job.kind;
        match job.rx.try_recv() {
            Ok(Ok(log)) => {
                self.job = None;
                for line in log.lines() {
                    self.console.push(line.to_string());
                }
                self.status = match kind {
                    JobKind::Run => self
                        .tr("status.run_done")
                        .replace("{}", &exit_code_of(&log)),
                    JobKind::Build => self.tr("status.build_done").to_string(),
                    JobKind::Flash => self.tr("status.flash_done").to_string(),
                };
                self.tab = BottomTab::Console;
            }
            Ok(Err(e)) => {
                self.job = None;
                for line in e.lines() {
                    self.console.push(line.to_string());
                }
                self.status = match kind {
                    JobKind::Run => self.tr("status.run_failed").to_string(),
                    JobKind::Build => self.tr("status.build_failed").to_string(),
                    JobKind::Flash => self.tr("status.flash_failed").to_string(),
                };
                self.tab = BottomTab::Console;
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            Err(TryRecvError::Disconnected) => {
                self.job = None;
                self.status = self.tr("status.run_failed").to_string();
            }
        }
    }

    fn handle_close(&mut self, ctx: &egui::Context) {
        if !ctx.input(|i| i.viewport().close_requested()) || self.allow_close {
            return;
        }
        if self.circuit.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::UnsavedCircuit(AfterCircuit::Quit));
        } else if self.dirty {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::Unsaved(After::Quit));
        }
    }

    fn sync_title(&mut self, ctx: &egui::Context) {
        let path = match self.workspace {
            Workspace::Circuit => self.circuit.path.clone(),
            _ => self.path.clone(),
        };
        let name = path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.tr("toolbar.untitled").to_string());
        let dirty = match self.workspace {
            Workspace::Circuit => self.circuit.dirty,
            _ => self.dirty,
        };
        let title = format!("{}{} — WireStudio", name, if dirty { " *" } else { "" });
        if title != self.window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let mut action = None;
        ctx.input(|i| {
            let cmd = i.modifiers.command;
            if cmd && i.key_pressed(egui::Key::N) {
                action = Some(Action::New);
            } else if cmd && i.key_pressed(egui::Key::O) {
                action = Some(Action::Open);
            } else if cmd && i.key_pressed(egui::Key::S) {
                action = Some(if i.modifiers.shift {
                    Action::SaveAs
                } else {
                    Action::Save
                });
            } else if cmd && i.key_pressed(egui::Key::Z) {
                action = Some(if i.modifiers.shift {
                    Action::Redo
                } else {
                    Action::Undo
                });
            } else if cmd && i.key_pressed(egui::Key::Enter) {
                action = Some(Action::Compile);
            } else if i.key_pressed(egui::Key::F5) {
                action = Some(Action::BuildRun);
            } else if i.key_pressed(egui::Key::F6) {
                action = Some(Action::BuildFirmware);
            } else if i.key_pressed(egui::Key::F7) {
                action = Some(Action::FlashFirmware);
            } else if i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace) {
                action = Some(Action::Delete);
            } else if i.key_pressed(egui::Key::F) {
                action = Some(Action::Fit);
            } else if i.key_pressed(egui::Key::Escape) {
                self.pending_link = None;
                self.palette_drag = None;
            }
        });
        if let Some(action) = action {
            self.act(action, ctx);
        }
    }

    // ------------------------------------------------------------------- ui

    pub fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.apply_appearance(&ctx);
        self.poll_job(&ctx);
        self.handle_close(&ctx);
        if self.needs_compile {
            self.compile_now(false);
        }
        self.sync_title(&ctx);
        self.shortcuts(&ctx);

        self.top_bar(ui);
        self.status_bar(ui);
        match self.workspace {
            Workspace::Graph => {
                self.bottom_panel(ui);
                self.palette_panel(ui);
                self.inspector_panel(ui);
                egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));
            }
            Workspace::Code => {
                self.bottom_panel(ui);
                egui::CentralPanel::default().show(ui, |ui| self.code_editor_ui(ui));
            }
            Workspace::Circuit => {
                self.circuit_ui(ui);
            }
            Workspace::Settings => {
                self.settings_ui(ui);
            }
        }

        self.windows(&ctx);
    }

    fn top_bar(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let mut action: Option<Action> = None;
        egui::Panel::top("wire_top").show(ui, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                let s = |k: &str| t(lang, k);
                ui.menu_button(s("menu.file"), |ui| {
                    if ui.button(s("file.new")).clicked() {
                        action = Some(Action::New);
                        ui.close();
                    }
                    if ui.button(s("file.open")).clicked() {
                        action = Some(Action::Open);
                        ui.close();
                    }
                    if ui.button(s("file.save")).clicked() {
                        action = Some(Action::Save);
                        ui.close();
                    }
                    if ui.button(s("file.save_as")).clicked() {
                        action = Some(Action::SaveAs);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(s("code.open")).clicked() {
                        action = Some(Action::CodeOpen);
                        ui.close();
                    }
                    if ui.button(s("code.new")).clicked() {
                        action = Some(Action::CodeNew);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(s("circuit.new")).clicked() {
                        action = Some(Action::CircuitNew);
                        ui.close();
                    }
                    if ui.button(s("circuit.open")).clicked() {
                        action = Some(Action::CircuitOpen);
                        ui.close();
                    }
                    if ui.button(s("circuit.save")).clicked() {
                        action = Some(Action::CircuitSave);
                        ui.close();
                    }
                    if ui.button(s("circuit.save_as")).clicked() {
                        action = Some(Action::CircuitSaveAs);
                        ui.close();
                    }
                    ui.menu_button(s("circuit.examples"), |ui| {
                        for (i, (key, _)) in Schematic::examples().iter().enumerate() {
                            if ui.button(s(key)).clicked() {
                                action = Some(Action::CircuitExample(i));
                                ui.close();
                            }
                        }
                    });
                    if ui.button(s("circuit.export_netlist")).clicked() {
                        action = Some(Action::CircuitExport);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(s("lib.export")).clicked() {
                        action = Some(Action::LibraryExport);
                        ui.close();
                    }
                    if ui.button(s("lib.import")).clicked() {
                        action = Some(Action::LibraryImport);
                        ui.close();
                    }
                    ui.separator();
                    ui.menu_button(s("file.examples"), |ui| {
                        for (i, (_, key, _)) in EXAMPLES.iter().enumerate() {
                            if ui.button(s(key)).clicked() {
                                action = Some(Action::Example(i));
                                ui.close();
                            }
                        }
                    });
                    ui.separator();
                    if ui.button(s("file.export_asm")).clicked() {
                        action = Some(Action::ExportAsm);
                        ui.close();
                    }
                    if ui.button(s("file.import_library")).clicked() {
                        action = Some(Action::ImportLibrary);
                        ui.close();
                    }
                    if ui.button(s("file.export_library")).clicked() {
                        action = Some(Action::ExportLibrary);
                        ui.close();
                    }
                    if ui.button(s("file.import_asm")).clicked() {
                        action = Some(Action::ImportAsm);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(s("file.quit")).clicked() {
                        action = Some(Action::Quit);
                        ui.close();
                    }
                });
                ui.menu_button(s("menu.edit"), |ui| {
                    if ui.button(s("edit.undo")).clicked() {
                        action = Some(Action::Undo);
                        ui.close();
                    }
                    if ui.button(s("edit.redo")).clicked() {
                        action = Some(Action::Redo);
                        ui.close();
                    }
                    if ui.button(s("edit.delete")).clicked() {
                        action = Some(Action::Delete);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(s("edit.compile")).clicked() {
                        action = Some(Action::Compile);
                        ui.close();
                    }
                    if ui.button(s("edit.build_run")).clicked() {
                        action = Some(Action::BuildRun);
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(s("edit.variables")).clicked() {
                        action = Some(Action::Variables);
                        ui.close();
                    }
                });
                ui.menu_button(s("menu.mcu"), |ui| {
                    if ui.button(s("mcu.use_avr")).clicked() {
                        action = Some(Action::SetTarget(Target::AvrAtmega328p));
                        ui.close();
                    }
                    if ui.button(s("mcu.use_stm32")).clicked() {
                        action = Some(Action::SetTarget(Target::Stm32F411));
                        ui.close();
                    }
                    if ui.button(s("mcu.use_esp32")).clicked() {
                        action = Some(Action::SetTarget(Target::Esp32));
                        ui.close();
                    }
                    if ui.button(s("mcu.use_esp8266")).clicked() {
                        action = Some(Action::SetTarget(Target::Esp8266));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(s("mcu.use_c")).clicked() {
                        action = Some(Action::SetTarget(Target::CLang));
                        ui.close();
                    }
                    if ui.button(s("mcu.use_cpp")).clicked() {
                        action = Some(Action::SetTarget(Target::Cpp));
                        ui.close();
                    }
                    if ui.button(s("mcu.use_csharp")).clicked() {
                        action = Some(Action::SetTarget(Target::CSharp));
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(s("toolbar.build_fw")).clicked() {
                        action = Some(Action::BuildFirmware);
                        ui.close();
                    }
                    if ui.button(s("toolbar.flash")).clicked() {
                        action = Some(Action::FlashFirmware);
                        ui.close();
                    }
                    if ui.button(s("mcu.board_settings")).clicked() {
                        action = Some(Action::BoardSettings);
                        ui.close();
                    }
                });
                ui.menu_button(s("menu.nodes"), |ui| {
                    if ui.button(s("edit.custom_node")).clicked() {
                        action = Some(Action::NewCustom);
                        ui.close();
                    }
                    if ui.button(s("file.import_library")).clicked() {
                        action = Some(Action::ImportLibrary);
                        ui.close();
                    }
                    if ui.button(s("file.export_library")).clicked() {
                        action = Some(Action::ExportLibrary);
                        ui.close();
                    }
                    if ui.button(s("file.import_asm")).clicked() {
                        action = Some(Action::ImportAsm);
                        ui.close();
                    }
                });
                ui.menu_button(s("menu.view"), |ui| {
                    if ui.button(s("workspace.graph")).clicked() {
                        self.workspace = Workspace::Graph;
                        ui.close();
                    }
                    if ui.button(s("workspace.code")).clicked() {
                        self.workspace = Workspace::Code;
                        self.refresh_generated_tab();
                        ui.close();
                    }
                    if ui.button(s("workspace.circuit")).clicked() {
                        self.workspace = Workspace::Circuit;
                        ui.close();
                    }
                    if ui.button(s("workspace.settings")).clicked() {
                        self.workspace = Workspace::Settings;
                        ui.close();
                    }
                    ui.separator();
                    if ui.button(s("edit.fit")).clicked() {
                        action = Some(Action::Fit);
                        ui.close();
                    }
                    if ui.button(s("edit.zoom_in")).clicked() {
                        action = Some(Action::ZoomIn);
                        ui.close();
                    }
                    if ui.button(s("edit.zoom_out")).clicked() {
                        action = Some(Action::ZoomOut);
                        ui.close();
                    }
                });
                ui.menu_button(s("menu.language"), |ui| {
                    if ui
                        .selectable_label(self.settings.lang.is_none(), s("lang.system"))
                        .clicked()
                    {
                        action = Some(Action::SetLang(None));
                        ui.close();
                    }
                    for l in [Lang::En, Lang::Ru] {
                        if ui.selectable_label(self.lang == l, l.label()).clicked() {
                            action = Some(Action::SetLang(Some(l)));
                            ui.close();
                        }
                    }
                });
                ui.menu_button(s("menu.help"), |ui| {
                    if ui.button(s("edit.shortcuts")).clicked() {
                        action = Some(Action::Shortcuts);
                        ui.close();
                    }
                    if ui.button(s("help.about")).clicked() {
                        action = Some(Action::About);
                        ui.close();
                    }
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                let s = |k: &str| t(lang, k);
                if ui.button(s("file.open")).clicked() {
                    action = Some(Action::Open);
                }
                if ui.button(s("file.save")).clicked() {
                    action = Some(Action::Save);
                }
                if ui.button(s("toolbar.compile")).clicked() {
                    action = Some(Action::Compile);
                }
                if ui.button(s("toolbar.run")).clicked() {
                    action = Some(Action::BuildRun);
                }
                ui.separator();
                if ui
                    .selectable_label(self.workspace == Workspace::Graph, s("workspace.graph"))
                    .clicked()
                {
                    self.workspace = Workspace::Graph;
                }
                if ui
                    .selectable_label(self.workspace == Workspace::Code, s("workspace.code"))
                    .clicked()
                {
                    self.workspace = Workspace::Code;
                    self.refresh_generated_tab();
                }
                if ui
                    .selectable_label(self.workspace == Workspace::Circuit, s("workspace.circuit"))
                    .clicked()
                {
                    self.workspace = Workspace::Circuit;
                }
                if ui
                    .selectable_label(self.workspace == Workspace::Settings, s("workspace.settings"))
                    .clicked()
                {
                    self.workspace = Workspace::Settings;
                }
                if self.workspace == Workspace::Circuit {
                    if ui.button(s("circuit.run")).clicked() {
                        action = Some(Action::CircuitRun);
                    }
                    if ui.button(s("circuit.fit")).clicked() {
                        action = Some(Action::CircuitFit);
                    }
                }
                ui.separator();
                if Target::parse(&self.graph.target)
                    .map(|t| t.is_mcu())
                    .unwrap_or(false)
                {
                    if ui.button(s("toolbar.build_fw")).clicked() {
                        action = Some(Action::BuildFirmware);
                    }
                    if ui.button(s("toolbar.flash")).clicked() {
                        action = Some(Action::FlashFirmware);
                    }
                }
                ui.separator();
                ui.label(s("toolbar.target"));
                let current = self.graph.target.clone();
                egui::ComboBox::from_id_salt("target_combo")
                    .selected_text(current.clone())
                    .width(145.0)
                    .show_ui(ui, |ui| {
                        for target in Target::all().iter().copied() {
                            let mut label = target.label().to_string();
                            if !toolchain::can_build(target) {
                                label.push_str(&format!(" — {}", s("target.not_buildable")));
                            }
                            if ui
                                .selectable_label(current == target.as_str(), label)
                                .clicked()
                            {
                                action = Some(Action::SetTarget(target));
                            }
                        }
                    });
                ui.separator();
                if let Some(job) = &self.job {
                    ui.spinner();
                    ui.label(
                        RichText::new(format!("{:.1}s", job.started.elapsed().as_secs_f32()))
                            .small()
                            .weak(),
                    );
                }
                // The file name and the zoom sit at the right edge. They go
                // into one group, otherwise they overlap each other.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let (path, dirty) = match self.workspace {
                        Workspace::Circuit => (self.circuit.path.clone(), self.circuit.dirty),
                        _ => (self.path.clone(), self.dirty),
                    };
                    let mut name = path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| s("toolbar.untitled").to_string());
                    if dirty {
                        name.push_str(" *");
                    }
                    ui.label(RichText::new(name).monospace());
                    ui.separator();
                    let in_circuit = self.workspace == Workspace::Circuit;
                    let zoom = if in_circuit {
                        self.circuit.view.zoom
                    } else {
                        self.view.zoom
                    };
                    ui.label(format!("{:.0}%", zoom * 100.0));
                    if ui.button(s("toolbar.fit")).clicked() {
                        action = Some(if in_circuit {
                            Action::CircuitFit
                        } else {
                            Action::Fit
                        });
                    }
                });
            });
        });
        if let Some(action) = action {
            self.act(action, &ui.ctx().clone());
        }
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("wire_status").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).small());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        RichText::new(format!("{} · {}", self.graph.target, self.out.stats.lines))
                            .small()
                            .weak(),
                    );
                });
            });
        });
    }

    fn bottom_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("wire_bottom")
            .resizable(true)
            .default_size(250.0)
            .min_size(120.0)
            .show(ui, |ui| {
                let lang = self.lang;
                let s = |k: &str| t(lang, k);
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.tab == BottomTab::Asm, s("bottom.asm"))
                        .clicked()
                    {
                        self.tab = BottomTab::Asm;
                    }
                    let errors = self
                        .out
                        .diagnostics
                        .iter()
                        .filter(|d| d.severity == Severity::Error)
                        .count();
                    let problems = if errors > 0 {
                        format!("{} ({errors})", s("bottom.problems"))
                    } else {
                        s("bottom.problems").to_string()
                    };
                    if ui
                        .selectable_label(self.tab == BottomTab::Problems, problems)
                        .clicked()
                    {
                        self.tab = BottomTab::Problems;
                    }
                    if ui
                        .selectable_label(self.tab == BottomTab::Console, s("bottom.console"))
                        .clicked()
                    {
                        self.tab = BottomTab::Console;
                    }
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| match self.tab {
                        BottomTab::Asm => {
                            if ui.button(s("bottom.copy")).clicked() {
                                ui.ctx().copy_text(self.out.asm.clone());
                                self.status = s("status.copied").to_string();
                            }
                            if ui.button(s("bottom.save_asm")).clicked() {
                                self.export_asm();
                            }
                        }
                        BottomTab::Console => {
                            if ui.button(s("bottom.clear")).clicked() {
                                self.console.clear();
                            }
                            if ui.button(s("bottom.copy")).clicked() {
                                ui.ctx().copy_text(self.console.join("\n"));
                                self.status = s("status.copied").to_string();
                            }
                        }
                        BottomTab::Problems => {}
                    });
                });
                ui.separator();
                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| match self.tab {
                        BottomTab::Asm => {
                            if self.out.asm.trim().is_empty() {
                                ui.label(s("bottom.no_asm"));
                            } else {
                                ui.add(
                                    egui::Label::new(
                                        RichText::new(&self.out.asm).monospace().size(12.0),
                                    )
                                    .selectable(true),
                                );
                            }
                        }
                        BottomTab::Problems => {
                            if self.out.diagnostics.is_empty() {
                                ui.label(s("bottom.no_problems"));
                            }
                            let mut jump = None;
                            for d in &self.out.diagnostics {
                                let (color, sev) = match d.severity {
                                    Severity::Error => (Color32::from_rgb(235, 105, 105), "error"),
                                    Severity::Warning => {
                                        (Color32::from_rgb(230, 180, 80), "warning")
                                    }
                                    Severity::Info => (Color32::from_rgb(120, 180, 230), "info"),
                                };
                                ui.horizontal(|ui| {
                                    let text = format!(
                                        "[{}] {}",
                                        s(&format!("severity.{sev}")),
                                        d.message
                                    );
                                    if ui
                                        .add(
                                            egui::Label::new(
                                                RichText::new(text).color(color).monospace(),
                                            )
                                            .selectable(true)
                                            .sense(egui::Sense::click()),
                                        )
                                        .clicked()
                                    {
                                        if let Some(node) = &d.node {
                                            jump = Some(node.clone());
                                        }
                                    }
                                });
                            }
                            if let Some(node) = jump {
                                if self.graph.node(&node).is_some() {
                                    self.selected = Some(node);
                                }
                            }
                        }
                        BottomTab::Console => {
                            if self.console.is_empty() {
                                ui.label(s("bottom.no_output"));
                            }
                            for line in &self.console {
                                ui.add(
                                    egui::Label::new(RichText::new(line).monospace().size(12.0))
                                        .selectable(true),
                                );
                            }
                        }
                    });
            });
    }

    fn palette_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::left("wire_palette")
            .resizable(true)
            .default_size(236.0)
            .min_size(170.0)
            .show(ui, |ui| {
                let lang = self.lang;
                let s = |k: &str| t(lang, k);
                ui.add_space(4.0);
                ui.heading(s("palette.title"));
                ui.add(
                    egui::TextEdit::singleline(&mut self.palette_filter)
                        .hint_text(s("palette.search"))
                        .desired_width(f32::INFINITY),
                );
                ui.add_space(4.0);
                let filter = self.palette_filter.to_ascii_lowercase();
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        self.palette_group(
                            ui,
                            "palette.flow",
                            &["Entry", "Branch", "Anchor", "Return"],
                            &filter,
                        );
                        self.palette_group(
                            ui,
                            "palette.values",
                            &[
                                "Const", "VarGet", "VarSet", "BinOp", "Cmp", "RegGet", "RegSet",
                            ],
                            &filter,
                        );
                        self.palette_group(
                            ui,
                            "palette.output",
                            &["PrintU64", "PrintStr"],
                            &filter,
                        );
                        let avr = Target::parse(&self.graph.target)
                            .map(|t| t.is_avr())
                            .unwrap_or(false);
                        if avr {
                            self.palette_group(
                                ui,
                                "palette.mcu",
                                &[
                                    "McuPinMode",
                                    "McuDigitalWrite",
                                    "McuDigitalRead",
                                    "McuDelayMs",
                                    "McuDelayUs",
                                    "McuUartInit",
                                    "McuUartWrite",
                                    "McuUartWriteStr",
                                    "McuAdcRead",
                                    "McuRegisterWrite",
                                    "McuRegisterRead",
                                ],
                                &filter,
                            );
                        }
                        self.palette_group(
                            ui,
                            "palette.system",
                            &["Call", "Syscall", "RawAsm"],
                            &filter,
                        );
                        self.palette_group(ui, "palette.notes", &["Comment"], &filter);
                        let kinds: Vec<String> = self
                            .graph
                            .custom_nodes
                            .iter()
                            .map(|d| d.kind.clone())
                            .collect();
                        if !kinds.is_empty() {
                            let refs: Vec<&str> = kinds.iter().map(|s| s.as_str()).collect();
                            self.palette_group(ui, "palette.custom", &refs, &filter);
                        }
                    });
                ui.separator();
                ui.label(RichText::new(s("palette.hint")).small().weak());
                let avr = Target::parse(&self.graph.target)
                    .map(|t| t.is_avr())
                    .unwrap_or(false);
                if !avr {
                    ui.label(RichText::new(s("palette.mcu_hint")).small().weak());
                }
            });
    }

    fn palette_group(&mut self, ui: &mut egui::Ui, title: &str, kinds: &[&str], filter: &str) {
        let visible: Vec<&str> = kinds
            .iter()
            .copied()
            .filter(|kind| {
                filter.is_empty()
                    || kind.to_ascii_lowercase().contains(filter)
                    || self.node_title(kind).to_ascii_lowercase().contains(filter)
            })
            .collect();
        if visible.is_empty() {
            return;
        }
        ui.add_space(6.0);
        ui.label(RichText::new(self.tr(title)).small().strong().weak());
        for kind in visible {
            self.palette_item(ui, kind);
        }
    }

    fn palette_item(&mut self, ui: &mut egui::Ui, kind: &str) {
        let title = self.node_title(kind);
        let width = ui.available_width().max(120.0);
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(width, 26.0), egui::Sense::click_and_drag());
        let fill = if resp.hovered() {
            Color32::from_rgb(46, 52, 68)
        } else {
            Color32::from_rgb(32, 36, 46)
        };
        ui.painter()
            .rect_filled(rect, egui::CornerRadius::same(6), fill);
        let mut text = title;
        if text.chars().count() > 24 {
            text = text.chars().take(23).collect::<String>() + "…";
        }
        ui.painter().text(
            rect.left_center() + egui::vec2(8.0, 0.0),
            Align2::LEFT_CENTER,
            text,
            egui::FontId::proportional(13.0),
            Color32::from_rgb(220, 226, 236),
        );
        if resp.clicked() {
            self.add_node_centered(kind);
        }
        if resp.drag_started() {
            self.palette_drag = Some(kind.to_string());
        }
        ui.add_space(2.0);
    }

    fn windows(&mut self, ctx: &egui::Context) {
        self.variables_window(ctx);
        self.custom_editor_window(ctx);
        self.confirm_window(ctx);
        self.board_window(ctx);
        self.about_window(ctx);
        self.shortcuts_window(ctx);
        self.instrument_windows(ctx);
    }

    fn board_window(&mut self, ctx: &egui::Context) {
        if !self.board_open {
            return;
        }
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        let mut open = true;
        let mut close = false;
        let mut action: Option<Action> = None;
        egui::Window::new(s("board.title"))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(420.0)
            .show(ctx, |ui| {
                let mut changed = false;
                let target = Target::parse(&self.graph.target).unwrap_or(Target::Aarch64Macos);
                if target.is_avr() || target.is_esp() {
                    ui.label(RichText::new(s("board.port")).small().weak());
                    ui.horizontal(|ui| {
                        let mut port = self.settings.avr_port.clone().unwrap_or_default();
                        let resp = ui.add(
                            egui::TextEdit::singleline(&mut port)
                                .desired_width(210.0)
                                .hint_text("/dev/cu.usbmodem…"),
                        );
                        if resp.changed() {
                            self.settings.avr_port = if port.trim().is_empty() {
                                None
                            } else {
                                Some(port.trim().to_string())
                            };
                            changed = true;
                        }
                        if !self.avr_ports.is_empty() {
                            egui::ComboBox::from_id_salt("avr_port_pick")
                                .selected_text(s("board.port"))
                                .width(120.0)
                                .show_ui(ui, |ui| {
                                    for p in self.avr_ports.clone() {
                                        if ui.selectable_label(false, &p).clicked() {
                                            self.settings.avr_port = Some(p);
                                            changed = true;
                                        }
                                    }
                                });
                        }
                        if ui.button(s("board.refresh")).clicked() {
                            self.avr_ports = toolchain::avr_ports();
                            if self.settings.avr_port.is_none() {
                                self.settings.avr_port = self.avr_ports.first().cloned();
                            }
                            changed = true;
                        }
                    });
                    if self.avr_ports.is_empty() {
                        ui.label(RichText::new(s("board.no_ports")).small().weak());
                    }
                }

                if target.is_avr() {
                    ui.add_space(6.0);
                    ui.label(RichText::new(s("board.device")).small().weak());
                    let mut board = self
                        .settings
                        .avr_board
                        .clone()
                        .unwrap_or_else(|| "uno".to_string());
                    egui::ComboBox::from_id_salt("avr_board_pick")
                        .selected_text(if board.eq_ignore_ascii_case("nano") {
                            s("board.nano")
                        } else {
                            s("board.uno")
                        })
                        .width(210.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(board.eq_ignore_ascii_case("uno"), s("board.uno"))
                                .clicked()
                            {
                                board = "uno".to_string();
                                changed = true;
                            }
                            if ui
                                .selectable_label(
                                    board.eq_ignore_ascii_case("nano"),
                                    s("board.nano"),
                                )
                                .clicked()
                            {
                                board = "nano".to_string();
                                changed = true;
                            }
                        });
                    self.settings.avr_board = Some(board);
                }

                if target.is_stm32() {
                    ui.add_space(6.0);
                    ui.label(RichText::new(s("board.transport")).small().weak());
                    let mut transport = self
                        .settings
                        .stm32_transport
                        .clone()
                        .unwrap_or_else(|| "stlink".to_string());
                    egui::ComboBox::from_id_salt("stm32_transport_pick")
                        .selected_text(if transport.eq_ignore_ascii_case("dfu") {
                            s("board.dfu")
                        } else {
                            s("board.stlink")
                        })
                        .width(210.0)
                        .show_ui(ui, |ui| {
                            if ui
                                .selectable_label(
                                    transport.eq_ignore_ascii_case("stlink"),
                                    s("board.stlink"),
                                )
                                .clicked()
                            {
                                transport = "stlink".to_string();
                                changed = true;
                            }
                            if ui
                                .selectable_label(
                                    transport.eq_ignore_ascii_case("dfu"),
                                    s("board.dfu"),
                                )
                                .clicked()
                            {
                                transport = "dfu".to_string();
                                changed = true;
                            }
                        });
                    self.settings.stm32_transport = Some(transport);
                }

                if target.is_esp() {
                    ui.add_space(6.0);
                    ui.label(RichText::new(s("board.esp_core")).small().weak());
                    let installed = toolchain::esp_core_installed(target);
                    let status = if installed {
                        s("board.core_installed")
                    } else {
                        s("board.core_missing")
                    };
                    ui.label(RichText::new(status).small().weak());
                    if !installed {
                        ui.label(
                            RichText::new(format!(
                                "arduino-cli core install {}",
                                toolchain::esp_core_name(target).unwrap_or("")
                            ))
                            .monospace()
                            .small(),
                        );
                    }
                }

                ui.add_space(6.0);
                ui.label(RichText::new(s("board.folder")).small().weak());
                ui.label(RichText::new(self.firmware_dir().display().to_string()).monospace());
                let mut folder = self.settings.out_dir.clone().unwrap_or_default();
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut folder)
                        .desired_width(f32::INFINITY)
                        .hint_text(s("board.folder_default")),
                );
                if resp.changed() {
                    self.settings.out_dir = if folder.trim().is_empty() {
                        None
                    } else {
                        Some(folder.trim().to_string())
                    };
                    changed = true;
                }

                ui.add_space(8.0);
                ui.label(RichText::new(s("board.hint")).small().weak());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(s("toolbar.build_fw")).clicked() {
                        action = Some(Action::BuildFirmware);
                    }
                    if ui.button(s("toolbar.flash")).clicked() {
                        action = Some(Action::FlashFirmware);
                    }
                    if ui.button(s("board.close")).clicked() {
                        close = true;
                    }
                });
                if changed {
                    self.settings.save();
                }
            });
        if close || !open {
            self.board_open = false;
        }
        if let Some(action) = action {
            self.act(action, ctx);
        }
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        match self.confirm {
            Some(Confirm::Unsaved(after)) => self.graph_confirm_window(ctx, after),
            Some(Confirm::UnsavedCircuit(after)) => self.circuit_confirm_window(ctx, after),
            None => {}
        }
    }

    fn graph_confirm_window(&mut self, ctx: &egui::Context, after: After) {
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        let mut choice: Option<u8> = None;
        egui::Window::new(s("unsaved.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(s("unsaved.text"));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(s("unsaved.save")).clicked() {
                        choice = Some(0);
                    }
                    if ui.button(s("unsaved.discard")).clicked() {
                        choice = Some(1);
                    }
                    if ui.button(s("unsaved.cancel")).clicked() {
                        choice = Some(2);
                    }
                });
            });
        match choice {
            Some(0) => {
                if self.save(false) {
                    self.confirm = None;
                    self.after_unsaved(after, ctx);
                }
            }
            Some(1) => {
                self.confirm = None;
                self.after_unsaved(after, ctx);
            }
            Some(2) | None => {
                self.confirm = None;
            }
            _ => {}
        }
    }

    fn circuit_confirm_window(&mut self, ctx: &egui::Context, after: AfterCircuit) {
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        let mut choice: Option<u8> = None;
        egui::Window::new(s("unsaved.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(s("unsaved.circuit_text"));
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(s("unsaved.save")).clicked() {
                        choice = Some(0);
                    }
                    if ui.button(s("unsaved.discard")).clicked() {
                        choice = Some(1);
                    }
                    if ui.button(s("unsaved.cancel")).clicked() {
                        choice = Some(2);
                    }
                });
            });
        match choice {
            Some(0) => {
                if self.circuit_save(false) {
                    self.confirm = None;
                    self.after_circuit_unsaved(after, Some(ctx));
                }
            }
            Some(1) => {
                self.confirm = None;
                self.after_circuit_unsaved(after, Some(ctx));
            }
            Some(2) | None => {
                self.confirm = None;
            }
            _ => {}
        }
    }

    fn after_circuit_unsaved(&mut self, after: AfterCircuit, ctx: Option<&egui::Context>) {
        match after {
            AfterCircuit::Quit => {
                self.allow_close = true;
                if let Some(ctx) = ctx {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            AfterCircuit::New => {
                self.circuit.model = Schematic::empty();
                self.circuit.result = None;
                self.circuit.dirty = false;
                self.status = self.tr("status.circuit_new").to_string();
            }
            AfterCircuit::Open => self.circuit_open(),
            AfterCircuit::Example(idx) => self.circuit_load_example(idx),
        }
    }

    fn after_unsaved(&mut self, after: After, ctx: &egui::Context) {
        if after == After::Quit {
            self.allow_close = true;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        } else {
            self.dirty = false;
            self.proceed(after);
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        if !self.show_about {
            return;
        }
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        let mut close = false;
        egui::Window::new(s("about.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.label(s("about.text").replace("{}", wireasm_core::VERSION));
                ui.add_space(8.0);
                if ui.button(s("about.close")).clicked() {
                    close = true;
                }
            });
        if close {
            self.show_about = false;
        }
    }

    fn shortcuts_window(&mut self, ctx: &egui::Context) {
        if !self.show_shortcuts {
            return;
        }
        let lang = self.lang;
        let s = |k: &str| t(lang, k);
        let mut close = false;
        egui::Window::new(s("shortcuts.title"))
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.add(egui::Label::new(
                    RichText::new(s("shortcuts.text")).monospace(),
                ));
                ui.add_space(8.0);
                if ui.button(s("about.close")).clicked() {
                    close = true;
                }
            });
        if close {
            self.show_shortcuts = false;
        }
    }
}

impl eframe::App for WireStudio {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        self.ui(ui, frame);
    }
}

// ------------------------------------------------------------------ helpers

/// Whether the file holds source code rather than a graph.
fn is_source_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("c" | "h" | "cpp" | "hpp" | "cc" | "cs" | "ino" | "s" | "asm" | "rs" | "txt")
    )
}

pub fn default_graph() -> Graph {
    let target = toolchain::host_target().unwrap_or(Target::Aarch64Macos);
    Graph {
        name: "untitled".to_string(),
        target: target.as_str().to_string(),
        variables: Vec::new(),
        nodes: vec![
            Node {
                id: "entry".to_string(),
                kind: "Entry".to_string(),
                x: 80.0,
                y: 240.0,
                props: Default::default(),
            },
            Node {
                id: "zero".to_string(),
                kind: "Const".to_string(),
                x: 80.0,
                y: 430.0,
                props: [("value".to_string(), "0".to_string())].into(),
            },
            Node {
                id: "ret".to_string(),
                kind: "Return".to_string(),
                x: 560.0,
                y: 240.0,
                props: Default::default(),
            },
        ],
        edges: vec![
            Edge {
                from_node: "entry".to_string(),
                from_pin: "exec".to_string(),
                to_node: "ret".to_string(),
                to_pin: "exec".to_string(),
            },
            Edge {
                from_node: "zero".to_string(),
                from_pin: "value".to_string(),
                to_node: "ret".to_string(),
                to_pin: "value".to_string(),
            },
        ],
        custom_nodes: Vec::new(),
    }
}

pub fn node_key(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "Entry" => "node.Entry",
        "Const" => "node.Const",
        "VarGet" => "node.VarGet",
        "VarSet" => "node.VarSet",
        "BinOp" => "node.BinOp",
        "Cmp" => "node.Cmp",
        "Branch" => "node.Branch",
        "PrintU64" => "node.PrintU64",
        "PrintStr" => "node.PrintStr",
        "Return" => "node.Return",
        "RegGet" => "node.RegGet",
        "RegSet" => "node.RegSet",
        "Call" => "node.Call",
        "Syscall" => "node.Syscall",
        "RawAsm" => "node.RawAsm",
        "Anchor" => "node.Anchor",
        "Comment" => "node.Comment",
        "McuPinMode" => "node.McuPinMode",
        "McuDigitalWrite" => "node.McuDigitalWrite",
        "McuDigitalRead" => "node.McuDigitalRead",
        "McuDelayMs" => "node.McuDelayMs",
        "McuDelayUs" => "node.McuDelayUs",
        "McuUartInit" => "node.McuUartInit",
        "McuUartWrite" => "node.McuUartWrite",
        "McuUartWriteStr" => "node.McuUartWriteStr",
        "McuAdcRead" => "node.McuAdcRead",
        "McuRegisterWrite" => "node.McuRegisterWrite",
        "McuRegisterRead" => "node.McuRegisterRead",
        _ => return None,
    })
}

pub fn with_extension(path: PathBuf, ext: &str) -> PathBuf {
    if path.extension().is_none() {
        path.with_extension(ext)
    } else {
        path
    }
}

fn exit_code_of(log: &str) -> String {
    log.lines()
        .rev()
        .find_map(|l| l.strip_prefix("[exit "))
        .map(|s| s.trim_end_matches(']').to_string())
        .unwrap_or_else(|| "0".to_string())
}

pub fn sanitize_kind(s: &str) -> String {
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        let ok = c.is_ascii_alphanumeric() || c == '_' || c == '-';
        if ok {
            if i == 0 && c.is_ascii_digit() {
                out.push('_');
            }
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("node");
    }
    out
}

fn valid_kind(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn pins_ok(pins: &[CustomPin]) -> bool {
    let mut names = std::collections::BTreeSet::new();
    let mut execs = 0;
    for p in pins {
        if p.name.trim().is_empty() || !names.insert(p.name.clone()) {
            return false;
        }
        match p.ty.as_str() {
            TY_EXEC => {
                if p.name != "exec" {
                    return false;
                }
                execs += 1;
            }
            TY_U64 => {
                if p.name == "exec" {
                    return false;
                }
            }
            _ => return false,
        }
    }
    execs <= 1
}
