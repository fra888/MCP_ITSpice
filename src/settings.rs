use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppSettings {
    pub ngspice_executable: String,
    pub python_executable: String,
    pub ngspice_default_dirs: Vec<String>,
    pub user_model_dirs: Vec<String>,
}

pub fn user_home_dir() -> Option<PathBuf> {
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return Some(PathBuf::from(home));
        }
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        if !profile.trim().is_empty() {
            return Some(PathBuf::from(profile));
        }
    }
    if let (Ok(drive), Ok(path)) = (std::env::var("HOMEDRIVE"), std::env::var("HOMEPATH")) {
        let combined = format!("{drive}{path}");
        if !combined.trim().is_empty() {
            return Some(PathBuf::from(combined));
        }
    }
    None
}

pub fn find_in_path(exe_name: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(exe_name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

pub fn detect_python_path() -> String {
    #[cfg(windows)]
    {
        for exe in ["python.exe", "python3.exe", "py.exe"] {
            if let Some(found) = find_in_path(exe) {
                return found.display().to_string();
            }
        }
        if let Ok(local_app) = std::env::var("LOCALAPPDATA") {
            let py_root = PathBuf::from(local_app).join("Programs").join("Python");
            if let Ok(entries) = std::fs::read_dir(&py_root) {
                let mut dirs: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok().map(|e| e.path()))
                    .filter(|p| p.is_dir())
                    .collect();
                dirs.sort();
                for dir in dirs.into_iter().rev() {
                    let cand = dir.join("python.exe");
                    if cand.is_file() {
                        return cand.display().to_string();
                    }
                }
            }
        }
        return "python".to_string();
    }
    #[cfg(not(windows))]
    {
        for candidate in ["/usr/bin/python3", "/usr/local/bin/python3"] {
            if Path::new(candidate).is_file() {
                return candidate.to_string();
            }
        }
        if let Some(found) = find_in_path("python3") {
            return found.display().to_string();
        }
        "python3".to_string()
    }
}

pub fn detect_ngspice_path() -> String {
    #[cfg(windows)]
    {
        for exe in ["ngspice_con.exe", "ngspice.exe"] {
            if let Some(found) = find_in_path(exe) {
                return found.display().to_string();
            }
        }
        for candidate in [
            r"C:\Spice64\bin\ngspice_con.exe",
            r"C:\Spice64\bin\ngspice.exe",
            r"C:\Program Files\ngspice\bin\ngspice_con.exe",
            r"C:\Program Files\ngspice\bin\ngspice.exe",
            r"C:\msys64\ucrt64\bin\ngspice.exe",
            r"C:\msys64\mingw64\bin\ngspice.exe",
            r"C:\Spice64_dll\dll-vs\ngspice.dll",
            r"C:\Spice64\bin\ngspice.dll",
        ] {
            if Path::new(candidate).is_file() {
                return candidate.to_string();
            }
        }
        return "ngspice_con.exe".to_string();
    }
    #[cfg(not(windows))]
    {
        for candidate in ["/usr/bin/ngspice", "/usr/local/bin/ngspice"] {
            if Path::new(candidate).is_file() {
                return candidate.to_string();
            }
        }
        if let Some(found) = find_in_path("ngspice") {
            return found.display().to_string();
        }
        for lib_candidate in [
            "/lib/x86_64-linux-gnu/libngspice.so.0",
            "/usr/lib/x86_64-linux-gnu/libngspice.so.0",
            "/usr/local/lib/libngspice.dylib",
            "/opt/homebrew/lib/libngspice.dylib",
        ] {
            if Path::new(lib_candidate).is_file() {
                return lib_candidate.to_string();
            }
        }
        "ngspice".to_string()
    }
}

impl Default for AppSettings {
    fn default() -> Self {
        let mut ngspice_default_dirs = Vec::new();
        for env_var in ["NGSPICE_LIB_DIR", "SPICE_LIB_DIR"] {
            if let Ok(val) = std::env::var(env_var) {
                if !val.trim().is_empty() {
                    ngspice_default_dirs.push(val);
                }
            }
        }
        #[cfg(windows)]
        let platform_dirs: &[&str] = &[
            r"C:\Spice64\share\ngspice",
            r"C:\Spice64\share\ngspice\scripts",
            r"C:\Program Files\ngspice\share\ngspice",
            r"C:\msys64\ucrt64\share\ngspice",
            r"C:\msys64\mingw64\share\ngspice",
        ];
        #[cfg(not(windows))]
        let platform_dirs: &[&str] = &[
            "/usr/share/ngspice",
            "/usr/share/ngspice/scripts",
            "/usr/local/share/ngspice",
            "/usr/lib/x86_64-linux-gnu/ngspice",
        ];
        for p in platform_dirs {
            if !ngspice_default_dirs.iter().any(|x| x == p) {
                ngspice_default_dirs.push((*p).to_string());
            }
        }

        let mut user_model_dirs = vec!["./models".to_string()];
        if let Some(home) = user_home_dir() {
            user_model_dirs.push(
                home.join("MCP_ITSpice")
                    .join("models")
                    .display()
                    .to_string(),
            );
        }

        Self {
            ngspice_executable: detect_ngspice_path(),
            python_executable: detect_python_path(),
            ngspice_default_dirs,
            user_model_dirs,
        }
    }
}

impl AppSettings {
    pub fn config_path() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            if let Ok(appdata) = std::env::var("APPDATA") {
                if !appdata.trim().is_empty() {
                    return Some(PathBuf::from(appdata).join("MCP_ITSpice").join("settings.json"));
                }
            }
        }
        let home = user_home_dir()?;
        Some(home.join(".config").join("mcp_itspice").join("settings.json"))
    }

    pub fn load() -> Self {
        if let Some(path) = Self::config_path() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(parsed) = serde_json::from_str::<Self>(&content) {
                    return parsed;
                }
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let Some(path) = Self::config_path() else {
            return Err("User home/config directory environment variable not set".into());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create config dir: {e}"))?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize settings: {e}"))?;
        std::fs::write(&path, json).map_err(|e| format!("Failed to write settings: {e}"))
    }

    /// Returns all model search directories (user directories first so user overrides take priority).
    pub fn all_model_dirs(&self) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let mut seen = HashSet::new();
        for dir in self
            .user_model_dirs
            .iter()
            .chain(self.ngspice_default_dirs.iter())
        {
            let trimmed = dir.trim();
            if !trimmed.is_empty() && seen.insert(trimmed.to_string()) {
                out.push(PathBuf::from(trimmed));
            }
        }
        out
    }
}

