//! Classic code editor: tabs, syntax highlighting, open/save, build and run.
//!
//! The graph workspace generates source; this workspace shows it like a normal
//! editor so the generated assembler/C/C++/C# can be read, edited and built.
//! Highlighting is a small hand-written lexer: no external dependencies.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Instant;

use eframe::egui::{self, text::LayoutJob, Color32, FontId, RichText, TextFormat};
use wireasm_core::model::Target;
use wireasm_core::{artifact_stem, toolchain};

use crate::app::{BottomTab, Job, JobKind, WireStudio};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Workspace {
    Graph,
    Code,
    Circuit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Syntax {
    Asm,
    C,
    Cpp,
    CSharp,
    Plain,
}

impl Syntax {
    pub fn from_path(path: &Path) -> Syntax {
        match path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "s" | "asm" | "S" => Syntax::Asm,
            "c" | "h" => Syntax::C,
            "cpp" | "cc" | "cxx" | "hpp" | "ino" => Syntax::Cpp,
            "cs" => Syntax::CSharp,
            _ => Syntax::Plain,
        }
    }

    pub fn from_target(t: Target) -> Syntax {
        match t {
            Target::CLang => Syntax::C,
            Target::Cpp | Target::Esp32 | Target::Esp8266 => Syntax::Cpp,
            Target::CSharp => Syntax::CSharp,
            _ => Syntax::Asm,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Syntax::Asm => "asm",
            Syntax::C => "C",
            Syntax::Cpp => "C++",
            Syntax::CSharp => "C#",
            Syntax::Plain => "text",
        }
    }
}

pub struct CodeTab {
    pub path: Option<PathBuf>,
    pub title: String,
    pub text: String,
    pub dirty: bool,
    pub syntax: Syntax,
    /// The tab mirrors compiler output and can be refreshed after a compile.
    pub generated: bool,
}

impl CodeTab {
    pub fn untitled(syntax: Syntax) -> CodeTab {
        CodeTab {
            path: None,
            title: format!("untitled.{}", default_ext(syntax)),
            text: String::new(),
            dirty: false,
            syntax,
            generated: false,
        }
    }
}

fn default_ext(syntax: Syntax) -> &'static str {
    match syntax {
        Syntax::Asm => "s",
        Syntax::C => "c",
        Syntax::Cpp => "cpp",
        Syntax::CSharp => "cs",
        Syntax::Plain => "txt",
    }
}

#[derive(Default)]
pub struct CodeEditor {
    pub tabs: Vec<CodeTab>,
    pub active: usize,
    pub line_numbers: bool,
}

impl CodeEditor {
    pub fn new() -> CodeEditor {
        CodeEditor {
            tabs: Vec::new(),
            active: 0,
            line_numbers: true,
        }
    }

    pub fn active(&self) -> Option<&CodeTab> {
        self.tabs.get(self.active).filter(|_| !self.tabs.is_empty())
    }

    pub fn active_mut(&mut self) -> Option<&mut CodeTab> {
        self.tabs.get_mut(self.active)
    }

