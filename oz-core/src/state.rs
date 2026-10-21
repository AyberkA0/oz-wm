//! A strip's arrangement as plain data: saved to disk to survive restarts, kept in memory for
//! undo. The JSON has the same shape as the macOS version's `layout.json` entries.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::strip::{Row, Strip, WinId};

/// A strip's rows (windows, heights, widths and places in the row), scroll position and
/// focused window, plus the windows tiled in the regions around the scroll area.
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct StripState {
    pub rows: Vec<StateRow>,
    pub scroll: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub focused: Option<WinId>,
    /// Windows tiled in the regions outside the scroll area: region index (as text) ->
    /// windows in order.
    #[serde(default)]
    pub docks: BTreeMap<String, Vec<StateWindow>>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct StateRow {
    pub height: f64,
    pub windows: Vec<StateWindow>,
}

#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct StateWindow {
    /// Window id (the same while the compositor keeps running).
    pub id: WinId,
    /// To find it again after its app restarted (new id): the app and window title. On
    /// Wayland `app` is the xdg-toplevel app_id.
    pub app: String,
    #[serde(rename = "bundleID", default, skip_serializing_if = "Option::is_none")]
    pub bundle_id: Option<String>,
    pub title: String,
    /// Width share (`None`: 1) and, alone in a narrowed row, the place in it (`None`: left edge).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<f64>,
}

impl StateWindow {
    pub fn new(id: WinId) -> Self {
        Self { id, ..Default::default() }
    }

    pub fn described(id: WinId, app: &str, bundle_id: Option<&str>, title: &str) -> Self {
        Self { id, app: app.into(), bundle_id: bundle_id.map(Into::into), title: title.into(), ..Default::default() }
    }
}

impl StripState {
    /// Every window id it mentions.
    pub fn window_ids(&self) -> Vec<WinId> {
        self.rows
            .iter()
            .flat_map(|r| r.windows.iter().map(|w| w.id))
            .chain(self.docks.values().flat_map(|d| d.iter().map(|w| w.id)))
            .collect()
    }
}

/// What is known about a live window when matching saved ones to it.
#[derive(Clone, Debug, PartialEq)]
pub struct WindowDescription {
    pub id: WinId,
    pub app: String,
    pub bundle_id: Option<String>,
    pub title: String,
}

impl WindowDescription {
    pub fn new(id: WinId, app: &str, bundle_id: Option<&str>, title: &str) -> Self {
        Self { id, app: app.into(), bundle_id: bundle_id.map(Into::into), title: title.into() }
    }

    fn same_app(&self, w: &StateWindow) -> bool {
        match (&self.bundle_id, &w.bundle_id) {
            (Some(a), Some(b)) => a == b,
            _ => self.app == w.app,
        }
    }
}

pub struct WindowMatcher;

impl WindowMatcher {
    /// The live window a saved one stands for: the same id of the same app (the app kept
    /// running), else the first unused window of that app with the same title (it restarted).
    /// Never a window already used, nor a different app's.
    pub fn find(saved: &StateWindow, candidates: &[WindowDescription], used: &HashSet<WinId>) -> Option<WinId> {
        if let Some(c) = candidates.iter().find(|c| c.id == saved.id && !used.contains(&c.id)) {
            if c.same_app(saved) {
                return Some(c.id);
            }
        }
        candidates.iter().find(|c| !used.contains(&c.id) && c.same_app(saved) && c.title == saved.title).map(|c| c.id)
    }

    /// For a preset opened later, when windows are new (new ids, often new titles: a browser's
    /// tab title changes): the same window if it's still there, else a window of the same app
    /// with the same title, else the app's next window not used yet.
    pub fn find_loosely(saved: &StateWindow, candidates: &[WindowDescription], used: &HashSet<WinId>) -> Option<WinId> {
        Self::find(saved, candidates, used)
            .or_else(|| candidates.iter().find(|c| !used.contains(&c.id) && c.same_app(saved)).map(|c| c.id))
    }
}

impl Strip {
    /// This strip's arrangement; `describe` names each window (app, title) for matching later.
    pub fn state(&self, describe: impl Fn(WinId) -> Option<WindowDescription>) -> StripState {
        StripState {
            rows: self
                .rows
                .iter()
                .map(|row| StateRow {
                    height: row.height,
                    windows: row
                        .windows
                        .iter()
                        .map(|&id| {
                            let d = describe(id);
                            StateWindow {
                                id,
                                app: d.as_ref().map(|d| d.app.clone()).unwrap_or_default(),
                                bundle_id: d.as_ref().and_then(|d| d.bundle_id.clone()),
                                title: d.as_ref().map(|d| d.title.clone()).unwrap_or_default(),
                                weight: self.width_weights.get(&id).copied(),
                                offset: self.width_offsets.get(&id).copied(),
                            }
                        })
                        .collect(),
                })
                .collect(),
            scroll: self.scroll_y,
            focused: self.focused,
            docks: BTreeMap::new(),
        }
    }

    /// Rearranges this strip's windows like `state`: windows it matches go to their saved rows
    /// with their heights (at most `max_height`), widths and places; the rest keep their order,
    /// after them. Scroll and focus come back too. Returns the windows that were matched.
    pub fn restore(
        &mut self,
        state: &StripState,
        max_height: f64,
        mut matcher: impl FnMut(&StateWindow, &HashSet<WinId>) -> Option<WinId>,
    ) -> HashSet<WinId> {
        let mut used = HashSet::new();
        let mut new_rows = Vec::new();
        let mut weights = HashMap::new();
        let mut offsets = HashMap::new();
        let mut focus = None;
        for saved in &state.rows {
            let mut ids = Vec::new();
            for w in &saved.windows {
                let Some(id) = matcher(w, &used).filter(|&id| self.contains(id)) else { continue };
                ids.push(id);
                used.insert(id);
                if let Some(x) = w.weight {
                    weights.insert(id, x);
                }
                if let Some(x) = w.offset {
                    offsets.insert(id, x);
                }
                if Some(w.id) == state.focused {
                    focus = Some(id);
                }
            }
            if !ids.is_empty() {
                new_rows.push(Row::new(ids, saved.height.max(60.0).min(max_height)));
            }
        }
        for row in &self.rows {
            let rest: Vec<WinId> = row.windows.iter().copied().filter(|w| !used.contains(w)).collect();
            if !rest.is_empty() {
                new_rows.push(Row::new(rest, row.height));
            }
        }
        self.rows = new_rows;
        for &id in &used {
            match weights.get(&id) {
                Some(&w) => self.width_weights.insert(id, w),
                None => self.width_weights.remove(&id),
            };
            match offsets.get(&id) {
                Some(&o) => self.width_offsets.insert(id, o),
                None => self.width_offsets.remove(&id),
            };
        }
        if focus.is_some() {
            self.focused = focus;
        }
        self.scroll_y = state.scroll;
        self.render_scroll_y = self.scroll_y;
        used
    }
}
