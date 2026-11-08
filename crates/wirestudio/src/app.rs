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

use crate::i18n::{t, Lang};
use crate::settings::Settings;

pub const EXAMPLES: &[(&str, &str, &str)] = &[
    ("hello", "example.hello", include_str!("../../../examples/hello.json")),
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
    Variables,
    NewCustom,
    Shortcuts,
    About,
    SetLang(Option<Lang>),
    SetTarget(Target),
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
            allow_close: false,
            job: None,
            undo: Vec::new(),
            redo: Vec::new(),
            status: String::new(),
            fit_requested: true,
            last_canvas: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0)),
            window_title: String::new(),
        };
        app.status = app.tr("status.ready").to_string();
        if let Some(path) = initial {
            app.open_path(&path);
        }
        app
    }

    pub fn tr(&self, key: &str) -> &'static str {
        t(self.lang, key)
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
        self.undo.push(self.graph.to_json_value().stringify_pretty());
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
                        self.status = self.tr("status.auto_saved").replace("{}", &asm.display().to_string());
                    }
                }
            } else {
                let errors = self
                    .out
                    .diagnostics
                    .iter()
                    .filter(|d| d.severity == Severity::Error)
                    .count();
                self.status = self.tr("status.compile_failed").replace("{}", &errors.to_string());
                self.tab = BottomTab::Problems;
            }
        }
    }

    fn error(&mut self, msg: impl Into<String>) {
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
            .or_else(|| self.path.as_ref().and_then(|p| p.parent().map(|p| p.to_path_buf())))
    }

    fn remember_dir(&mut self, path: &Path) {
        if let Some(parent) = path.parent() {
            self.settings.last_dir = Some(parent.display().to_string());
            self.settings.save();
        }
    }

    fn default_file_name(&self, ext: &str) -> String {
        format!("{}.{}", artifact_stem(&self.graph), ext)
    }

    fn pick_open(&mut self, title: &str, filters: &[(&str, &[&str])]) -> Option<PathBuf> {
        let mut dlg = rfd::FileDialog::new().set_title(title);
        if let Some(dir) = self.dialog_dir() {
            dlg = dlg.set_directory(dir);
        }
        for (name, exts) in filters {
            dlg = dlg.add_filter(*name, exts);
        }
        dlg.pick_file()
    }

    fn pick_save(
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
        self.status = self.tr("status.saved").replace("{}", &path.display().to_string());
        self.console
            .push(self.status.clone());
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
                self.status = self.tr("status.opened").replace("{}", &path.display().to_string());
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

    fn load_example(&mut self, idx: usize) {
        let Some((_, _, src)) = EXAMPLES.get(idx) else { return };
        let parsed = Json::parse(src).and_then(|v| Graph::from_json_value(&v));
        match parsed {
            Ok(mut graph) => {
                if let Some(host) = toolchain::host_target() {
                    graph.target = host.as_str().to_string();
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
                    self.status = self.tr("status.exported_lib").replace("{}", &path.display().to_string());
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
        self.status = self
            .tr("status.imported_asm")
            .replace("{}", &stem);
        self.console.push(self.status.clone());
    }

    pub fn save_custom_def(&mut self, original: Option<String>, def: CustomNodeDef) -> Result<(), String> {
        if def.kind.trim().is_empty() || !valid_kind(&def.kind) {
            return Err(self.tr("custom.error_kind").to_string());
        }
        if pins_for(&def.kind).is_some() {
            return Err(self.tr("custom.error_builtin").to_string());
        }
        if original.as_deref() != Some(def.kind.as_str()) && self.graph.custom(&def.kind).is_some() {
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
            Action::Save => {
                self.save(false);
            }
            Action::SaveAs => {
                self.save(true);
            }
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
            Action::Delete => self.delete_selected(),
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
        let Some(id) = self.selected.clone() else { return };
        self.snapshot();
        self.graph.edges.retain(|e| e.from_node != id && e.to_node != id);
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
        if !toolchain::can_build(target) {
            self.status = self
                .tr("status.host_only")
                .replace("{}", target.as_str());
            return;
        }
        let asm = self.out.asm.clone();
        let stem = artifact_stem(&self.graph);
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let dir = std::env::temp_dir().join("wirestudio");
                std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
                let asm_path = dir.join(format!("{stem}.s"));
                let exe_path = dir.join(format!("{stem}-{}", std::process::id()));
                std::fs::write(&asm_path, asm).map_err(|e| format!("write asm: {e}"))?;
                toolchain::assemble(&asm_path, &exe_path, target)?;
                let run = toolchain::run(&exe_path, 5)?;
                let mut log = format!("$ {}:{} ({})\n", asm_path.display(), stem, target.as_str());
                log.push_str(&run.stdout);
                log.push_str(&run.stderr);
                if run.timed_out {
                    log.push_str("[killed after 5 s]\n");
                }
                log.push_str(&format!(
                    "[exit {}]\n",
                    run.status.map(|c| c.to_string()).unwrap_or_else(|| "signal".to_string())
                ));
                Ok(log)
            })();
            let _ = tx.send(result);
        });
        self.job = Some(Job {
            rx,
            started: Instant::now(),
        });
        self.status = self.tr("status.running").to_string();
        self.tab = BottomTab::Console;
        ctx.request_repaint();
    }

    fn poll_job(&mut self, ctx: &egui::Context) {
        let Some(job) = &self.job else { return };
        match job.rx.try_recv() {
            Ok(Ok(log)) => {
                self.job = None;
                for line in log.lines() {
                    self.console.push(line.to_string());
                }
                self.status = self
                    .tr("status.run_done")
                    .replace("{}", &exit_code_of(&log));
                self.tab = BottomTab::Console;
            }
            Ok(Err(e)) => {
                self.job = None;
                for line in e.lines() {
                    self.console.push(line.to_string());
                }
                self.status = self.tr("status.run_failed").to_string();
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
        if ctx.input(|i| i.viewport().close_requested()) && self.dirty && !self.allow_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::Unsaved(After::Quit));
        }
    }

    fn sync_title(&mut self, ctx: &egui::Context) {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| self.tr("toolbar.untitled").to_string());
        let title = format!(
            "{}{} — WireStudio",
            name,
            if self.dirty { " *" } else { "" }
        );
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
        self.poll_job(&ctx);
        self.handle_close(&ctx);
        if self.needs_compile {
            self.compile_now(false);
        }
        self.sync_title(&ctx);
        self.shortcuts(&ctx);

        self.top_bar(ui);
        self.status_bar(ui);
        self.bottom_panel(ui);
        self.palette_panel(ui);
        self.inspector_panel(ui);
        egui::CentralPanel::default().show(ui, |ui| self.canvas(ui));

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
                        if ui
                            .selectable_label(self.lang == l, l.label())
                            .clicked()
                        {
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
                ui.label(s("toolbar.target"));
                let current = self.graph.target.clone();
                egui::ComboBox::from_id_salt("target_combo")
                    .selected_text(current.clone())
                    .width(145.0)
                    .show_ui(ui, |ui| {
                        for target in Target::all() {
                            let mut label = target.as_str().to_string();
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
                if ui.button(s("toolbar.fit")).clicked() {
                    action = Some(Action::Fit);
                }
                ui.label(format!("{:.0}%", self.view.zoom * 100.0));
                if let Some(job) = &self.job {
                    ui.spinner();
                    ui.label(
                        RichText::new(format!("{:.1}s", job.started.elapsed().as_secs_f32()))
                            .small()
                            .weak(),
                    );
                }
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    let mut name = self
                        .path
                        .as_ref()
                        .and_then(|p| p.file_name())
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| s("toolbar.untitled").to_string());
                    if self.dirty {
                        name.push_str(" *");
                    }
                    ui.label(RichText::new(name).monospace());
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
                        RichText::new(format!(
                            "{} · {}",
                            self.graph.target,
                            self.out.stats.lines
                        ))
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
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        match self.tab {
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
                        }
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
                                    Severity::Warning => (Color32::from_rgb(230, 180, 80), "warning"),
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
                                    egui::Label::new(
                                        RichText::new(line).monospace().size(12.0),
                                    )
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
                        self.palette_group(ui, "palette.flow", &["Entry", "Branch", "Anchor", "Return"], &filter);
                        self.palette_group(
                            ui,
                            "palette.values",
                            &["Const", "VarGet", "VarSet", "BinOp", "Cmp", "RegGet", "RegSet"],
                            &filter,
                        );
                        self.palette_group(ui, "palette.output", &["PrintU64", "PrintStr"], &filter);
                        self.palette_group(ui, "palette.system", &["Call", "Syscall", "RawAsm"], &filter);
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
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, 26.0), egui::Sense::click_and_drag());
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
        self.about_window(ctx);
        self.shortcuts_window(ctx);
    }

    fn confirm_window(&mut self, ctx: &egui::Context) {
        let Some(Confirm::Unsaved(after)) = self.confirm else {
            return;
        };
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
                ui.add(egui::Label::new(RichText::new(s("shortcuts.text")).monospace()));
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