    /// Show (or refresh) the compiler output for the current target.
    pub fn set_generated(&mut self, target: Target, stem: &str, text: &str) {
        let ext = target.source_extension();
        let title = format!("{stem}.{ext}");
        if let Some(tab) = self
            .tabs
            .iter_mut()
            .find(|t| t.generated && t.title == title)
        {
            if !tab.dirty {
                tab.text = text.to_string();
            }
            tab.syntax = Syntax::from_target(target);
            return;
        }
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.generated) {
            if !tab.dirty {
                tab.title = title;
                tab.text = text.to_string();
                tab.syntax = Syntax::from_target(target);
                return;
            }
        }
        self.tabs.push(CodeTab {
            path: None,
            title,
            text: text.to_string(),
            dirty: false,
            syntax: Syntax::from_target(target),
            generated: true,
        });
        self.active = self.tabs.len() - 1;
    }

    pub fn open(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        if let Some(idx) = self
            .tabs
            .iter()
            .position(|t| t.path.as_deref() == Some(path))
        {
            self.active = idx;
            return Ok(());
        }
        let title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        self.tabs.push(CodeTab {
            path: Some(path.to_path_buf()),
            title,
            text,
            dirty: false,
            syntax: Syntax::from_path(path),
            generated: false,
        });
        self.active = self.tabs.len() - 1;
        Ok(())
    }

    pub fn save_active(&mut self, path: Option<PathBuf>) -> Result<PathBuf, String> {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return Err("no open file".to_string());
        };
        let path = path
            .or_else(|| tab.path.clone())
            .ok_or_else(|| "no path: use Save as".to_string())?;
        std::fs::write(&path, &tab.text)
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        tab.path = Some(path.clone());
        tab.title = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        tab.syntax = Syntax::from_path(&path);
        tab.dirty = false;
        tab.generated = false;
        Ok(path)
    }

    pub fn close(&mut self, idx: usize) {
        if idx >= self.tabs.len() {
            return;
        }
        self.tabs.remove(idx);
        if self.active >= self.tabs.len() {
            self.active = self.tabs.len().saturating_sub(1);
        }
    }
}

// ---------------------------------------------------------------- highlighting

const DEFAULT: Color32 = Color32::from_rgb(212, 212, 212);
const COMMENT: Color32 = Color32::from_rgb(106, 153, 85);
const STRING: Color32 = Color32::from_rgb(206, 145, 120);
const NUMBER: Color32 = Color32::from_rgb(181, 206, 168);
const KEYWORD: Color32 = Color32::from_rgb(86, 156, 214);
const TYPE: Color32 = Color32::from_rgb(78, 201, 176);
const REGISTER: Color32 = Color32::from_rgb(156, 220, 254);
const PREPROC: Color32 = Color32::from_rgb(197, 134, 192);
const LABEL: Color32 = Color32::from_rgb(220, 220, 170);
const OPCODE: Color32 = Color32::from_rgb(86, 156, 214);

const C_KEYWORDS: &[&str] = &[
    "if", "else", "for", "while", "do", "switch", "case", "default", "break", "continue", "return",
    "goto", "sizeof", "typedef", "struct", "union", "enum", "static", "const", "volatile",
    "extern", "register", "inline", "unsigned", "signed", "void", "int", "char", "short", "long",
    "float", "double", "auto", "restrict",
];

const CPP_KEYWORDS: &[&str] = &[
    "class",
    "namespace",
    "template",
    "typename",
    "public",
    "private",
    "protected",
    "virtual",
    "override",
    "new",
    "delete",
    "this",
    "nullptr",
    "true",
    "false",
    "using",
    "try",
    "catch",
    "throw",
    "constexpr",
    "noexcept",
    "static_cast",
    "reinterpret_cast",
    "const_cast",
    "dynamic_cast",
];

const CS_KEYWORDS: &[&str] = &[
    "using",
    "namespace",
    "class",
    "struct",
    "interface",
    "static",
    "void",
    "int",
    "uint",
    "long",
    "ulong",
    "byte",
    "bool",
    "string",
    "var",
    "new",
    "return",
    "if",
    "else",
    "for",
    "while",
    "do",
    "switch",
    "case",
    "default",
    "break",
    "continue",
    "goto",
    "public",
    "private",
    "protected",
    "internal",
    "readonly",
    "const",
    "true",
    "false",
    "null",
    "this",
];

const ASM_REGISTERS: &[&str] = &[
    "r0", "r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8", "r9", "r10", "r11", "r12", "r13", "r14",
    "r15", "r16", "r17", "r18", "r19", "r20", "r21", "r22", "r23", "r24", "r25", "r26", "r27",
    "r28", "r29", "r30", "r31", "sp", "lr", "pc", "fp", "rax", "rbx", "rcx", "rdx", "rsi", "rdi",
    "rbp", "rsp", "rip", "x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9", "x10", "x11",
    "x12", "x13", "x14", "x15", "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27",
    "x28", "w0", "w1", "w2", "w3", "w4", "w5", "w6", "w7", "w8", "w9", "w10", "w11", "w12", "w13",
    "w14", "w15", "xzr", "wzr",
];