#[derive(Debug, Clone, Default)]
pub struct ModelIndex {
    pub models: HashMap<String, PathBuf>,
    pub model_kinds: HashMap<String, String>,
    pub subckts: HashMap<String, PathBuf>,
    pub original_names: HashMap<String, String>,
    pub scanned_files: usize,
}

impl ModelIndex {
    pub fn scan(dirs: &[PathBuf]) -> Self {
        let mut index = Self::default();
        for dir in dirs {
            if dir.is_dir() {
                index.scan_dir(dir, 0);
            }
        }
        index
    }

    fn scan_dir(&mut self, dir: &Path, depth: usize) {
        if depth > 3 {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                self.scan_dir(&path, depth + 1);
            } else if is_spice_model_file(&path) {
                self.scan_file(&path);
            }
        }
    }

    fn scan_file(&mut self, path: &Path) {
        let Ok(content) = std::fs::read_to_string(path) else {
            return;
        };
        self.scanned_files += 1;
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('*') || trimmed.starts_with(';') {
                continue;
            }
            let mut parts = trimmed.split_whitespace();
            let Some(kw) = parts.next() else {
                continue;
            };
            let kw_low = kw.to_ascii_lowercase();
            if kw_low == ".model" {
                if let Some(name) = parts.next() {
                    let key = name.to_ascii_lowercase();
                    self.models.entry(key.clone()).or_insert_with(|| path.to_path_buf());
                    self.original_names
                        .entry(key.clone())
                        .or_insert_with(|| name.to_string());
                    if let Some(kind_tok) = parts.next() {
                        let kind_clean = kind_tok
                            .split('(')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .to_ascii_uppercase();
                        if !kind_clean.is_empty() {
                            self.model_kinds.entry(key).or_insert(kind_clean);
                        }
                    }
                }
            } else if kw_low == ".subckt" {
                if let Some(name) = parts.next() {
                    let key = name.to_ascii_lowercase();
                    self.subckts
                        .entry(key.clone())
                        .or_insert_with(|| path.to_path_buf());
                    self.original_names
                        .entry(key)
                        .or_insert_with(|| name.to_string());
                }
            }
        }
    }

    pub fn find_model_file(&self, name: &str) -> Option<&PathBuf> {
        let key = name.trim().to_ascii_lowercase();
        self.models.get(&key).or_else(|| self.subckts.get(&key))
    }

    /// Returns `(display_name, kind, file_path)` for all `.model` entries matching any of `kinds`.
    pub fn list_models_for_kinds(&self, kinds: &[&str]) -> Vec<(String, String, PathBuf)> {
        let mut out = Vec::new();
        for (key, path) in &self.models {
            let k = self
                .model_kinds
                .get(key)
                .map(|s| s.as_str())
                .unwrap_or("");
            if kinds.iter().any(|want| k.eq_ignore_ascii_case(want)) {
                let name = self
                    .original_names
                    .get(key)
                    .cloned()
                    .unwrap_or_else(|| key.clone());
                out.push((name, k.to_string(), path.clone()));
            }
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }

    /// Returns `(display_name, file_path)` for all indexed `.subckt` entries.
    pub fn list_subckts(&self) -> Vec<(String, PathBuf)> {
        let mut out = Vec::new();
        for (key, path) in &self.subckts {
            let name = self
                .original_names
                .get(key)
                .cloned()
                .unwrap_or_else(|| key.clone());
            out.push((name, path.clone()));
        }
        out.sort_by(|a, b| a.0.cmp(&b.0));
        out
    }
}

fn is_spice_model_file(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "lib" | "mod" | "mdl" | "sub" | "cir" | "sp" | "inc" | "txt"
    )
}
