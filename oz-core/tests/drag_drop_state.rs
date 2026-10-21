use std::collections::{BTreeMap, HashMap, HashSet};

use oz_core::*;

// 1000 px viewport at y=50, gap 10 (as in the strip tests).
fn params() -> LayoutParams {
    LayoutParams::new(Rect::new(10.0, 50.0, 1000.0, 1000.0), 40.0, 1100.0, 10.0)
}

fn make_strip(heights: &[f64]) -> Strip {
    let mut s = Strip::new();
    for (i, &h) in heights.iter().enumerate() {
        s.insert(i as WinId + 1, h, None, InsertPosition::End);
    }
    s
}

fn drop(s: &Strip, x: f64, y: f64, dragging: WinId) -> Option<(DropSlot, Rect)> {
    s.drop_slot(Point::new(x, y), dragging, &params(), &HashMap::new(), 6.0)
}

fn slot(s: &Strip, x: f64, y: f64, dragging: WinId) -> Option<DropSlot> {
    drop(s, x, y, dragging).map(|(slot, _)| slot)
}

// Drop slots

#[test]
fn drop_between_rows() {
    let s = make_strip(&[300.0, 300.0, 300.0]); // rows at y 50, 360, 670
                                                // Cursor in the upper half of row 2: above it. Window 3 is dragged.
    let (a, line) = drop(&s, 500.0, 400.0, 3).unwrap();
    assert_eq!(a, DropSlot::Row { before: Some(2) });
    // The line sits in the gap between rows 1 and 2, across the row.
    assert_eq!(line, Rect::new(10.0, 352.0, 1000.0, 6.0));
    // Lower half of the last row: after it.
    assert_eq!(slot(&s, 500.0, 900.0, 1), Some(DropSlot::Row { before: None }));
}

#[test]
fn drop_where_it_already_is_shows_nothing() {
    let s = make_strip(&[300.0, 300.0, 300.0]);
    // Window 2 dropped just above or below its own row: no move.
    assert_eq!(drop(&s, 500.0, 400.0, 2), None);
    assert_eq!(drop(&s, 500.0, 620.0, 2), None);
}

#[test]
fn drop_beside_a_window() {
    let s = make_strip(&[300.0, 300.0]);
    // Near the right edge of window 1 (its right quarter): beside it, on the right.
    let (r, line) = drop(&s, 950.0, 200.0, 2).unwrap();
    assert_eq!(r, DropSlot::Beside { anchor: 1, right: true });
    assert_eq!(line, Rect::new(1012.0, 50.0, 6.0, 300.0));
    assert_eq!(slot(&s, 40.0, 200.0, 2), Some(DropSlot::Beside { anchor: 1, right: false }));
    // In its middle: between rows instead.
    assert_eq!(slot(&s, 500.0, 100.0, 2), Some(DropSlot::Row { before: Some(1) }));
}

#[test]
fn drop_into_an_empty_strip() {
    assert_eq!(slot(&Strip::new(), 500.0, 500.0, 9), Some(DropSlot::Row { before: None }));
}

// Scrolling under a drag

#[test]
fn drag_scroll_only_in_the_edge_bands() {
    let zone = Rect::new(0.0, 40.0, 1000.0, 1000.0); // bands: 150 px
    let v = |y: f64, scroll: f64| DragScroll::velocity(y, zone, 1000.0, scroll, 2000.0);
    // The middle never scrolls.
    assert_eq!(v(540.0, 500.0), (0.0, false));
    // Top band scrolls up, bottom band down, faster at the very edge.
    assert!(v(60.0, 500.0).0 < v(170.0, 500.0).0);
    assert!(v(60.0, 500.0).0 < 0.0);
    assert!(v(1030.0, 500.0).0 > v(900.0, 500.0).0);
    assert!(v(900.0, 500.0).0 > 0.0);
    assert!((v(40.0, 500.0).0 + 1500.0).abs() < 0.5);
    // At the top of the strip there's nothing more to show above.
    assert_eq!(v(60.0, 0.0), (0.0, true));
}

// Saving and restoring

#[test]
fn state_round_trips_through_json() {
    let mut s = make_strip(&[300.0, 400.0]);
    s.set_lone_width(2, Some(0.5));
    s.set_lone_offset(2, 0.25);
    s.scroll_y = 120.0;
    s.focused = Some(2);
    let state = s
        .state(|id| Some(WindowDescription::new(id, &format!("App{id}"), Some(&format!("b.{id}")), &format!("T{id}"))));
    let json = serde_json::to_string(&state).unwrap();
    let decoded: StripState = serde_json::from_str(&json).unwrap();
    assert_eq!(decoded, state);
    assert_eq!(decoded.rows[1].windows[0].weight, Some(0.5));
    assert_eq!(decoded.rows[1].windows[0].offset, Some(0.25));
    // Same keys as the macOS version's layout.json.
    assert!(json.contains("\"bundleID\":\"b.1\""));
}

#[test]
fn reads_a_macos_layout_entry() {
    let json = r#"{"docks":{"0":[{"app":"Finder","id":7,"title":"Home"}]},"focused":3,
        "rows":[{"height":500,"windows":[{"app":"Safari","bundleID":"com.apple.Safari","id":3,"title":"Docs","weight":0.5}]}],
        "scroll":0}"#;
    let state: StripState = serde_json::from_str(json).unwrap();
    assert_eq!(state.window_ids(), vec![3, 7]);
    assert_eq!(state.rows[0].windows[0].bundle_id.as_deref(), Some("com.apple.Safari"));
}

