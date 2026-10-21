//! Settings from `$XDG_CONFIG_HOME/oz/config.json` (`~/.config/oz/config.json`): the same
//! object the macOS version's `oz.set({...})` takes, e.g. `{"gap": 12, "swipe": {"fingers": 3}}`.

use std::path::PathBuf;

use oz_core::Settings;

pub fn path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_default();
    base.join("oz").join("config.json")
}

/// The settings in the config file, defaults for anything it doesn't set (or when it's
/// missing or can't be read).
pub fn load() -> Settings {
    let path = path();
    let Ok(text) = std::fs::read_to_string(&path) else { return Settings::default() };
    match serde_json::from_str(&text) {
        Ok(json) => Settings::from_json(&json),
        Err(e) => {
            tracing::warn!("ignoring {}: {e}", path.display());
            Settings::default()
        }
    }
}

/// The terminal to open: `$TERMINAL`, else foot.
pub fn terminal() -> String {
    std::env::var("TERMINAL").unwrap_or_else(|_| "foot".into())
}
