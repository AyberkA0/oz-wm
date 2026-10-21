use serde::{Deserialize, Serialize};

/// A point in logical pixels, origin at the top left.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// A rectangle in logical pixels, origin at the top left, y growing downwards.
#[derive(Clone, Copy, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn min_x(&self) -> f64 {
        self.x
    }
    pub fn max_x(&self) -> f64 {
        self.x + self.w
    }
    pub fn min_y(&self) -> f64 {
        self.y
    }
    pub fn max_y(&self) -> f64 {
        self.y + self.h
    }
    pub fn mid_x(&self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn mid_y(&self) -> f64 {
        self.y + self.h / 2.0
    }
    pub fn area(&self) -> f64 {
        self.w * self.h
    }
    pub fn is_empty(&self) -> bool {
        self.w <= 0.0 || self.h <= 0.0
    }

    /// Half-open, like the macOS version: the right and bottom edges are outside.
    pub fn contains(&self, p: Point) -> bool {
        p.x >= self.min_x() && p.x < self.max_x() && p.y >= self.min_y() && p.y < self.max_y()
    }

    /// The overlap, `None` when they don't touch (touching edges give a zero-size rectangle).
    pub fn intersection(&self, o: &Rect) -> Option<Rect> {
        let x0 = self.min_x().max(o.min_x());
        let y0 = self.min_y().max(o.min_y());
        let x1 = self.max_x().min(o.max_x());
        let y1 = self.max_y().min(o.max_y());
        if x1 < x0 || y1 < y0 {
            return None;
        }
        Some(Rect::new(x0, y0, x1 - x0, y1 - y0))
    }

    pub fn inset(&self, dx: f64, dy: f64) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w - 2.0 * dx, self.h - 2.0 * dy)
    }

    /// The rectangle `t` of the way (0...1) to `o`, on whole pixels (for sliding windows).
    pub fn interpolated(&self, o: &Rect, t: f64) -> Rect {
        let mix = |a: f64, b: f64| (a + (b - a) * t).round();
        Rect::new(mix(self.x, o.x), mix(self.y, o.y), mix(self.w, o.w), mix(self.h, o.h))
    }
}
