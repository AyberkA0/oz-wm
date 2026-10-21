use std::collections::HashMap;

use oz_core::*;

// 1000 px tall viewport starting at y=50, gap 10.
fn params() -> LayoutParams {
    LayoutParams::new(Rect::new(10.0, 50.0, 1000.0, 1000.0), 40.0, 1100.0, 10.0)
}
const VH: f64 = 1000.0;

fn make_strip(heights: &[f64]) -> Strip {
    let mut s = Strip::new();
    for (i, &h) in heights.iter().enumerate() {
        s.insert(i as WinId + 1, h, None, InsertPosition::End);
    }
    s
}

fn placements(s: &Strip, p: &LayoutParams, scroll: f64, min_heights: &[(WinId, f64)]) -> HashMap<WinId, Placement> {
    let mins: HashMap<WinId, f64> = min_heights.iter().copied().collect();
    s.layout(p, scroll, &mins).into_iter().map(|i| (i.id, i.placement)).collect()
}

fn frames(s: &Strip, scroll: f64) -> HashMap<WinId, Placement> {
    placements(s, &params(), scroll, &[])
}

fn frame(x: f64, y: f64, w: f64, h: f64) -> Placement {
    Placement::Frame(Rect::new(x, y, w, h))
}

fn rows(s: &Strip) -> Vec<Vec<WinId>> {
    s.rows.iter().map(|r| r.windows.clone()).collect()
}

#[test]
fn stacks_rows_vertically() {
    let s = make_strip(&[495.0, 495.0, 495.0]);
    let f = frames(&s, 0.0);
    assert_eq!(f[&1], frame(10.0, 50.0, 1000.0, 495.0));
    assert_eq!(f[&2], frame(10.0, 555.0, 1000.0, 495.0));
    // Third row starts at 1060 < hard_bottom -> partially visible below.
    assert_eq!(f[&3], frame(10.0, 1060.0, 1000.0, 495.0));
}

#[test]
fn half_ratio_fills_viewport() {
    let h = Strip::height_for_ratio(0.5, VH, 10.0);
    assert_eq!(h * 2.0 + 10.0, VH);
}

#[test]
fn scroll_parks_and_shrinks() {
    let s = make_strip(&[495.0, 495.0, 495.0]);
    // Scrolled so row 1 has only 45 px left above the viewport -> parked (below min_visible).
    assert_eq!(frames(&s, 460.0)[&1], Placement::Park);
    // Scrolled a little: row 1 pinned to hard_top and shrunk.
    assert_eq!(frames(&s, 100.0)[&1], frame(10.0, 40.0, 1000.0, 405.0));
    // A window that can't get that small gets parked instead.
    assert_eq!(placements(&s, &params(), 100.0, &[(1, 450.0)])[&1], Placement::Park);
}

#[test]
fn scroll_area_cuts_off_at_its_bottom() {
    // Right half of the screen, ending at y=700 (above the screen bottom).
    let mut p = LayoutParams::new(Rect::new(510.0, 50.0, 500.0, 640.0), 40.0, 700.0, 10.0);
    p.top_overflow = TopOverflow::Slide;
    p.cut_bottom = true;
    let s = make_strip(&[315.0, 315.0, 315.0]);
    let f = placements(&s, &p, 0.0, &[]);
    assert_eq!(f[&1], frame(510.0, 50.0, 500.0, 315.0));
    assert_eq!(f[&2], frame(510.0, 375.0, 500.0, 315.0));
    // Starts at 700 = the area's bottom: out of view.
    assert_eq!(f[&3], Placement::Park);
    // Scrolled half a row: the second row crosses the bottom edge and is drawn cut off.
    let f = placements(&s, &p, -100.0, &[]);
    assert_eq!(f[&2], Placement::Overflow(Rect::new(510.0, 475.0, 500.0, 315.0)));
    // Shrink mode keeps only the part inside the area.
    p.top_overflow = TopOverflow::Shrink;
    assert_eq!(placements(&s, &p, -100.0, &[])[&2], frame(510.0, 475.0, 500.0, 225.0));
    // Without cut_bottom (area reaching the screen bottom) windows run off as before.
    p.cut_bottom = false;
    p.hard_bottom = 1100.0;
    assert_eq!(placements(&s, &p, -100.0, &[])[&2], frame(510.0, 475.0, 500.0, 315.0));
}

