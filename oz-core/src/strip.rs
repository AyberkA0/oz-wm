//! The strip: rows of windows stacked from top to bottom, scrolled under a viewport.

use std::collections::HashMap;

use crate::geometry::Rect;
use crate::settings::{InsertPosition, TopOverflow};

/// Window identifier, handed out by the compositor.
pub type WinId = u32;

/// A horizontal row of the strip. Usually holds one window; `slurp_in` and drops beside a
/// window put several side by side, sharing the row's width.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub windows: Vec<WinId>,
    pub height: f64,
}

impl Row {
    pub fn new(windows: Vec<WinId>, height: f64) -> Self {
        Self { windows, height }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Placement {
    /// Put the window at this frame.
    Frame(Rect),
    /// The window is out of view: don't show it (unmap or skip rendering it).
    Park,
    /// The window's full frame sticks out past the scroll area's top (or, with `cut_bottom`,
    /// bottom) edge. On Wayland the compositor draws it there and clips it to the area.
    Overflow(Rect),
}

/// One window's place in a layout pass.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutItem {
    pub id: WinId,
    pub placement: Placement,
    /// Full height of the window's row (a shrunk window is shorter than this).
    pub row_height: f64,
    /// Where the whole window would be, ignoring the edges.
    pub frame: Rect,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutParams {
    /// Area windows are laid out in (the scroll area minus its outer gaps).
    pub viewport: Rect,
    /// Highest y a window may be drawn at (the top of the scroll area).
    pub hard_top: f64,
    /// Windows whose top is below this are out of view.
    pub hard_bottom: f64,
    /// The scroll area ends above the screen's bottom: windows are cut off at `hard_bottom`
    /// like at the top, instead of running off the screen.
    pub cut_bottom: bool,
    pub gap: f64,
    pub width_ratio: f64,
    pub top_overflow: TopOverflow,
    pub min_visible: f64,
}

impl LayoutParams {
    /// Defaults as in the macOS version: full width rows, shrink at the edges, 80 px minimum.
    pub fn new(viewport: Rect, hard_top: f64, hard_bottom: f64, gap: f64) -> Self {
        Self {
            viewport,
            hard_top,
            hard_bottom,
            cut_bottom: false,
            gap,
            width_ratio: 1.0,
            top_overflow: TopOverflow::Shrink,
            min_visible: 80.0,
        }
    }

    /// Lowest y anything may be drawn at (unbounded when windows may run off the screen).
    pub fn clip_bottom(&self) -> f64 {
        if self.cut_bottom {
            self.hard_bottom
        } else {
            f64::MAX
        }
    }
}

/// A vertically scrolling strip of rows: one per (workspace, output).
#[derive(Clone, Debug, Default)]
pub struct Strip {
    pub rows: Vec<Row>,
    /// Logical scroll position (where we want to be), in pixels from the top of the strip.
    pub scroll_y: f64,
    /// Scroll position currently rendered (differs from `scroll_y` while animating).
    pub render_scroll_y: f64,
    pub focused: Option<WinId>,
    /// Width share of each window (default 1). Windows sharing a row split it in proportion
    /// to their shares; a window alone in its row with a share below 1 takes only that
    /// fraction of the row and leaves the rest empty.
    pub width_weights: HashMap<WinId, f64>,
    /// Where a narrowed window alone in its row sits: the space left of it, as a fraction of
    /// the row's width (default 0, the left edge). Set by resizing it from either side.
    pub width_offsets: HashMap<WinId, f64>,
}

impl Strip {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn window_ids(&self) -> Vec<WinId> {
        self.rows.iter().flat_map(|r| r.windows.iter().copied()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    pub fn contains(&self, id: WinId) -> bool {
        self.locate(id).is_some()
    }

    /// (row, column) of a window.
    pub fn locate(&self, id: WinId) -> Option<(usize, usize)> {
        self.rows.iter().enumerate().find_map(|(r, row)| row.windows.iter().position(|&w| w == id).map(|c| (r, c)))
    }

    // Geometry

    pub fn row_top(&self, index: usize, gap: f64) -> f64 {
        self.rows.iter().take(index).map(|r| r.height + gap).sum()
    }

    pub fn content_height(&self, gap: f64) -> f64 {
        if self.rows.is_empty() {
            0.0
        } else {
            self.row_top(self.rows.len(), gap) - gap
        }
    }

    pub fn max_scroll(&self, vh: f64, gap: f64) -> f64 {
        (self.content_height(gap) - vh).max(0.0)
    }

    pub fn height_for_ratio(ratio: f64, vh: f64, gap: f64) -> f64 {
        // Two 1/2 rows plus the gap between them fill the viewport exactly.
        (ratio * (vh + gap) - gap).round().max(40.0)
    }

    // Editing

    pub fn insert(&mut self, id: WinId, height: f64, anchor: Option<WinId>, position: InsertPosition) {
        let anchor_row = anchor.and_then(|a| self.locate(a)).map(|(r, _)| r);
        let index = match position {
            InsertPosition::End => self.rows.len(),
            InsertPosition::Start => 0,
            InsertPosition::Below => anchor_row.map_or(self.rows.len(), |r| r + 1),
            InsertPosition::Above => anchor_row.unwrap_or(self.rows.len()),
        };
        self.rows.insert(index, Row::new(vec![id], height));
    }

    /// Inserts `id` as a row of its own just above the row holding `anchor` (at the end
    /// when there is no anchor). Used when a window is dropped between two rows.
    pub fn insert_row(&mut self, id: WinId, height: f64, before: Option<WinId>) {
        let index = before.and_then(|a| self.locate(a)).map_or(self.rows.len(), |(r, _)| r);
        self.rows.insert(index, Row::new(vec![id], height));
    }

    /// Puts `id` next to `anchor` in its row, on its right or left, sharing the row's width.
    pub fn insert_beside(&mut self, id: WinId, anchor: WinId, right: bool) -> bool {
        let Some((r, c)) = self.locate(anchor) else { return false };
        self.rows[r].windows.insert(if right { c + 1 } else { c }, id);
        self.width_weights.remove(&id);
        true
    }

    /// Removes a window; returns the neighbour that should receive focus.
    pub fn remove(&mut self, id: WinId) -> Option<WinId> {
        let (r, c) = self.locate(id)?;
        self.rows[r].windows.remove(c);
        self.width_weights.remove(&id);
        self.width_offsets.remove(&id);
        let neighbor = if self.rows[r].windows.is_empty() {
            self.rows.remove(r);
            if r < self.rows.len() {
                self.rows[r].windows.first().copied()
            } else if r > 0 {
                self.rows[r - 1].windows.first().copied()
            } else {
                None
            }
        } else {
            let w = &self.rows[r].windows;
            Some(w[c.min(w.len() - 1)])
        };
        if self.focused == Some(id) {
            self.focused = neighbor;
        }
        neighbor
    }

    pub fn neighbor(&self, id: WinId, direction: Direction) -> Option<WinId> {
        let (r, c) = self.locate(id)?;
        match direction {
            Direction::Up | Direction::Down => {
                let t = if direction == Direction::Down { r.checked_add(1)? } else { r.checked_sub(1)? };
                let target = &self.rows.get(t)?.windows;
                Some(target[c.min(target.len() - 1)])
            }
            Direction::Left | Direction::Right => {
                let t = if direction == Direction::Right { c.checked_add(1)? } else { c.checked_sub(1)? };
                self.rows[r].windows.get(t).copied()
            }
        }
    }

    /// Moves a window up/down. A window sharing a row is first expelled into its own row.
    pub fn move_vertical(&mut self, id: WinId, delta: isize) -> bool {
        let Some((r, c)) = self.locate(id) else { return false };
        if self.rows[r].windows.len() > 1 {
            self.rows[r].windows.remove(c);
            let height = self.rows[r].height;
            self.rows.insert(if delta > 0 { r + 1 } else { r }, Row::new(vec![id], height));
            return true;
        }
        let t = r as isize + delta;
        if t < 0 || t as usize >= self.rows.len() {
            return false;
        }
        self.rows.swap(r, t as usize);
        true
    }

    /// Moving up/down into a row that has room for this window: the next row holds a single
    /// narrowed window and this one fits in the space beside it. Then it joins that row on the
    /// side it's already on, the two sharing the row in proportion to their widths. False (and
    /// nothing changes) when it doesn't fit; the caller then moves it as usual.
    pub fn join_if_fits(&mut self, id: WinId, delta: isize) -> bool {
        let Some((r, c)) = self.locate(id) else { return false };
        let t = r as isize + delta;
        if t < 0 || t as usize >= self.rows.len() {
            return false;
        }
        let mut t = t as usize;
        if self.rows[t].windows.len() != 1 {
            return false;
        }
        let other = self.rows[t].windows[0];
        if self.weight(other) >= 1.0 {
            return false;
        }
        let row = self.rows[r].windows.clone();
        // Its share of the row width now: alone, its own (narrowed) width; shared, its part.
        let share = if row.len() == 1 { self.weight(id).min(1.0) } else { self.width_fraction(id).unwrap_or(1.0) };
        if share > 1.0 - self.weight(other) + 0.02 {
            return false;
        }
        // Which side of the other window it is on (by the middles of the two).
        let mine = if row.len() == 1 {
            self.width_offsets.get(&id).copied().unwrap_or(0.0) + share / 2.0
        } else {
            row[..c].iter().filter_map(|&w| self.width_fraction(w)).sum::<f64>() + share / 2.0
        };
        let right = mine >= self.width_offsets.get(&other).copied().unwrap_or(0.0) + self.weight(other) / 2.0;
        self.rows[r].windows.remove(c);
        if self.rows[r].windows.is_empty() {
            self.rows.remove(r);
            if t > r {
                t -= 1;
            }
        }
        self.width_weights.insert(id, share);
        self.width_offsets.remove(&id);
        self.width_offsets.remove(&other);
        if right {
            self.rows[t].windows.push(id);
        } else {
            self.rows[t].windows.insert(0, id);
        }
        true
    }

    pub fn move_horizontal(&mut self, id: WinId, delta: isize) -> bool {
        let Some((r, c)) = self.locate(id) else { return false };
        let t = c as isize + delta;
        if t < 0 || t as usize >= self.rows[r].windows.len() {
            return false;
        }
        self.rows[r].windows.swap(c, t as usize);
        true
    }

    pub fn move_to_edge(&mut self, id: WinId, top: bool) {
        let Some((r, c)) = self.locate(id) else { return };
        if self.rows[r].windows.len() > 1 {
            self.rows[r].windows.remove(c);
            let row = Row::new(vec![id], self.rows[r].height);
            let at = if top { 0 } else { self.rows.len() };
            self.rows.insert(at, row);
        } else {
            let row = self.rows.remove(r);
            let at = if top { 0 } else { self.rows.len() };
            self.rows.insert(at, row);
        }
    }

    /// Pulls the first window of the row below into this row.
    pub fn slurp_in(&mut self, id: WinId) -> bool {
        let Some((r, _)) = self.locate(id) else { return false };
        if r + 1 >= self.rows.len() {
            return false;
        }
        let w = self.rows[r + 1].windows.remove(0);
        self.width_weights.remove(&w);
        self.rows[r].windows.push(w);
        if self.rows[r + 1].windows.is_empty() {
            self.rows.remove(r + 1);
        }
        true
    }

    /// Moves this window into the row above (appended on the right) or below (on the left),
    /// so it sits side by side with that row's windows.
    pub fn join_vertical(&mut self, id: WinId, delta: isize) -> bool {
        let Some((r, c)) = self.locate(id) else { return false };
        let t = r as isize + delta;
        if t < 0 || t as usize >= self.rows.len() {
            return false;
        }
        let mut target = t as usize;
        self.rows[r].windows.remove(c);
        if self.rows[r].windows.is_empty() {
            self.rows.remove(r);
            if delta > 0 {
                target -= 1;
            }
        }
        if delta < 0 {
            self.rows[target].windows.push(id);
        } else {
            self.rows[target].windows.insert(0, id);
        }
        self.width_weights.remove(&id);
        true
    }

    // Widths within a row

    pub fn weight(&self, id: WinId) -> f64 {
        self.width_weights.get(&id).copied().unwrap_or(1.0)
    }

    /// Share of the row's width this window gets (1 when alone in its row).
    pub fn width_fraction(&self, id: WinId) -> Option<f64> {
        let (r, _) = self.locate(id)?;
        let sum: f64 = self.rows[r].windows.iter().map(|&w| self.weight(w)).sum();
        Some(if sum > 0.0 { self.weight(id) / sum } else { 1.0 })
    }

    /// Gives this window `fraction` of its row; the others keep their proportions.
    pub fn set_width_fraction(&mut self, id: WinId, fraction: f64) -> bool {
        let Some((r, _)) = self.locate(id) else { return false };
        if self.rows[r].windows.len() < 2 {
            return false;
        }
        let f = fraction.clamp(0.1, 0.9);
        let others: f64 = self.rows[r].windows.iter().filter(|&&w| w != id).map(|&w| self.weight(w)).sum();
        self.width_weights.insert(id, f * others / (1.0 - f));
        true
    }

    pub fn cycled_width_fraction(&self, id: WinId, ratios: &[f64], forward: bool) -> Option<f64> {
        let cur = self.width_fraction(id)?;
        if ratios.is_empty() {
            return None;
        }
        let mut sorted = ratios.to_vec();
        sorted.sort_by(f64::total_cmp);
        if forward {
            return sorted.iter().copied().find(|&x| x > cur + 0.01).or(sorted.first().copied());
        }
        sorted.iter().rev().copied().find(|&x| x < cur - 0.01).or(sorted.last().copied())
    }

    /// Sets the share of a window alone in its row (`None` / >= 0.97 means full width). It
    /// keeps its place in the row, moved left only as far as it needs to still fit.
    pub fn set_lone_width(&mut self, id: WinId, share: Option<f64>) {
        match share {
            Some(s) if s < 0.97 => {
                self.width_weights.insert(id, s.max(0.1));
                if let Some(o) = self.width_offsets.get(&id).copied() {
                    self.set_lone_offset(id, o);
                }
            }
            _ => {
                self.width_weights.remove(&id);
                self.width_offsets.remove(&id);
            }
        }
    }

    /// Places a narrowed window alone in its row: `offset` is the space left of it as a
    /// fraction of the row's width, kept within the row.
    pub fn set_lone_offset(&mut self, id: WinId, offset: f64) {
        let o = offset.max(0.0).min((1.0 - self.weight(id)).max(0.0));
        if o < 0.005 {
            self.width_offsets.remove(&id);
        } else {
            self.width_offsets.insert(id, o);
        }
    }

    /// The row's left edge and width for these layout parameters.
    pub fn row_extent(p: &LayoutParams) -> (f64, f64) {
        let row_width = (p.viewport.w * p.width_ratio.clamp(0.1, 1.0)).round();
        ((p.viewport.min_x() + (p.viewport.w - row_width) / 2.0).round(), row_width)
    }

    /// Equal widths for every window in the row of `id`.
    pub fn equalize_row(&mut self, id: WinId) {
        let Some((r, _)) = self.locate(id) else { return };
        for w in self.rows[r].windows.clone() {
            self.width_weights.remove(&w);
        }
    }

    /// Expels this window from its row into a new row below.
    pub fn barf_out(&mut self, id: WinId) -> bool {
        let Some((r, c)) = self.locate(id) else { return false };
        if self.rows[r].windows.len() < 2 {
            return false;
        }
        self.rows[r].windows.remove(c);
        let height = self.rows[r].height;
        self.rows.insert(r + 1, Row::new(vec![id], height));
        self.width_weights.remove(&id);
        true
    }

    /// Next/previous preset height for the row containing `id`.
    pub fn cycled_height(&self, id: WinId, ratios: &[f64], vh: f64, gap: f64, forward: bool) -> Option<f64> {
        let (r, _) = self.locate(id)?;
        if ratios.is_empty() {
            return None;
        }
        let mut heights: Vec<f64> = ratios.iter().map(|&x| Self::height_for_ratio(x, vh, gap)).collect();
        heights.sort_by(f64::total_cmp);
        let cur = self.rows[r].height;
        if forward {
            return heights.iter().copied().find(|&h| h > cur + 2.0).or(heights.first().copied());
        }
        heights.iter().rev().copied().find(|&h| h < cur - 2.0).or(heights.last().copied())
    }

    // Scrolling

    /// Smallest scroll change that brings the row of `id` fully into view.
    pub fn scroll_to_reveal(&self, id: WinId, vh: f64, gap: f64) -> f64 {
        let Some((r, _)) = self.locate(id) else { return self.scroll_y };
        let top = self.row_top(r, gap);
        let h = self.rows[r].height;
        if h >= vh - 0.5 || top < self.scroll_y {
            return top;
        }
        if top + h > self.scroll_y + vh {
            return top + h - vh;
        }
        self.scroll_y
    }

    pub fn scroll_to_center(&self, id: WinId, vh: f64, gap: f64) -> f64 {
        let Some((r, _)) = self.locate(id) else { return self.scroll_y };
        (self.row_top(r, gap) + self.rows[r].height / 2.0 - vh / 2.0).round()
    }

    pub fn scroll_to_top(&self, id: WinId, gap: f64) -> f64 {
        self.locate(id).map_or(self.scroll_y, |(r, _)| self.row_top(r, gap))
    }

    pub fn clamp_scroll(&self, s: f64, vh: f64, gap: f64) -> f64 {
        s.max(0.0).min(self.max_scroll(vh, gap))
    }

    /// Scroll positions a swipe may come to rest at: each row aligned to the top edge (like
    /// turning to the next panel), plus both ends.
    pub fn snap_targets(&self, vh: f64, gap: f64) -> Vec<f64> {
        let m = self.max_scroll(vh, gap);
        let mut t = vec![0.0, m];
        t.extend((0..self.rows.len()).map(|i| self.row_top(i, gap).min(m)));
        sorted_unique(t)
    }

    /// Scroll positions where a window edge lines up with a viewport edge: a row's top on the
    /// top edge, or a row's bottom on the bottom edge.
    pub fn edge_targets(&self, vh: f64, gap: f64) -> Vec<f64> {
        let m = self.max_scroll(vh, gap);
        let mut t = vec![0.0, m];
        for i in 0..self.rows.len() {
            let top = self.row_top(i, gap);
            t.push(top.max(0.0).min(m));
            t.push((top + self.rows[i].height - vh).max(0.0).min(m));
        }
        sorted_unique(t)
    }

    /// Free scrolling with inertia: glide on from `s` with `velocity` (px/s), and settle on a
    /// window edge only if the glide ends within `snap_distance` of one.
    pub fn resting_scroll(&self, s: f64, velocity: f64, vh: f64, gap: f64, snap_distance: f64, momentum: f64) -> f64 {
        let projected = self.clamp_scroll(s + velocity * 0.25 * momentum, vh, gap).round();
        if snap_distance <= 0.0 {
            return projected;
        }
        let nearest = self
            .edge_targets(vh, gap)
            .into_iter()
            .min_by(|a, b| (a - projected).abs().total_cmp(&(b - projected).abs()));
        match nearest {
            Some(n) if (n - projected).abs() <= snap_distance => n,
            _ => projected,
        }
    }

    /// Where a swipe released at `s` with `velocity` (px/s, positive = scrolling down) lands.
    pub fn snap_target(&self, s: f64, velocity: f64, vh: f64, gap: f64) -> f64 {
        let targets = self.snap_targets(vh, gap);
        let projected = s + velocity * 0.2;
        let flick = vh * 0.5;
        let mut candidates = targets.clone();
        if velocity > flick {
            candidates = targets.iter().copied().filter(|&t| t > s + 1.0).collect();
            if candidates.is_empty() {
                candidates = vec![*targets.last().unwrap()];
            }
        } else if velocity < -flick {
            candidates = targets.iter().copied().filter(|&t| t < s - 1.0).collect();
            if candidates.is_empty() {
                candidates = vec![targets[0]];
            }
        }
        candidates.into_iter().min_by(|a, b| (a - projected).abs().total_cmp(&(b - projected).abs())).unwrap()
    }

    /// Topmost row fully visible at scroll `s` (or the row covering the top edge).
    pub fn top_visible_row(&self, s: f64, vh: f64, gap: f64) -> Option<usize> {
        let mut covering = None;
        for i in 0..self.rows.len() {
            let top = self.row_top(i, gap);
            let bottom = top + self.rows[i].height;
            if top >= s - 1.0 && bottom <= s + vh + 1.0 {
                return Some(i);
            }
            if covering.is_none() && bottom > s {
                covering = Some(i);
            }
        }
        covering
    }

    pub fn is_fully_visible(&self, id: WinId, s: f64, vh: f64, gap: f64) -> bool {
        let Some((r, _)) = self.locate(id) else { return false };
        let top = self.row_top(r, gap);
        top >= s - 1.0 && top + self.rows[r].height <= s + vh + 1.0
    }

    // Layout

    /// Computes where every window goes when the strip is scrolled to `scroll`. `min_heights`
    /// holds the smallest height a window accepts (from its xdg-toplevel min size).
    pub fn layout(&self, p: &LayoutParams, scroll: f64, min_heights: &HashMap<WinId, f64>) -> Vec<LayoutItem> {
        let mut out = Vec::with_capacity(self.rows.len());
        let (x0, row_width) = Self::row_extent(p);
        let mut y = p.viewport.min_y() - scroll;
        for row in &self.rows {
            let top = y.round();
            let bottom = top + row.height;
            y += row.height + p.gap;
            let n = row.windows.len();
            let available = row_width - p.gap * (n as f64 - 1.0);
            let weights: Vec<f64> = row.windows.iter().map(|&w| self.weight(w)).collect();
            let total: f64 = weights.iter().sum();
            // A lone, narrowed window keeps its width; the rest of the row stays empty.
            let lone_width = (n == 1 && weights[0] < 1.0).then(|| (row_width * weights[0]).round());
            let mut x = x0;
            if let (Some(lw), Some(o)) = (lone_width, self.width_offsets.get(&row.windows[0])) {
                x = (x0 + (row_width * o).round()).min(x0 + row_width - lw);
            }
            for (i, &id) in row.windows.iter().enumerate() {
                // The last panel takes what's left so rounding never leaves a gap.
                let w = lone_width.unwrap_or(if i == n - 1 {
                    x0 + row_width - x
                } else {
                    (available * weights[i] / total).round()
                });
                let frame = Rect::new(x, top, w, bottom - top);
                out.push(LayoutItem {
                    id,
                    placement: Self::placement(frame, p, min_heights.get(&id).copied()),
                    row_height: row.height,
                    frame,
                });
                x += w + p.gap;
            }
        }
        out
    }

    /// How a window whose full frame is `r` is shown, given the scroll area's edges.
    pub fn placement(r: Rect, p: &LayoutParams, min_height: Option<f64>) -> Placement {
        if r.max_y() <= p.hard_top + 1.0 || r.min_y() >= p.hard_bottom - 1.0 {
            return Placement::Park;
        }
        if r.min_y() < p.hard_top || r.max_y() > p.clip_bottom() + 1.0 {
            if p.top_overflow == TopOverflow::Slide {
                return Placement::Overflow(r);
            }
            let top = r.min_y().max(p.hard_top);
            let visible = r.max_y().min(p.clip_bottom()) - top;
            if p.top_overflow == TopOverflow::Shrink && visible >= p.min_visible.max(min_height.unwrap_or(0.0)) {
                return Placement::Frame(Rect::new(r.x, top, r.w, visible));
            }
            return Placement::Park;
        }
        Placement::Frame(r)
    }
}

fn sorted_unique(mut v: Vec<f64>) -> Vec<f64> {
    v.sort_by(f64::total_cmp);
    v.dedup();
    v
}
