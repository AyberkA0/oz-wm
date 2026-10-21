//! Saved desktops: the apps on a workspace and how their windows were arranged, to open again
//! later (apps launched, windows put back in place).

use serde::{Deserialize, Serialize};

use crate::state::StripState;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub name: String,
    /// When it was saved, seconds since the Unix epoch.
    pub created: f64,
    /// The windows' arrangement (window ids in it are only hints: they change when apps restart).
    pub desktop: StripState,
    /// The workspace's scroll area as fractions of the output (x, y from the top, width, height).
    #[serde(rename = "scrollArea", default, skip_serializing_if = "Option::is_none")]
    pub scroll_area: Option<[f64; 4]>,
    /// Apps to open, in the order their windows appear.
    pub apps: Vec<PresetApp>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PresetApp {
    /// The app id (on Wayland: the xdg-toplevel app_id, usually the desktop entry name).
    #[serde(rename = "bundleID", default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    pub name: String,
    /// How to start it when the app id doesn't lead to a desktop entry (a command line).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl Preset {
    /// At most this many are kept.
    pub const LIMIT: usize = 5;

    /// A name from the apps: "Firefox, Kitty, Slack" or "Firefox, Kitty and 3 more".
    pub fn default_name(apps: &[PresetApp]) -> String {
        let names: Vec<&str> = apps.iter().map(|a| a.name.as_str()).collect();
        match names.len() {
            0 => "Empty desktop".into(),
            1..=3 => names.join(", "),
            n => format!("{} and {} more", names[..2].join(", "), n - 2),
        }
    }
}