#[test]
fn dropping_between_rows_and_beside_a_window() {
    let mut s = make_strip(&[300.0, 300.0, 300.0]); // rows: [1] [2] [3]
                                                    // Window 3 dragged above row 2.
    s.remove(3);
    s.insert_row(3, 300.0, Some(2));
    assert_eq!(rows(&s), vec![vec![1], vec![3], vec![2]]);
    // Window 1 dropped on the right of window 2: they share a row.
    s.remove(1);
    assert!(s.insert_beside(1, 2, true));
    assert_eq!(rows(&s), vec![vec![3], vec![2, 1]]);
    // ...and to the end of the strip.
    s.remove(3);
    s.insert_row(3, 300.0, None);
    assert_eq!(rows(&s), vec![vec![2, 1], vec![3]]);
    assert!(!s.insert_beside(4, 99, false));
}

#[test]
fn slide_mode_reports_overflow() {
    let mut p = params();
    p.top_overflow = TopOverflow::Slide;
    let s = make_strip(&[495.0, 495.0]);
    let f = placements(&s, &p, 100.0, &[]);
    assert_eq!(f[&1], Placement::Overflow(Rect::new(10.0, -50.0, 1000.0, 495.0)));
    assert_eq!(f[&2], frame(10.0, 455.0, 1000.0, 495.0));
    // Fully above the top is still parked.
    assert_eq!(placements(&s, &p, 600.0, &[])[&1], Placement::Park);
}

#[test]
fn row_with_two_panels() {
    let mut s = make_strip(&[495.0, 495.0]);
    assert!(s.slurp_in(1));
    assert_eq!(s.rows.len(), 1);
    let f = frames(&s, 0.0);
    assert_eq!(f[&1], frame(10.0, 50.0, 495.0, 495.0));
    assert_eq!(f[&2], frame(515.0, 50.0, 495.0, 495.0));
    assert!(s.barf_out(2));
    assert_eq!(rows(&s), vec![vec![1], vec![2]]);
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.001
}

#[test]
fn widths_within_row() {
    let mut s = make_strip(&[495.0, 495.0, 495.0]);
    s.slurp_in(1); // row 0 = [1, 2]
    assert!(close(s.width_fraction(1).unwrap(), 0.5));
    assert!(s.set_width_fraction(1, 2.0 / 3.0));
    assert!(close(s.width_fraction(1).unwrap(), 2.0 / 3.0));
    let f = frames(&s, 0.0);
    assert_eq!(f[&1], frame(10.0, 50.0, 660.0, 495.0));
    assert_eq!(f[&2], frame(680.0, 50.0, 330.0, 495.0));
    let next = s.cycled_width_fraction(1, &[1.0 / 3.0, 0.5, 2.0 / 3.0], true).unwrap();
    assert!(close(next, 1.0 / 3.0));
    s.equalize_row(2);
    assert!(close(s.width_fraction(2).unwrap(), 0.5));
    // Alone in a row: nothing to share.
    assert!(!s.set_width_fraction(3, 0.5));
}

#[test]
fn narrow_lone_window_leaves_space() {
    let mut s = make_strip(&[400.0, 300.0]);
    // Narrowed alone: half width, the rest of the row stays empty, even with a narrow window
    // in the next row that would fit there.
    s.set_lone_width(1, Some(0.5));
    s.set_lone_width(2, Some(0.3));
    assert_eq!(rows(&s), vec![vec![1], vec![2]]);
    assert_eq!(frames(&s, 0.0)[&1], frame(10.0, 50.0, 500.0, 400.0));
    // Back to full width.
    s.set_lone_width(1, None);
    assert_eq!(frames(&s, 0.0)[&1], frame(10.0, 50.0, 1000.0, 400.0));
}

