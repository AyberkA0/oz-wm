//! Where a dragged window lands, and scrolling the strip under a drag.

use std::collections::HashMap;

use crate::geometry::{Point, Rect};
use crate::strip::{LayoutParams, Strip, WinId};

/// Where in the strip a window dropped in the scroll area goes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropSlot {
    /// A row of its own just above the row holding this window (`None`: after the last row).
    Row { before: Option<WinId> },
    /// In this window's row, on its right or left.
    Beside { anchor: WinId, right: bool },
}

impl Strip {
    /// The slot a window dragged to `p` (inside the scroll area) would take, and the split line
    /// that shows it: beside a window when the cursor is near its left or right edge (a quarter
    /// of its width, at most 160 px), otherwise between the rows above or below the cursor
    /// (above the row under it in its upper half). `None` when it would land where it already
    /// is. `line` is the thickness of the split line.
    pub fn drop_slot(
        &self,
        p: Point,
        dragging: WinId,
        lp: &LayoutParams,
        min_heights: &HashMap<WinId, f64>,
        line: f64,
    ) -> Option<(DropSlot, Rect)> {
        let vp = lp.viewport;
        let gap = lp.gap;
        let t = line;
        if self.rows.is_empty() {
            return Some((DropSlot::Row { before: None }, Rect::new(vp.x, vp.y, vp.w, t)));
        }
        let frames: HashMap<WinId, Rect> =
            self.layout(lp, self.render_scroll_y, min_heights).into_iter().map(|item| (item.id, item.frame)).collect();
        let top = |i: usize| vp.min_y() - self.render_scroll_y + self.row_top(i, gap);
        let bottom = |i: usize| top(i) + self.rows[i].height;
        let (row_x0, row_width) = Strip::row_extent(lp);
        // The row under the cursor (or the nearest one).
        let r = (0..self.rows.len()).find(|&i| p.y < bottom(i) + gap / 2.0).unwrap_or(self.rows.len() - 1);
        let own = self.locate(dragging);

        // Near a window's left/right edge: beside it.
        let windows = &self.rows[r].windows;
        let hit = windows
            .iter()
            .position(|w| frames.get(w).is_some_and(|f| p.x >= f.min_x() - gap && p.x <= f.max_x() + gap));
        if let Some(c) = hit.filter(|&c| windows[c] != dragging) {
            let f = frames[&windows[c]];
            let edge = (f.w * 0.25).min(160.0);
            let right = if p.x < f.min_x() + edge {
                Some(false)
            } else if p.x > f.max_x() - edge {
                Some(true)
            } else {
                None
            };
            if let Some(right) = right {
                // Already right there (the neighbour on that side is the dragged window): no move.
                let neighbour = if right { c as isize + 1 } else { c as isize - 1 };
                let noop = own.is_some_and(|(orow, ocol)| orow == r && ocol as isize == neighbour);
                if !noop {
                    let x = if right { f.max_x() + gap / 2.0 } else { f.min_x() - gap / 2.0 };
                    let y0 = top(r).max(vp.min_y());
                    let y1 = bottom(r).min(vp.max_y());
                    return Some((
                        DropSlot::Beside { anchor: windows[c], right },
                        Rect::new((x - t / 2.0).round(), y0, t, (y1 - y0).max(t)),
                    ));
                }
            }
        }

        // Between rows: above this row when the cursor is in its upper half, else below it.
        let index = if p.y < (top(r) + bottom(r)) / 2.0 { r } else { r + 1 };
        let boundary = |k: usize| if k == 0 { top(0) - gap / 2.0 } else { bottom(k - 1) + gap / 2.0 };
        // Landing where it already is: no line.
        if let Some((orow, _)) = own {
            if self.rows[orow].windows.len() == 1 && (index == orow || index == orow + 1) {
                return None;
            }
        }
        let anchor = self.rows.get(index).and_then(|row| row.windows.iter().copied().find(|&w| w != dragging));
        let y = boundary(index).max(vp.min_y() + t / 2.0).min(vp.max_y() - t / 2.0);
        Some((DropSlot::Row { before: anchor }, Rect::new(row_x0, (y - t / 2.0).round(), row_width, t)))
    }
}

/// Scrolling the strip while a window is dragged over the scroll area: only the bands along
/// its top and bottom edges (15% of its height, at least 60 px) scroll, faster the deeper the
/// cursor is in them, from 0.15 to 1.5 viewport heights a second.
pub struct DragScroll;

impl DragScroll {
    /// `(velocity, in_band)`: velocity in px/s (positive = down), 0 where it can't scroll
    /// further that way; `in_band` false in the middle of the area.
    pub fn velocity(cursor_y: f64, zone: Rect, vh: f64, scroll: f64, max_scroll: f64) -> (f64, bool) {
        let band = (zone.h * 0.15).max(60.0);
        let (depth, dir) = if cursor_y - zone.min_y() < band {
            (1.0 - (cursor_y - zone.min_y()) / band, -1.0)
        } else if zone.max_y() - cursor_y < band {
            (1.0 - (zone.max_y() - cursor_y) / band, 1.0)
        } else {
            return (0.0, false);
        };
        if (dir < 0.0 && scroll <= 0.0) || (dir > 0.0 && scroll >= max_scroll) {
            return (0.0, true);
        }
        // A gentle start at the band's inner border, quick at the very edge.
        (dir * vh * (0.15 + 1.35 * depth.clamp(0.0, 1.0).powf(1.5)), true)
    }
}
