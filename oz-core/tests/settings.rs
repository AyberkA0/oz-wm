use oz_core::*;
use serde_json::json;

#[test]
fn defaults_match_the_macos_version() {
    let s = Settings::default();
    assert_eq!(s.gap, 10.0);
    assert_eq!(s.heights.len(), 4);
    assert_eq!(s.swipe.snap, SnapMode::Near);
    assert_eq!(s.new_window_position, InsertPosition::Below);
}

#[test]
fn applies_valid_values_and_ignores_the_rest() {
    let s = Settings::from_json(&json!({
        "gap": 16,
        "width": 3,
        "heights": [0.25, 2, "x", 0.75],
        "widths": [1],
        "newWindowPosition": "end",
        "topOverflow": "sideways",
        "swipe": { "fingers": 9, "snap": false, "sensitivity": -1 },
        "doubleTap": { "action": "" },
        "unknown": true
    }));
    assert_eq!(s.gap, 16.0);
    assert_eq!(s.width, 1.0);
    assert_eq!(s.heights, vec![0.25, 0.75]);
    assert_eq!(s.widths, Settings::default().widths);
    assert_eq!(s.new_window_position, InsertPosition::End);
    assert_eq!(s.top_overflow, TopOverflow::Slide);
    assert_eq!(s.swipe.fingers, 5);
    assert_eq!(s.swipe.snap, SnapMode::Off);
    assert_eq!(s.swipe.sensitivity, 1.5);
    assert_eq!(s.double_tap.action, "overview");
}

#[test]
fn names_round_trip() {
    for m in [SnapMode::Always, SnapMode::Near, SnapMode::Off] {
        assert_eq!(SnapMode::from_name(m.name()), Some(m));
    }
    assert_eq!(TopOverflow::from_name("shrink"), Some(TopOverflow::Shrink));
    assert_eq!(InsertPosition::from_name("start"), Some(InsertPosition::Start));
}
