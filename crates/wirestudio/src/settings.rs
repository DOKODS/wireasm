//! Small settings file: language override, last used directory, export folder.
//! Stored as JSON next to the platform's usual config location.

use std::path::PathBuf;

use wireasm_core::json::Json;

#[derive(Clone, Debug, Default)]
pub struct Settings {
    /// `None` means "follow the system locale".
    pub lang: Option<String>,
    pub last_dir: Option<String>,
    pub out_dir: Option<String>,
    /// Serial port and board used for AVR uploads.
    pub avr_port: Option<String>,
    pub avr_board: Option<String>,
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
        let _ = std::fs::write(path, Json::Obj(m).stringify_pretty());
    }
}