#[test]
fn narrow_lone_window_keeps_its_place() {
    let mut s = make_strip(&[400.0]);
    // Narrowed from its left edge: it sits on the right of the row.
    s.set_lone_width(1, Some(0.4));
    s.set_lone_offset(1, 0.6);
    assert_eq!(frames(&s, 0.0)[&1], frame(610.0, 50.0, 400.0, 400.0));
    // Wider again: it stays where it is as far as it fits, then moves left.
    s.set_lone_width(1, Some(0.5));
    assert_eq!(frames(&s, 0.0)[&1], frame(510.0, 50.0, 500.0, 400.0));
    // In the middle.
    s.set_lone_offset(1, 0.25);
    assert_eq!(frames(&s, 0.0)[&1], frame(260.0, 50.0, 500.0, 400.0));
    // Full width forgets the place.
    s.set_lone_width(1, None);
    assert_eq!(s.width_offsets.get(&1), None);
    assert_eq!(frames(&s, 0.0)[&1], frame(10.0, 50.0, 1000.0, 400.0));
}

#[test]
fn moving_into_a_row_with_room() {
    let mut s = make_strip(&[400.0, 300.0, 300.0]); // rows: [1] [2] [3]
    s.set_lone_width(1, Some(0.5)); // 1 takes the left half of its row
    s.set_lone_width(3, Some(0.4));
    // 3 (0.4) moves up past 2, which is full width: no room there.
    assert!(!s.join_if_fits(3, -1));
    s.move_vertical(3, -1); // [1] [3] [2]
                            // Now the row above holds 1 with half the row free: 3 joins it, on the right.
    s.set_lone_offset(3, 0.6);
    assert!(s.join_if_fits(3, -1));
    assert_eq!(rows(&s), vec![vec![1, 3], vec![2]]);
    // They share the row 0.5 : 0.4 (990 px after the gap).
    let f = frames(&s, 0.0);
    assert_eq!(f[&1], frame(10.0, 50.0, 550.0, 400.0));
    assert_eq!(f[&3], frame(570.0, 50.0, 440.0, 400.0));
    // Moving on up leaves the row again (into a row of its own above it).
    assert!(!s.join_if_fits(3, -1));
    s.move_vertical(3, -1);
    assert_eq!(rows(&s), vec![vec![3], vec![1], vec![2]]);
    // A full-width window never fits anywhere.
    assert!(!s.join_if_fits(2, -1));
}

#[test]
fn join_rows() {
    let mut s = make_strip(&[300.0, 300.0, 300.0]);
    assert!(s.join_vertical(2, -1));
    assert_eq!(rows(&s), vec![vec![1, 2], vec![3]]);
    assert!(s.join_vertical(1, 1));
    assert_eq!(rows(&s), vec![vec![2], vec![1, 3]]);
    assert!(!s.join_vertical(2, -1));
    assert!(s.join_vertical(2, 1));
    assert_eq!(rows(&s), vec![vec![2, 1, 3]]);
}

#[test]
fn remove_focuses_neighbor() {
    let mut s = make_strip(&[300.0, 300.0, 300.0]);
    s.focused = Some(2);
    assert_eq!(s.remove(2), Some(3));
    assert_eq!(s.focused, Some(3));
    assert_eq!(s.remove(3), Some(1));
}

#[test]
fn move_and_neighbors() {
    let mut s = make_strip(&[300.0, 300.0, 300.0]);
    assert!(s.move_vertical(1, 1));
    assert_eq!(s.window_ids(), vec![2, 1, 3]);
    assert!(!s.move_vertical(3, 1));
    assert_eq!(s.neighbor(1, Direction::Down), Some(3));
    assert_eq!(s.neighbor(1, Direction::Up), Some(2));
    assert_eq!(s.neighbor(1, Direction::Left), None);
    assert_eq!(s.neighbor(2, Direction::Up), None);
}

