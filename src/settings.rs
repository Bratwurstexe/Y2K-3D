//! Portable settings: everything is kept in a folder beside the program, nothing in the registry
//! or the user profile.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default)]
    pub last_folder: Option<String>,
    #[serde(default)]
    pub library_root: Option<String>,
    /// "de" or "en".
    #[serde(default = "default_language")]
    pub language: String,
}

pub fn default_language() -> String {
    "en".into()
}

pub fn default_model() -> String {
    "sonnet".into()
}

pub const MODELS: [(&str, &str); 3] = [
    ("sonnet", "Sonnet (ausgewogen)"),
    ("opus", "Opus (am stärksten)"),
    ("haiku", "Haiku (am schnellsten)"),
];

impl Default for Settings {
    fn default() -> Self {
        Settings {
            model: default_model(),
            last_folder: None,
            library_root: None,
            language: default_language(),
        }
    }
}

pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn data_dir() -> PathBuf {
    exe_dir().join("Daten")
}

fn file() -> PathBuf {
    data_dir().join("einstellungen.json")
}

impl Settings {
    pub fn load() -> Settings {
        std::fs::read_to_string(file())
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let _ = std::fs::create_dir_all(data_dir());
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(file(), s);
        }
    }

    /// Folder with the original CD content (Terrain Maps, Picture Textures, OBJ Models...).
    /// By default the folder above the program (the program lives in a subfolder of it).
    pub fn library_root(&self) -> Option<PathBuf> {
        if let Some(r) = &self.library_root {
            let p = PathBuf::from(r);
            if p.is_dir() {
                return Some(p);
            }
        }
        let mut d = exe_dir();
        for _ in 0..3 {
            if d.join("Terrain  Maps").is_dir()
                || d.join("Picture Textures").is_dir()
                || d.join("OBJ Models").is_dir()
            {
                return Some(d);
            }
            if !d.pop() {
                break;
            }
        }
        std::env::var_os("Y2K3D_LIBRARY")
            .map(PathBuf::from)
            .filter(|p| p.is_dir())
    }
}

/// Picture files (TIF/PNG/JPG/BMP) in a library subfolder.
pub fn library_files(root: Option<PathBuf>, sub: &str, exts: &[&str]) -> Vec<PathBuf> {
    let Some(root) = root else { return Vec::new() };
    let mut v: Vec<PathBuf> = std::fs::read_dir(root.join(sub))
        .map(|rd| {
            rd.filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| {
                    p.extension()
                        .and_then(|e| e.to_str())
                        .map(|e| exts.iter().any(|x| x.eq_ignore_ascii_case(e)))
                        .unwrap_or(false)
                })
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}