const ASM_OPCODES: &[&str] = &[
    "mov", "movs", "movw", "ldr", "str", "ldrb", "strb", "ldrh", "strh", "ldrsb", "ldrsh", "add",
    "adds", "sub", "subs", "mul", "div", "udiv", "sdiv", "and", "ands", "orr", "eor", "bic", "lsl",
    "lsls", "lsr", "lsrs", "asr", "asrs", "cmp", "cmn", "tst", "teq", "b", "bl", "bx", "blx",
    "beq", "bne", "bhs", "blo", "bhi", "bls", "bge", "blt", "bgt", "ble", "bcs", "bcc", "bmi",
    "bpl", "bvs", "bvc", "push", "pop", "nop", "ret", "svc", "adc", "sbc", "adr", "ldm", "stm",
    "cpsid", "cpsie", "dsb", "isb", "dmb", "cbnz", "cbz", "sxtb", "uxtb", "sxth", "uxth", "rev",
    "clz", "ite", "itt", "ittt", "wfi", "wfe", "sev", "ldmia", "stmia", "ldrd", "strd", "sbi",
    "cbi", "in", "out", "rjmp", "jmp", "call", "rcall", "lpm", "sts", "lds", "sei", "cli",
];

fn format(color: Color32, font: FontId) -> TextFormat {
    TextFormat {
        font_id: font,
        color,
        ..Default::default()
    }
}

/// Layouter used by `TextEdit`, so editing stays responsive and highlighted.
pub fn highlight(
    ui: &egui::Ui,
    text: &dyn egui::TextBuffer,
    wrap_width: f32,
    syntax: Syntax,
) -> std::sync::Arc<egui::Galley> {
    let font = FontId::monospace(14.0);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    let src = text.as_str();
    match syntax {
        Syntax::Plain => {
            job.append(src, 0.0, format(DEFAULT, font));
        }
        Syntax::Asm => highlight_asm(src, &font, &mut job),
        Syntax::C | Syntax::Cpp | Syntax::CSharp => highlight_c_like(src, &font, &mut job, syntax),
    }
    ui.ctx().fonts_mut(|f| f.layout_job(job))
}

fn highlight_asm(src: &str, font: &FontId, job: &mut LayoutJob) {
    let mut first_word_on_line = true;
    for line in src.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let newline = if line.ends_with('\n') { "\n" } else { "" };
        let trimmed = body.trim_start();
        let indent_len = body.len() - trimmed.len();
        if indent_len > 0 {
            job.append(&body[..indent_len], 0.0, format(DEFAULT, font.clone()));
        }

        // Whole-line comment: ; @ // and # pseudo-ops / directives.
        let comment_start =
            if trimmed.starts_with(';') || trimmed.starts_with('@') || trimmed.starts_with("//") {
                Some(0)
            } else if trimmed.starts_with('#') || trimmed.starts_with('.') {
                // Directives are highlighted as keywords, not comments.
                None
            } else {
                find_asm_comment(trimmed)
            };

        if let Some(cut) = comment_start {
            let (code, comment) = trimmed.split_at(cut);
            highlight_asm_code(code, font, job, first_word_on_line);
            job.append(comment, 0.0, format(COMMENT, font.clone()));
        } else if trimmed.starts_with('#') || trimmed.starts_with('.') {
            job.append(trimmed, 0.0, format(PREPROC, font.clone()));
        } else {
            highlight_asm_code(trimmed, font, job, first_word_on_line);
        }
        job.append(newline, 0.0, format(DEFAULT, font.clone()));
        first_word_on_line = true;
    }
}

fn find_asm_comment(s: &str) -> Option<usize> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut in_string = false;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_string = !in_string,
            b';' | b'@' if !in_string => return Some(i),
            b'/' if !in_string && i + 1 < bytes.len() && bytes[i + 1] == b'/' => return Some(i),
            _ => {}
        }
        i += 1;
    }
    None
}

