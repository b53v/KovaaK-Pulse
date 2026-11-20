use std::fs;
use std::path::{Path, PathBuf};

use directories::ProjectDirs;

use crate::model::AppConfig;

const KOVAAK_REL_STATS: &str = r"steamapps\common\FPSAimTrainer\FPSAimTrainer\stats";
const KOVAAK_REL_PERF: &str = r"steamapps\common\FPSAimTrainer\FPSAimTrainer\performances";

/// Returns the application data directory (e.g. `%APPDATA%\KovaaK-Pulse\data` or fallback `.`).
pub fn app_data_dir() -> PathBuf {
    if let Some(proj) = ProjectDirs::from("com", "KovaaKPulse", "KovaaK-Pulse") {
        let dir = proj.data_local_dir().to_path_buf();
        let _ = fs::create_dir_all(&dir);
        return dir;
    }
    PathBuf::from(".")
}

/// Returns the config file path (`config.json`).
pub fn config_path() -> PathBuf {
    app_data_dir().join("config.json")
}

/// Returns the default SQLite database path (`kovaak_pulse.db`).
pub fn default_db_path() -> PathBuf {
    app_data_dir().join("kovaak_pulse.db")
}

/// Loads `AppConfig` from disk, returning `AppConfig::default()` if missing or unreadable.
pub fn load_config() -> AppConfig {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => AppConfig::default(),
    }
}

/// Persists `AppConfig` to disk as pretty-printed JSON.
pub fn save_config(cfg: &AppConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create config dir: {e}"))?;
    }
    let json = serde_json::to_string_pretty(cfg)
        .map_err(|e| format!("Failed to serialize config: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("Failed to write config: {e}"))
}

/// Resolves KovaaK's `(stats_dir, performances_dir)` from user overrides or auto-detection.
pub fn resolve_kovaak_dirs(cfg: &AppConfig) -> Option<(PathBuf, PathBuf)> {
    // 1. Check user overrides first
    if let Some(stats_str) = &cfg.stats_dir {
        let stats = PathBuf::from(stats_str);
        if stats.exists() {
            let perf = cfg
                .performances_dir
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    stats
                        .parent()
                        .map(|p| p.join("performances"))
                        .unwrap_or_else(|| stats.clone())
                });
            return Some((stats, perf));
        }
    }

    // 2. Auto-detect via Steam registry + libraryfolders.vdf + common paths
    auto_detect_kovaak_dirs()
}

/// Searches Windows Steam installation and all Steam libraries for KovaaK's directories.
pub fn auto_detect_kovaak_dirs() -> Option<(PathBuf, PathBuf)> {
    let mut steam_roots: Vec<PathBuf> = Vec::new();

    #[cfg(windows)]
    {
        if let Some(p) = read_steam_path_from_registry() {
            steam_roots.push(p);
        }
    }

    for candidate in [
        r"C:\Program Files (x86)\Steam",
        r"C:\Program Files\Steam",
        r"D:\Steam",
        r"D:\SteamLibrary",
        r"E:\SteamLibrary",
    ] {
        let p = PathBuf::from(candidate);
        if p.exists() && !steam_roots.contains(&p) {
            steam_roots.push(p);
        }
    }

    // Collect additional Steam library folders from libraryfolders.vdf
    let mut all_libraries = steam_roots.clone();
    for root in &steam_roots {
        let vdf_path = root.join("steamapps").join("libraryfolders.vdf");
        if let Ok(text) = fs::read_to_string(&vdf_path) {
            for lib in parse_steam_library_folders(&text) {
                if !all_libraries.contains(&lib) {
                    all_libraries.push(lib);
                }
            }
        }
    }

    for lib in all_libraries {
        let stats = lib.join(KOVAAK_REL_STATS);
        let perf = lib.join(KOVAAK_REL_PERF);
        if stats.exists() {
            return Some((stats, perf));
        }
    }

    None
}

#[cfg(windows)]
fn read_steam_path_from_registry() -> Option<PathBuf> {
    use winreg::enums::*;
    use winreg::RegKey;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(r"Software\Valve\Steam") {
        if let Ok(path) = key.get_value::<String, _>("SteamPath") {
            let p = PathBuf::from(path.replace('/', "\\"));
            if p.exists() {
                return Some(p);
            }
        }
    }

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    for sub in [
        r"SOFTWARE\WOW6432Node\Valve\Steam",
        r"SOFTWARE\Valve\Steam",
    ] {
        if let Ok(key) = hklm.open_subkey(sub) {
            if let Ok(path) = key.get_value::<String, _>("InstallPath") {
                let p = PathBuf::from(path);
                if p.exists() {
                    return Some(p);
                }
            }
        }
    }

    None
}

/// Parses `"path" "..."` entries from Steam's `libraryfolders.vdf`.
pub fn parse_steam_library_folders(vdf_content: &str) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for line in vdf_content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\"path\"") {
            let rest = rest.trim();
            if let Some(inner) = rest.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
                let unescaped = inner.replace("\\\\", "\\");
                let p = PathBuf::from(unescaped);
                if !dirs.contains(&p) {
                    dirs.push(p);
                }
            }
        }
    }
    dirs
}

/// Helper to check whether a directory looks like a valid KovaaK's stats folder.
pub fn is_valid_dir(path: &Path) -> bool {
    path.exists() && path.is_dir()
}
