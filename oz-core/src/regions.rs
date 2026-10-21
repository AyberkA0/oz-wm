//! The regions around the scroll area that windows dropped outside it are tiled in.

use crate::geometry::Rect;

/// Splits the part of the output outside the scroll area into rectangles.
///
/// The outside is a frame around the area with up to four sides (left, right, top, bottom).
/// Each side becomes one rectangle, which is the fewest possible; what remains to choose is
/// which side each corner square belongs to (the strip above/below or the one beside it).
/// Of those choices we keep the one with the longest rectangle (then the second longest, and
/// so on, then the largest area). Sides too thin to hold a window are left out.
pub struct OutsideRegions;

impl OutsideRegions {
    pub const MIN_THICKNESS: f64 = 150.0;

    pub fn compute(o: Rect, zone: Rect, min_thickness: f64) -> Vec<Rect> {
        let Some(z) = zone.intersection(&o).filter(|z| !z.is_empty()) else { return Vec::new() };
        let left = z.min_x() - o.min_x();
        let right = o.max_x() - z.max_x();
        let top = z.min_y() - o.min_y();
        let bottom = o.max_y() - z.max_y();
        let (has_l, has_r) = (left >= min_thickness, right >= min_thickness);
        let (has_t, has_b) = (top >= min_thickness, bottom >= min_thickness);

        let mut best: Vec<Rect> = Vec::new();
        let mut best_score: Vec<f64> = Vec::new();
        // A corner is a choice only between two strips that both exist; otherwise it goes to
        // the one that does (or stays uncovered).
        // Order: top-left, top-right, bottom-left, bottom-right.
        let choice = [has_l && has_t, has_r && has_t, has_l && has_b, has_r && has_b];
        for mask in 0..16u32 {
            // Bit set: that corner goes to the vertical strip (left/right) instead of top/bottom.
            let bit = |i: u32| mask & (1 << i) != 0;
            // Skip masks that set a non-choice bit.
            if (0..4).any(|i| bit(i) && !choice[i as usize]) {
                continue;
            }
            // true: the corner belongs to the vertical strip beside it.
            let tl_side = has_l && (!has_t || bit(0));
            let tr_side = has_r && (!has_t || bit(1));
            let bl_side = has_l && (!has_b || bit(2));
            let br_side = has_r && (!has_b || bit(3));
            let mut rects = Vec::with_capacity(4);
            if has_l {
                let y0 = if tl_side { o.min_y() } else { z.min_y() };
                let y1 = if bl_side { o.max_y() } else { z.max_y() };
                rects.push(Rect::new(o.min_x(), y0, left, y1 - y0));
            }
            if has_r {
                let y0 = if tr_side { o.min_y() } else { z.min_y() };
                let y1 = if br_side { o.max_y() } else { z.max_y() };
                rects.push(Rect::new(z.max_x(), y0, right, y1 - y0));
            }
            if has_t {
                let x0 = if tl_side { z.min_x() } else { o.min_x() };
                let x1 = if tr_side { z.max_x() } else { o.max_x() };
                rects.push(Rect::new(x0, o.min_y(), x1 - x0, top));
            }
            if has_b {
                let x0 = if bl_side { z.min_x() } else { o.min_x() };
                let x1 = if br_side { z.max_x() } else { o.max_x() };
                rects.push(Rect::new(x0, z.max_y(), x1 - x0, bottom));
            }
            let mut score: Vec<f64> = rects.iter().map(|r| r.w.max(r.h)).collect();
            score.sort_by(|a, b| b.total_cmp(a));
            score.push(rects.iter().map(Rect::area).sum());
            if best.is_empty() || precedes_descending(&score, &best_score) {
                best = rects;
                best_score = score;
            }
        }
        // Stable order: top to bottom, then left to right.
        best.sort_by(|a, b| a.min_y().total_cmp(&b.min_y()).then(a.min_x().total_cmp(&b.min_x())));
        best
    }

    /// Frames for `count` windows sharing `region`, side by side along its long side (stacked
    /// when it's taller than wide), with `gap` between them and around them.
    pub fn frames(region: Rect, count: usize, gap: f64) -> Vec<Rect> {
        if count == 0 {
            return Vec::new();
        }
        let r = region.inset(gap, gap);
        let vertical = r.h >= r.w;
        let total = (if vertical { r.h } else { r.w }) - gap * (count as f64 - 1.0);
        let end = if vertical { r.max_y() } else { r.max_x() };
        let mut pos = if vertical { r.min_y() } else { r.min_x() };
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            // The last one takes what's left so rounding never leaves a gap.
            let len = if i == count - 1 { end - pos } else { (total / count as f64).round() };
            out.push(if vertical { Rect::new(r.x, pos, r.w, len) } else { Rect::new(pos, r.y, len, r.h) });
            pos += len + gap;
        }
        out
    }
}

/// `a` comes before `b` when compared element by element with "greater" first (Swift's
/// `lexicographicallyPrecedes(by: >)`).
fn precedes_descending(a: &[f64], b: &[f64]) -> bool {
    for (x, y) in a.iter().zip(b) {
        if x > y {
            return true;
        }
        if y > x {
            return false;
        }
    }
    a.len() < b.len()
}