fn highlight_asm_code(code: &str, font: &FontId, job: &mut LayoutJob, mut first: bool) {
    for token in tokens(code) {
        let color = match token.kind {
            TokKind::Ident => {
                let lower = token.text.to_ascii_lowercase();
                if ASM_REGISTERS.contains(&lower.as_str()) {
                    REGISTER
                } else if ASM_OPCODES.contains(&lower.as_str()) && first {
                    first = false;
                    OPCODE
                } else if token.text.ends_with(':') {
                    LABEL
                } else {
                    DEFAULT
                }
            }
            TokKind::Number => NUMBER,
            TokKind::String => STRING,
            TokKind::Space => DEFAULT,
            TokKind::Punct => DEFAULT,
        };
        job.append(token.text, 0.0, format(color, font.clone()));
    }
}

fn highlight_c_like(src: &str, font: &FontId, job: &mut LayoutJob, syntax: Syntax) {
    for line in src.split_inclusive('\n') {
        let body = line.strip_suffix('\n').unwrap_or(line);
        let newline = if line.ends_with('\n') { "\n" } else { "" };
        let trimmed = body.trim_start();
        let indent_len = body.len() - trimmed.len();
        if indent_len > 0 {
            job.append(&body[..indent_len], 0.0, format(DEFAULT, font.clone()));
        }
        if trimmed.starts_with('#') {
            job.append(trimmed, 0.0, format(PREPROC, font.clone()));
        } else if trimmed.starts_with("//") {
            job.append(trimmed, 0.0, format(COMMENT, font.clone()));
        } else {
            let mut in_string = false;
            let mut i = 0;
            let mut last = 0;
            let bytes = trimmed.as_bytes();
            while i < bytes.len() {
                match bytes[i] {
                    b'"' => in_string = !in_string,
                    b'/' if !in_string && i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                        highlight_c_code(&trimmed[last..i], font, job, syntax);
                        job.append(&trimmed[i..], 0.0, format(COMMENT, font.clone()));
                        last = trimmed.len();
                        break;
                    }
                    _ => {}
                }
                i += 1;
            }
            if last < trimmed.len() {
                highlight_c_code(&trimmed[last..], font, job, syntax);
            }
        }
        job.append(newline, 0.0, format(DEFAULT, font.clone()));
    }
}

fn highlight_c_code(code: &str, font: &FontId, job: &mut LayoutJob, syntax: Syntax) {
    let mut previous_is_dot = false;
    for token in tokens(code) {
        let color = match token.kind {
            TokKind::Ident => {
                let is_type = matches!(
                    token.text,
                    "uint8_t"
                        | "uint16_t"
                        | "uint32_t"
                        | "uint64_t"
                        | "int8_t"
                        | "int16_t"
                        | "int32_t"
                        | "int64_t"
                        | "size_t"
                        | "ulong"
                        | "uint"
                );
                if is_type {
                    TYPE
                } else if token.text.ends_with(':') {
                    LABEL
                } else if previous_is_dot {
                    DEFAULT
                } else if C_KEYWORDS.contains(&token.text)
                    || (syntax == Syntax::Cpp && CPP_KEYWORDS.contains(&token.text))
                    || (syntax == Syntax::CSharp && CS_KEYWORDS.contains(&token.text))
                {
                    KEYWORD
                } else if is_c_function(token.text) {
                    LABEL
                } else {
                    DEFAULT
                }
            }
            TokKind::Number => NUMBER,
            TokKind::String => STRING,
            TokKind::Space => DEFAULT,
            TokKind::Punct => DEFAULT,
        };
        previous_is_dot = token.text == ".";
        job.append(token.text, 0.0, format(color, font.clone()));
    }
}

