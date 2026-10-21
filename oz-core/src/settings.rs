//! User settings, with the same names and meanings as the macOS version's `oz.set({...})`.
//! Settings that only made sense on macOS (snapshot stand-ins, live capture, slow-app
//! proxies) are gone: a compositor draws every window itself and clips it natively.

use serde_json::Value;

/// What to do with a window that is partly scrolled past the scroll area's edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TopOverflow {
    /// Let it slide off the edge, clipped to the scroll area.
    Slide,
    /// Pin it to the edge and shrink it to the still-visible part.
    Shrink,
    /// Hide it as soon as it reaches the edge.
    Hide,
}

/// How a swipe comes to rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapMode {
    /// Always land with a row on the top edge (one panel at a time).
    Always,
    /// Glide freely; settle on a window edge only when released close to one.
    Near,
    /// Glide freely, never snap.
    Off,
}

/// Where a new window opens, relative to the focused one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InsertPosition {
    Below,
    Above,
    End,
    Start,
}

macro_rules! names {
    ($t:ty { $($v:ident = $s:literal),* }) => {
        impl $t {
            pub fn name(self) -> &'static str {
                match self { $(Self::$v => $s),* }
            }
            pub fn from_name(s: &str) -> Option<Self> {
                match s { $($s => Some(Self::$v),)* _ => None }
            }
        }
    };
}
names!(TopOverflow { Slide = "slide", Shrink = "shrink", Hide = "hide" });
names!(SnapMode { Always = "always", Near = "near", Off = "off" });
names!(InsertPosition { Below = "below", Above = "above", End = "end", Start = "start" });

#[derive(Clone, Debug, PartialEq)]
pub struct SwipeSettings {
    pub enabled: bool,
    pub fingers: u32,
    /// 1.0 = swiping the full touchpad height scrolls one viewport height.
    pub sensitivity: f64,
    /// Fingers up -> strip moves up (reveals windows below), like natural scrolling.
    pub natural: bool,
    pub snap: SnapMode,
    /// `Near` mode: snap when released within this many pixels of a window edge.
    pub snap_distance: f64,
    /// Scales how far a flick keeps gliding after release.
    pub momentum: f64,
    /// Focus the top window after a swipe. Off by default: scrolling only looks around.
    pub focus: bool,
}

impl Default for SwipeSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            fingers: 3,
            sensitivity: 1.5,
            natural: true,
            snap: SnapMode::Near,
            snap_distance: 70.0,
            momentum: 1.0,
            focus: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct DoubleTapSettings {
    pub enabled: bool,
    pub fingers: u32,
    /// What a three-finger double tap does: an overview, or any command name.
    pub action: String,
    pub interval: f64,
}

impl Default for DoubleTapSettings {
    fn default() -> Self {
        Self { enabled: true, fingers: 3, action: "overview".into(), interval: 0.4 }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub gap: f64,
    pub outer_gap: f64,
    /// Row width as a fraction of the scroll area's width.
    pub width: f64,
    /// Height presets for cycleHeight (fractions of the viewport height).
    pub heights: Vec<f64>,
    pub default_height: f64,
    /// Width presets for cycleWidth when several windows share a row (fractions of the row).
    pub widths: Vec<f64>,
    /// Step for resizeWidth / resizeHeight when no argument is given.
    pub resize_step: f64,
    pub new_window_position: InsertPosition,
    /// Seconds.
    pub animation_duration: f64,
    pub top_overflow: TopOverflow,
    pub min_visible: f64,
    pub swipe: SwipeSettings,
    pub double_tap: DoubleTapSettings,
    pub debug: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            gap: 10.0,
            outer_gap: 10.0,
            width: 1.0,
            heights: vec![1.0 / 3.0, 0.5, 2.0 / 3.0, 1.0],
            default_height: 0.5,
            widths: vec![1.0 / 3.0, 0.5, 2.0 / 3.0],
            resize_step: 0.05,
            new_window_position: InsertPosition::Below,
            animation_duration: 0.2,
            // A compositor clips natively, so sliding off the edge costs nothing here.
            top_overflow: TopOverflow::Slide,
            min_visible: 80.0,
            swipe: SwipeSettings::default(),
            double_tap: DoubleTapSettings::default(),
            debug: false,
        }
    }
}

