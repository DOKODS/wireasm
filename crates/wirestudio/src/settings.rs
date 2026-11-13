/// Small settings file: language override, last used directory, export folder,
/// and the parameters of the interface.
///
/// Stored as JSON next to the platform's usual config location.
use std::path::PathBuf;

use wireasm_core::json::Json;

/// The look of the interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
}

impl Theme {
    pub fn key(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }

    pub fn from_key(key: &str) -> Option<Theme> {
        match key.trim().to_ascii_lowercase().as_str() {
            "dark" => Some(Theme::Dark),
            "light" => Some(Theme::Light),
            _ => None,
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            Theme::Dark => "settings.theme_dark",
            Theme::Light => "settings.theme_light",
        }
    }
}

/// How the schematic sheet is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetStyle {
    /// The Multisim look: a light sheet, red wires, blue labels.
    Multisim,
    /// The dark look the editor had before.
    Dark,
}

impl SheetStyle {
    pub fn key(self) -> &'static str {
        match self {
            SheetStyle::Multisim => "multisim",
            SheetStyle::Dark => "dark",
        }
    }

    pub fn from_key(key: &str) -> Option<SheetStyle> {
        match key.trim().to_ascii_lowercase().as_str() {
            "multisim" => Some(SheetStyle::Multisim),
            "dark" => Some(SheetStyle::Dark),
            _ => None,
        }
    }

    pub fn label_key(self) -> &'static str {
        match self {
            SheetStyle::Multisim => "settings.style_multisim",
            SheetStyle::Dark => "settings.style_dark",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Settings {
    /// `None` means "follow the system locale".
    pub lang: Option<String>,
    pub last_dir: Option<String>,
    pub out_dir: Option<String>,
    /// Serial port used for AVR and ESP uploads, board for AVR.
    pub avr_port: Option<String>,
    pub avr_board: Option<String>,
    /// STM32 upload transport: `stlink` (default) or `dfu`.
    pub stm32_transport: Option<String>,
    /// Theme of the interface.
    pub theme: Option<String>,
    /// Look of the schematic sheet.
    pub sheet_style: Option<String>,
    /// Size of the interface font, in points.
    pub ui_scale: Option<f64>,
    /// Size of the code font, in points.
    pub code_font_size: Option<f64>,
    /// Show the line numbers in the code editor.
    pub line_numbers: Option<bool>,
    /// Show the grid on the schematic sheet.
    pub show_grid: Option<bool>,
    /// Show the node numbers on the schematic sheet.
    pub show_node_numbers: Option<bool>,
    /// Show the instrument panels right after a run.
    pub auto_open_panels: Option<bool>,
    /// Step of the transient analysis, in seconds.
    pub sim_dt: Option<f64>,
    /// Length of the transient analysis, in seconds.
    pub sim_stop: Option<f64>,
}

fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|h| PathBuf::from(h).join("Library/Application Support/WireStudio"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|d| PathBuf::from(d).join("WireStudio"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
            .map(|d| d.join("wirestudio"))
    }
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("settings.json"))
}

impl Settings {
    pub fn load() -> Settings {
        let Some(path) = config_path() else {
            return Settings::default();
        };
        let Ok(src) = std::fs::read_to_string(&path) else {
            return Settings::default();
        };
        let Ok(v) = Json::parse(&src) else {
            return Settings::default();
        };
        let s = |key: &str| {
            v.get(key)
                .and_then(|x| x.as_str())
                .map(|x| x.to_string())
                .filter(|x| !x.is_empty())
        };
        Settings {
            lang: s("lang"),
            last_dir: s("lastDir"),
            out_dir: s("outDir"),
            avr_port: s("avrPort"),
            avr_board: s("avrBoard"),
            stm32_transport: s("stm32Transport"),
            theme: s("theme"),
            sheet_style: s("sheetStyle"),
            ui_scale: v.get("uiScale").and_then(|x| x.as_f64()),
            code_font_size: v.get("codeFontSize").and_then(|x| x.as_f64()),
            line_numbers: v.get("lineNumbers").and_then(|x| x.as_bool()),
            show_grid: v.get("showGrid").and_then(|x| x.as_bool()),
            show_node_numbers: v.get("showNodeNumbers").and_then(|x| x.as_bool()),
            auto_open_panels: v.get("autoOpenPanels").and_then(|x| x.as_bool()),
            sim_dt: v.get("simDt").and_then(|x| x.as_f64()),
            sim_stop: v.get("simStop").and_then(|x| x.as_f64()),
        }
    }

    pub fn save(&self) {
        let Some(path) = config_path() else { return };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let mut m = std::collections::BTreeMap::new();
        if let Some(lang) = &self.lang {
            m.insert("lang".to_string(), Json::Str(lang.clone()));
        }
        if let Some(dir) = &self.last_dir {
            m.insert("lastDir".to_string(), Json::Str(dir.clone()));
        }
        if let Some(dir) = &self.out_dir {
            m.insert("outDir".to_string(), Json::Str(dir.clone()));
        }
        if let Some(port) = &self.avr_port {
            m.insert("avrPort".to_string(), Json::Str(port.clone()));
        }
        if let Some(board) = &self.avr_board {
            m.insert("avrBoard".to_string(), Json::Str(board.clone()));
        }
        if let Some(transport) = &self.stm32_transport {
            m.insert("stm32Transport".to_string(), Json::Str(transport.clone()));
        }
        if let Some(theme) = &self.theme {
            m.insert("theme".to_string(), Json::Str(theme.clone()));
        }
        if let Some(style) = &self.sheet_style {
            m.insert("sheetStyle".to_string(), Json::Str(style.clone()));
        }
        if let Some(scale) = self.ui_scale {
            m.insert("uiScale".to_string(), Json::Num(scale));
        }
        if let Some(size) = self.code_font_size {
            m.insert("codeFontSize".to_string(), Json::Num(size));
        }
        if let Some(flag) = self.line_numbers {
            m.insert("lineNumbers".to_string(), Json::Bool(flag));
        }
        if let Some(flag) = self.show_grid {
            m.insert("showGrid".to_string(), Json::Bool(flag));
        }
        if let Some(flag) = self.show_node_numbers {
            m.insert("showNodeNumbers".to_string(), Json::Bool(flag));
        }
        if let Some(flag) = self.auto_open_panels {
            m.insert("autoOpenPanels".to_string(), Json::Bool(flag));
        }
        if let Some(dt) = self.sim_dt {
            m.insert("simDt".to_string(), Json::Num(dt));
        }
        if let Some(stop) = self.sim_stop {
            m.insert("simStop".to_string(), Json::Num(stop));
        }
        let _ = std::fs::write(path, Json::Obj(m).stringify_pretty());
    }
}
