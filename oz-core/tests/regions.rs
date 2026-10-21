use oz_core::*;

// A 1710x1074 usable output starting below a 38 px panel.
const SCREEN: Rect = Rect::new(0.0, 38.0, 1710.0, 1074.0);

fn compute(zone: Rect) -> Vec<Rect> {
    OutsideRegions::compute(SCREEN, zone, OutsideRegions::MIN_THICKNESS)
}

#[test]
fn right_half_zone_leaves_the_left_half() {
    let zone = Rect::new(855.0, 38.0, 855.0, 1074.0);
    assert_eq!(compute(zone), vec![Rect::new(0.0, 38.0, 855.0, 1074.0)]);
}

#[test]
fn whole_screen_zone_has_no_outside() {
    assert_eq!(compute(SCREEN), vec![]);
}

#[test]
fn thin_sides_are_left_out() {
    // 100 px to the left is too thin for a window; the right side remains.
    let zone = Rect::new(100.0, 38.0, 1000.0, 1074.0);
    assert_eq!(compute(zone), vec![Rect::new(1100.0, 38.0, 610.0, 1074.0)]);
}

#[test]
fn corner_zone_gives_the_longer_strip_the_corner() {
    // Zone in the bottom right: an L-shaped outside of two rectangles. The output is wider
    // than tall, so the top strip running the full width is the longest possible.
    let zone = Rect::new(600.0, 438.0, 1110.0, 674.0);
    assert_eq!(compute(zone), vec![Rect::new(0.0, 38.0, 1710.0, 400.0), Rect::new(0.0, 438.0, 600.0, 674.0)]);
}

#[test]
fn centered_zone_covers_every_side_with_four_rectangles() {
    let zone = Rect::new(400.0, 238.0, 900.0, 674.0);
    let regions = compute(zone);
    assert_eq!(regions.len(), 4);
    // Together with the zone they tile the output exactly: no overlap, nothing missing.
    let area: f64 = zone.area() + regions.iter().map(Rect::area).sum::<f64>();
    assert_eq!(area, SCREEN.area());
    for (i, a) in regions.iter().enumerate() {
        assert!(a.intersection(&zone).is_none_or(|o| o.area() == 0.0));
        for b in &regions[i + 1..] {
            assert!(a.intersection(b).is_none_or(|o| o.area() == 0.0));
        }
    }
    // The longest rectangle runs the full width of the output.
    let longest = regions.iter().map(|r| r.w.max(r.h)).fold(0.0, f64::max);
    assert_eq!(longest, 1710.0);
}

#[test]
fn frames_split_along_the_long_side() {
    // A tall region: two windows stacked, each half the height.
    let tall = Rect::new(0.0, 0.0, 600.0, 1000.0);
    assert_eq!(
        OutsideRegions::frames(tall, 2, 10.0),
        vec![Rect::new(10.0, 10.0, 580.0, 485.0), Rect::new(10.0, 505.0, 580.0, 485.0)]
    );
    // A wide region: side by side, each half the width.
    let wide = Rect::new(0.0, 0.0, 1000.0, 400.0);
    assert_eq!(
        OutsideRegions::frames(wide, 2, 10.0),
        vec![Rect::new(10.0, 10.0, 485.0, 380.0), Rect::new(505.0, 10.0, 485.0, 380.0)]
    );
}