impl Settings {
    pub fn from_json(j: &Value) -> Self {
        let mut s = Self::default();
        s.apply(j);
        s
    }

    /// Applies the keys present in `j` (the config object), checking each value; unknown keys
    /// and bad values are ignored.
    pub fn apply(&mut self, j: &Value) {
        let num = |k: &str| j.get(k).and_then(Value::as_f64);
        let flag = |k: &str| j.get(k).and_then(Value::as_bool);
        let text = |k: &str| j.get(k).and_then(Value::as_str);
        let list = |k: &str, ok: fn(f64) -> bool| -> Option<Vec<f64>> {
            let r: Vec<f64> = j.get(k)?.as_array()?.iter().filter_map(Value::as_f64).filter(|&v| ok(v)).collect();
            (!r.is_empty()).then_some(r)
        };
        if let Some(v) = num("gap") {
            self.gap = v.max(0.0);
        }
        if let Some(v) = num("outerGap") {
            self.outer_gap = v.max(0.0);
        }
        if let Some(v) = num("width") {
            self.width = v.clamp(0.1, 1.0);
        }
        if let Some(r) = list("heights", |v| v > 0.0 && v <= 1.0) {
            self.heights = r;
        }
        if let Some(v) = num("defaultHeight").filter(|&v| v > 0.0 && v <= 1.0) {
            self.default_height = v;
        }
        if let Some(r) = list("widths", |v| v > 0.0 && v < 1.0) {
            self.widths = r;
        }
        if let Some(v) = num("resizeStep").filter(|&v| v > 0.0 && v < 1.0) {
            self.resize_step = v;
        }
        if let Some(p) = text("newWindowPosition").and_then(InsertPosition::from_name) {
            self.new_window_position = p;
        }
        if let Some(v) = num("animationDuration") {
            self.animation_duration = v.max(0.0);
        }
        if let Some(t) = text("topOverflow").and_then(TopOverflow::from_name) {
            self.top_overflow = t;
        }
        if let Some(v) = num("minVisible") {
            self.min_visible = v.max(0.0);
        }
        if let Some(v) = flag("debug") {
            self.debug = v;
        }

        if let Some(s) = j.get("swipe").filter(|v| v.is_object()) {
            let num = |k: &str| s.get(k).and_then(Value::as_f64);
            let flag = |k: &str| s.get(k).and_then(Value::as_bool);
            let sw = &mut self.swipe;
            if let Some(v) = flag("enabled") {
                sw.enabled = v;
            }
            if let Some(v) = num("fingers") {
                sw.fingers = (v as i64).clamp(2, 5) as u32;
            }
            if let Some(v) = num("sensitivity").filter(|&v| v > 0.0) {
                sw.sensitivity = v;
            }
            if let Some(v) = flag("natural") {
                sw.natural = v;
            }
            match s.get("snap") {
                Some(Value::String(m)) => {
                    if let Some(m) = SnapMode::from_name(m) {
                        sw.snap = m;
                    }
                }
                Some(Value::Bool(b)) => sw.snap = if *b { SnapMode::Always } else { SnapMode::Off },
                _ => {}
            }
            if let Some(v) = num("snapDistance") {
                sw.snap_distance = v.max(0.0);
            }
            if let Some(v) = num("momentum") {
                sw.momentum = v.max(0.0);
            }
            if let Some(v) = flag("focus") {
                sw.focus = v;
            }
        }
        if let Some(d) = j.get("doubleTap").filter(|v| v.is_object()) {
            let dt = &mut self.double_tap;
            if let Some(v) = d.get("enabled").and_then(Value::as_bool) {
                dt.enabled = v;
            }
            if let Some(v) = d.get("fingers").and_then(Value::as_f64) {
                dt.fingers = (v as i64).clamp(2, 5) as u32;
            }
            if let Some(v) = d.get("action").and_then(Value::as_str).filter(|v| !v.is_empty()) {
                dt.action = v.into();
            }
            if let Some(v) = d.get("interval").and_then(Value::as_f64).filter(|&v| v > 0.0) {
                dt.interval = v;
            }
        }
    }
}