fn is_c_function(name: &str) -> bool {
    matches!(
        name,
        "printf"
            | "main"
            | "pinMode"
            | "digitalWrite"
            | "digitalRead"
            | "delay"
            | "delayMicroseconds"
            | "analogRead"
            | "Serial"
            | "begin"
            | "print"
            | "println"
            | "write"
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TokKind {
    Ident,
    Number,
    String,
    Space,
    Punct,
}

struct Tok<'a> {
    text: &'a str,
    kind: TokKind,
}

/// A tiny lexer that keeps every byte of the input, so highlighting never
/// changes the text.
fn tokens(src: &str) -> Vec<Tok<'_>> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let start = i;
        let c = bytes[i] as char;
        if c.is_whitespace() {
            while i < bytes.len() && (bytes[i] as char).is_whitespace() {
                i += 1;
            }
            out.push(Tok {
                text: &src[start..i],
                kind: TokKind::Space,
            });
        } else if c == '"' || c == '\'' {
            let quote = bytes[i];
            i += 1;
            while i < bytes.len() {
                if bytes[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if bytes[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(Tok {
                text: &src[start..i.min(src.len())],
                kind: TokKind::String,
            });
        } else if c.is_ascii_digit()
            || (c == '0'
                && i + 1 < bytes.len()
                && matches!(bytes[i + 1], b'x' | b'X' | b'b' | b'B'))
        {
            while i < bytes.len()
                && (bytes[i].is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || bytes[i] == b'.'
                    || ((bytes[i] == b'+' || bytes[i] == b'-')
                        && i > start
                        && matches!(bytes[i - 1], b'e' | b'E' | b'p' | b'P')))
            {
                i += 1;
            }
            out.push(Tok {
                text: &src[start..i],
                kind: TokKind::Number,
            });
        } else if c.is_ascii_alphabetic() || c == '_' || c == '.' || c == '%' {
            while i < bytes.len()
                && ((bytes[i] as char).is_ascii_alphanumeric()
                    || bytes[i] == b'_'
                    || bytes[i] == b'.'
                    || bytes[i] == b'%')
            {
                i += 1;
            }
            // A trailing ':' makes an asm label part of the token.
            if i < bytes.len() && bytes[i] == b':' {
                i += 1;
            }
            out.push(Tok {
                text: &src[start..i],
                kind: TokKind::Ident,
            });
        } else {
            i += 1;
            out.push(Tok {
                text: &src[start..i],
                kind: TokKind::Punct,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------- UI

fn default_name(t: Target, stem: &str) -> String {
    format!("{stem}.{}", t.source_extension())
}

impl WireStudio {
    pub fn code_open(&mut self) {
        let filters: Vec<(&str, &[&str])> = vec![
            (
                "Source",
                &["c", "h", "cpp", "hpp", "cc", "cs", "ino", "s", "asm"],
            ),
            ("All files", &["*"]),
        ];
        if let Some(path) = self.pick_open(self.tr("code.open"), &filters) {
            match self.editor.open(&path) {
                Ok(()) => {
                    self.status = self
                        .tr("status.code_opened")
                        .replace("{}", &path.display().to_string());
                    self.console.push(self.status.clone());
                }
                Err(e) => self.error(e),
            }
        }
    }

    pub fn code_save(&mut self, save_as: bool) {
        let Some(tab) = self.editor.active() else {
            self.status = self.tr("code.no_file").to_string();
            return;
        };
        let path = if save_as || tab.path.is_none() {
            let target = Target::parse(&self.graph.target).unwrap_or(Target::Aarch64Macos);
            let stem = artifact_stem(&self.graph);
            let name = if tab.path.is_none() {
                default_name(target, &stem)
            } else {
                tab.title.clone()
            };
            let filters: Vec<(&str, &[&str])> = vec![
                (
                    "Source",
                    &["c", "h", "cpp", "hpp", "cc", "cs", "ino", "s", "asm"],
                ),
                ("All files", &["*"]),
            ];
            match self.pick_save(self.tr("code.save_as"), &filters, &name) {
                Some(p) => Some(p),
                None => {
                    self.status = self.tr("status.cancelled").to_string();
                    return;
                }
            }
        } else {
            None
        };
        match self.editor.save_active(path) {
            Ok(path) => {
                self.remember_dir(&path);
                self.status = self
                    .tr("status.saved")
                    .replace("{}", &path.display().to_string());
                self.console.push(self.status.clone());
                self.refresh_generated_tab();
            }
            Err(e) => self.error(e),
        }
    }

    pub fn code_new(&mut self, syntax: Syntax) {
        self.editor.tabs.push(CodeTab::untitled(syntax));
        self.editor.active = self.editor.tabs.len() - 1;
        self.workspace = Workspace::Code;
    }

    /// Keep the generated tab in sync with the current graph compile.
    pub fn refresh_generated_tab(&mut self) {
        let Some(target) = Target::parse(&self.graph.target) else {
            return;
        };
        if self.out.asm.is_empty() {
            return;
        }
        let stem = artifact_stem(&self.graph);
        self.editor.set_generated(target, &stem, &self.out.asm);
    }

    pub fn code_build_run(&mut self, ctx: &egui::Context, run: bool) {
        let Some(tab) = self.editor.active() else {
            self.status = self.tr("code.no_file").to_string();
            return;
        };
        let Some(target) = Target::parse(&self.graph.target) else {
            self.error(self.tr("status.unknown_target"));
            return;
        };
        let text = tab.text.clone();
        let stem = artifact_stem(&self.graph);
        let out_dir = self.firmware_dir();
        let transport = self
            .settings
            .stm32_transport
            .clone()
            .unwrap_or_else(|| "stlink".to_string());
        let run = run && (target.is_host_asm() || target.is_source());
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = (|| -> Result<String, String> {
                let mut log = String::new();
                if target.is_source() {
                    let build = toolchain::build_source(&text, &out_dir, &stem, target)?;
                    log.push_str(&format!("[source] {}\n", build.source.display()));
                    log.push_str(&format!("[program] {}\n", build.exe.display()));
                    if run {
                        let res = toolchain::run(&build.exe, 5)?;
                        log.push_str(&res.stdout);
                        log.push_str(&res.stderr);
                        log.push_str(&format!(
                            "[exit {}]\n",
                            res.status
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "signal".to_string())
                        ));
                    }
                } else if target.is_avr() {
                    let build = toolchain::build_avr(&text, &out_dir, &stem)?;
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
                } else if target.is_stm32() {
                    let build = toolchain::build_stm32(&text, &out_dir, &stem)?;
                    if !build.size.trim().is_empty() {
                        log.push_str(build.size.trim_end());
                        log.push('\n');
                    }
                    log.push_str(&format!(
                        "[elf] {}\n[hex] {}\n[bin] {}\n[transport] {}\n",
                        build.elf.display(),
                        build.hex.display(),
                        build.bin.display(),
                        transport
                    ));
                } else if target.is_esp() {
                    let build = toolchain::build_esp(&text, &out_dir, &stem, target)?;
                    log.push_str(&format!("[sketch] {}\n", build.ino.display()));
                    log.push_str(&format!("[output] {}\n", build.output_dir.display()));
                } else if target.is_host_asm() {
                    let dir = std::env::temp_dir().join("wirestudio");
                    std::fs::create_dir_all(&dir).map_err(|e| format!("temp dir: {e}"))?;
                    let asm_path = dir.join(format!("{stem}.s"));
                    let exe_path = dir.join(format!("{stem}-{}", std::process::id()));
                    std::fs::write(&asm_path, &text).map_err(|e| format!("write asm: {e}"))?;
                    toolchain::assemble(&asm_path, &exe_path, target)?;
                    log.push_str(&format!("[asm] {}\n", asm_path.display()));
                    if run {
                        let res = toolchain::run(&exe_path, 5)?;
                        log.push_str(&res.stdout);
                        log.push_str(&res.stderr);
                        log.push_str(&format!(
                            "[exit {}]\n",
                            res.status
                                .map(|c| c.to_string())
                                .unwrap_or_else(|| "signal".to_string())
                        ));
                    }
                }
                Ok(log)
            })();
            let _ = tx.send(result);
        });
        self.job = Some(Job {
            rx,
            started: Instant::now(),
            kind: if run { JobKind::Run } else { JobKind::Build },
        });
        self.status = self
            .tr(if run {
                "status.running"
            } else {
                "status.building_fw"
            })
            .to_string();
        self.tab = BottomTab::Console;
        ctx.request_repaint();
    }

    pub fn code_editor_ui(&mut self, ui: &mut egui::Ui) {
        let lang = self.lang;
        let s = |k: &str| crate::i18n::t(lang, k);
        let target = Target::parse(&self.graph.target).unwrap_or(Target::Aarch64Macos);

        // Toolbar.
        ui.horizontal(|ui| {
            if ui.button(s("code.new")).clicked() {
                self.code_new(Syntax::from_target(target));
            }
            if ui.button(s("code.open")).clicked() {
                self.code_open();
            }
            if ui.button(s("code.save")).clicked() {
                self.code_save(false);
            }
            if ui.button(s("code.save_as")).clicked() {
                self.code_save(true);
            }
            ui.separator();
            let can_run = target.is_host_asm() || target.is_source();
            if ui.button(s("code.build")).clicked() {
                self.code_build_run(ui.ctx(), false);
            }
            if can_run && ui.button(s("code.run")).clicked() {
                self.code_build_run(ui.ctx(), true);
            }
            ui.separator();
            ui.checkbox(&mut self.editor.line_numbers, s("code.line_numbers"));
            if let Some(tab) = self.editor.active() {
                ui.label(
                    RichText::new(format!(
                        "{} · {} · {} lines",
                        tab.syntax.label(),
                        tab.title,
                        tab.text.lines().count().max(1)
                    ))
                    .small()
                    .weak(),
                );
            }
        });
        ui.separator();

        // Tab strip.
        if !self.editor.tabs.is_empty() {
            let mut close: Option<usize> = None;
            let mut activate: Option<usize> = None;
            ui.horizontal_wrapped(|ui| {
                for (i, tab) in self.editor.tabs.iter().enumerate() {
                    let mut title = tab.title.clone();
                    if tab.dirty {
                        title.push('*');
                    }
                    let selected = i == self.editor.active;
                    if ui
                        .selectable_label(selected, RichText::new(title).monospace())
                        .clicked()
                    {
                        activate = Some(i);
                    }
                    if ui.small_button("×").clicked() {
                        close = Some(i);
                    }
                    ui.add_space(2.0);
                }
            });
            if let Some(i) = activate {
                self.editor.active = i;
            }
            if let Some(i) = close {
                self.editor.close(i);
            }
            ui.separator();
        }

        if self.editor.active().is_none() {
            ui.add_space(24.0);
            ui.label(RichText::new(s("code.empty")).weak());
            return;
        }

        let syntax = self
            .editor
            .active()
            .map(|t| t.syntax)
            .unwrap_or(Syntax::Plain);
        let line_numbers = self.editor.line_numbers;
        let mut text = self
            .editor
            .active()
            .map(|t| t.text.clone())
            .unwrap_or_default();
        let lines = text.lines().count().max(1);
        let gutter_width = (lines.to_string().len() as f32 * 8.0 + 14.0).max(38.0);

        let mut changed = false;
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    if line_numbers {
                        let numbers: String = (1..=lines).map(|n| format!("{n}\n")).collect();
                        ui.add_sized(
                            [gutter_width, 0.0],
                            egui::Label::new(
                                RichText::new(numbers)
                                    .monospace()
                                    .color(Color32::from_gray(120)),
                            )
                            .selectable(false),
                        );
                    }
                    let mut layouter =
                        |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
                            highlight(ui, buf, wrap_width, syntax)
                        };
                    let resp = ui.add(
                        egui::TextEdit::multiline(&mut text)
                            .font(egui::TextStyle::Monospace)
                            .code_editor()
                            .desired_width(1200.0)
                            .desired_rows(lines)
                            .lock_focus(true)
                            .layouter(&mut layouter),
                    );
                    changed = resp.changed();
                });
            });
        if changed {
            if let Some(tab) = self.editor.active_mut() {
                tab.text = text;
                tab.dirty = true;
            }
        }
    }
}