#[test]
fn restore_puts_windows_back_and_keeps_new_ones_after() {
    // Saved: [2] [1, 3] (1 and 3 side by side), focus on 3, scrolled.
    let saved = StripState {
        rows: vec![
            StateRow { height: 500.0, windows: vec![StateWindow::new(2)] },
            StateRow {
                height: 350.0,
                windows: vec![
                    StateWindow { weight: Some(0.6), ..StateWindow::new(1) },
                    StateWindow { weight: Some(0.4), ..StateWindow::new(3) },
                ],
            },
        ],
        scroll: 200.0,
        focused: Some(3),
        docks: BTreeMap::new(),
    };
    // Now: [1] [2] [3] [4] (4 is new).
    let mut s = make_strip(&[300.0, 300.0, 300.0, 300.0]);
    let live: HashSet<WinId> = s.window_ids().into_iter().collect();
    let used = s.restore(&saved, 1000.0, |w, used| (live.contains(&w.id) && !used.contains(&w.id)).then_some(w.id));
    assert_eq!(used, HashSet::from([1, 2, 3]));
    let rows: Vec<Vec<WinId>> = s.rows.iter().map(|r| r.windows.clone()).collect();
    assert_eq!(rows, vec![vec![2], vec![1, 3], vec![4]]);
    let heights: Vec<f64> = s.rows.iter().map(|r| r.height).collect();
    assert_eq!(heights, vec![500.0, 350.0, 300.0]);
    assert!((s.weight(1) - 0.6).abs() < 0.001);
    assert_eq!(s.focused, Some(3));
    assert_eq!(s.scroll_y, 200.0);
}

#[test]
fn preset_matching_falls_back_to_the_apps_next_window() {
    let live = [
        WindowDescription::new(50, "firefox", None, "New Tab"),
        WindowDescription::new(51, "firefox", None, "Docs"),
        WindowDescription::new(52, "slack", None, "Slack"),
    ];
    // Saved: two Firefox windows (titles that no longer exist except "Docs") and Slack.
    let docs = StateWindow::described(1, "firefox", None, "Docs");
    let mail = StateWindow::described(2, "firefox", None, "Inbox");
    let chat = StateWindow::described(3, "slack", None, "Slack");
    // The same title wins; then the app's next unused window; never another app's.
    assert_eq!(WindowMatcher::find_loosely(&docs, &live, &HashSet::new()), Some(51));
    assert_eq!(WindowMatcher::find_loosely(&mail, &live, &HashSet::from([51])), Some(50));
    assert_eq!(WindowMatcher::find_loosely(&mail, &live, &HashSet::from([50, 51])), None);
    assert_eq!(WindowMatcher::find_loosely(&chat, &live, &HashSet::new()), Some(52));
    let other = StateWindow::described(4, "thunderbird", None, "x");
    assert_eq!(WindowMatcher::find_loosely(&other, &live, &HashSet::new()), None);
}

#[test]
fn preset_round_trips_and_names_itself() {
    let app = |id: &str, name: &str| PresetApp { bundle_id: Some(id.into()), name: name.into(), path: None };
    let apps = vec![
        app("firefox", "Firefox"),
        app("kitty", "Kitty"),
        app("slack", "Slack"),
        app("org.gnome.Nautilus", "Files"),
    ];
    assert_eq!(Preset::default_name(&apps[..3]), "Firefox, Kitty, Slack");
    assert_eq!(Preset::default_name(&apps), "Firefox, Kitty and 2 more");
    assert_eq!(Preset::default_name(&[]), "Empty desktop");
    let p = Preset {
        name: "Work".into(),
        created: 0.0,
        desktop: StripState {
            rows: vec![StateRow { height: 400.0, windows: vec![StateWindow::described(1, "firefox", None, "")] }],
            ..Default::default()
        },
        scroll_area: Some([0.5, 0.0, 0.5, 1.0]),
        apps,
    };
    let decoded: Preset = serde_json::from_str(&serde_json::to_string(&p).unwrap()).unwrap();
    assert_eq!(decoded, p);
}

#[test]
fn matching_by_id_then_by_app_and_title() {
    let live = [
        WindowDescription::new(10, "kitty", None, "zsh"),
        WindowDescription::new(11, "firefox", None, "Docs"),
        WindowDescription::new(12, "firefox", None, "News"),
    ];
    // Same id, same app: that window.
    let term = StateWindow::described(10, "kitty", None, "old");
    assert_eq!(WindowMatcher::find(&term, &live, &HashSet::new()), Some(10));
    // The app restarted (new ids): found by app and title.
    let news = StateWindow::described(99, "firefox", None, "News");
    assert_eq!(WindowMatcher::find(&news, &live, &HashSet::new()), Some(12));
    assert_eq!(WindowMatcher::find(&news, &live, &HashSet::from([12])), None);
    // An id that now belongs to another app is not a match.
    let mail = StateWindow::described(11, "thunderbird", None, "Docs");
    assert_eq!(WindowMatcher::find(&mail, &live, &HashSet::new()), None);
}