#[test]
fn reveal_and_snap() {
    let mut s = make_strip(&[495.0, 495.0, 495.0, 495.0]);
    // Row 3 (top 1010) needs scrolling to 1010+495-1000 = 505.
    assert_eq!(s.scroll_to_reveal(3, VH, 10.0), 505.0);
    s.scroll_y = 900.0;
    assert_eq!(s.scroll_to_reveal(2, VH, 10.0), 505.0);
    // Snap targets are row tops, capped at max scroll (content 2010 - 1000).
    assert_eq!(s.snap_targets(VH, 10.0), vec![0.0, 505.0, 1010.0]);
    assert_eq!(s.snap_target(300.0, 0.0, VH, 10.0), 505.0);
    assert_eq!(s.snap_target(200.0, 0.0, VH, 10.0), 0.0);
    // A flick always advances at least one panel in its direction.
    assert_eq!(s.snap_target(30.0, 900.0, VH, 10.0), 505.0);
    assert_eq!(s.snap_target(480.0, -900.0, VH, 10.0), 0.0);
}

#[test]
fn free_scroll_snaps_only_near_edges() {
    let s = make_strip(&[495.0, 495.0, 495.0, 495.0]); // content 2010, max scroll 1010
                                                       // Released far from any edge: stays where the glide ends.
    assert_eq!(s.resting_scroll(250.0, 0.0, VH, 10.0, 60.0, 1.0), 250.0);
    // Close to row 1's top (505): snaps to it.
    assert_eq!(s.resting_scroll(470.0, 0.0, VH, 10.0, 60.0, 1.0), 505.0);
    // Momentum carries it on: 100 + 1000*0.25 = 350, then free.
    assert_eq!(s.resting_scroll(100.0, 1000.0, VH, 10.0, 60.0, 1.0), 350.0);
    // Never past the ends.
    assert_eq!(s.resting_scroll(900.0, 4000.0, VH, 10.0, 60.0, 1.0), 1010.0);
    // Bottom edges count too: row 2 bottom (1010+495) on the viewport bottom = 505.
    assert!(s.edge_targets(VH, 10.0).contains(&505.0));
    assert_eq!(s.resting_scroll(300.0, 0.0, VH, 10.0, 0.0, 1.0), 300.0);
}

#[test]
fn cycle_height() {
    let mut s = make_strip(&[495.0]);
    let ratios = [1.0 / 3.0, 0.5, 2.0 / 3.0, 1.0];
    assert_eq!(s.cycled_height(1, &ratios, VH, 10.0, true), Some(663.0));
    s.rows[0].height = 1000.0;
    assert_eq!(s.cycled_height(1, &ratios, VH, 10.0, true), Some(327.0));
    assert_eq!(s.cycled_height(1, &ratios, VH, 10.0, false), Some(663.0));
}

#[test]
fn insert_below_focused() {
    let mut s = make_strip(&[300.0, 300.0]);
    s.insert(9, 300.0, Some(1), InsertPosition::Below);
    assert_eq!(s.window_ids(), vec![1, 9, 2]);
    s.insert(8, 300.0, Some(2), InsertPosition::Above);
    assert_eq!(s.window_ids(), vec![1, 9, 8, 2]);
}

#[test]
fn top_visible_row() {
    let s = make_strip(&[495.0, 495.0, 495.0]);
    assert_eq!(s.top_visible_row(505.0, VH, 10.0), Some(1));
    assert_eq!(s.top_visible_row(300.0, VH, 10.0), Some(1));
}

#[test]
fn move_to_edge_and_horizontal() {
    let mut s = make_strip(&[300.0, 300.0, 300.0]);
    s.move_to_edge(3, true);
    assert_eq!(rows(&s), vec![vec![3], vec![1], vec![2]]);
    s.join_vertical(2, -1); // [3] [1, 2]
    assert!(s.move_horizontal(2, -1));
    assert_eq!(rows(&s), vec![vec![3], vec![2, 1]]);
    assert!(!s.move_horizontal(2, -1));
    s.move_to_edge(2, false);
    assert_eq!(rows(&s), vec![vec![3], vec![1], vec![2]]);
}
