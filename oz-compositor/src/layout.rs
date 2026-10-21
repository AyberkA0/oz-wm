//! Applying the strip's layout to the windows, and moving the view.

use std::collections::HashMap;

use oz_core::{LayoutParams, Placement, Rect, SnapMode, Strip, WinId};
use smithay::{
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel,
    utils::{Logical, Rectangle},
};

use crate::Oz;

impl Oz {
    /// The scroll area: the whole output for now (setting it per workspace is on the roadmap).
    pub fn scroll_area(&self) -> Option<Rectangle<i32, Logical>> {
        let output = self.space.outputs().next()?;
        self.space.output_geometry(output)
    }

    pub fn layout_params(&self) -> Option<LayoutParams> {
        let area = self.scroll_area()?;
        let s = &self.settings;
        let area = Rect::new(area.loc.x as f64, area.loc.y as f64, area.size.w as f64, area.size.h as f64);
        let mut p = LayoutParams::new(area.inset(s.outer_gap, s.outer_gap), area.min_y(), area.max_y(), s.gap);
        p.width_ratio = s.width;
        p.top_overflow = s.top_overflow;
        p.min_visible = s.min_visible;
        Some(p)
    }

    pub fn viewport_height(&self) -> f64 {
        self.layout_params().map_or(0.0, |p| p.viewport.h)
    }

    /// Height of a new row, from the settings.
    pub fn new_row_height(&self) -> f64 {
        let vh = self.viewport_height().max(200.0);
        Strip::height_for_ratio(self.settings.default_height, vh, self.settings.gap)
    }

    /// Puts every window where the strip says, for the scroll position on screen
    /// (`render_scroll_y`). Windows out of view are unmapped from the space (still alive, just
    /// not drawn); windows cut off by an edge are drawn whole and clipped by the output.
    pub fn apply_layout(&mut self) {
        let Some(p) = self.layout_params() else { return };
        self.layout_dirty = false;
        let mins: HashMap<WinId, f64> = HashMap::new();
        for item in self.strip.layout(&p, self.strip.render_scroll_y, &mins) {
            let Some(window) = self.windows.get(&item.id).cloned() else { continue };
            let frame = match item.placement {
                Placement::Frame(r) => r,
                Placement::Overflow(r) => r,
                Placement::Park => {
                    self.space.unmap_elem(&window);
                    continue;
                }
            };
            if let Some(toplevel) = window.toplevel() {
                toplevel.with_pending_state(|state| {
                    state.size = Some((frame.w as i32, frame.h as i32).into());
                    for s in [
                        xdg_toplevel::State::TiledLeft,
                        xdg_toplevel::State::TiledRight,
                        xdg_toplevel::State::TiledTop,
                        xdg_toplevel::State::TiledBottom,
                    ] {
                        state.states.set(s);
                    }
                });
                if toplevel.is_initial_configure_sent() {
                    toplevel.send_pending_configure();
                }
            }
            // The window's own geometry may start inside its surface (client-side shadows).
            let offset = window.geometry().loc;
            self.space.map_element(window, (frame.x as i32 - offset.x, frame.y as i32 - offset.y), false);
        }
    }

    /// Scrolls the view to `target` (clamped), animated by `tick`.
    pub fn scroll_to(&mut self, target: f64) {
        let vh = self.viewport_height();
        self.strip.scroll_y = self.strip.clamp_scroll(target, vh, self.settings.gap);
    }

    /// Brings the focused window fully into view.
    pub fn reveal_focused(&mut self) {
        if let Some(id) = self.strip.focused {
            let target = self.strip.scroll_to_reveal(id, self.viewport_height(), self.settings.gap);
            self.scroll_to(target);
        }
    }

    /// One frame of motion: the view eases towards `scroll_y`. Returns whether anything moved.
    pub fn tick(&mut self, dt: f64) -> bool {
        let s = &mut self.strip;
        let moving = (s.scroll_y - s.render_scroll_y).abs() > 0.5;
        if moving && self.swipe.is_none() {
            // Critically damped approach: about `animation_duration` to get there.
            let k = if self.settings.animation_duration <= 0.0 {
                1.0
            } else {
                1.0 - (-dt * 4.0 / self.settings.animation_duration).exp()
            };
            s.render_scroll_y += (s.scroll_y - s.render_scroll_y) * k;
            if (s.scroll_y - s.render_scroll_y).abs() <= 0.5 {
                s.render_scroll_y = s.scroll_y;
            }
        }
        if moving || self.layout_dirty {
            self.apply_layout();
            return true;
        }
        false
    }

    /// Where a swipe released with `velocity` (px/s) comes to rest, by the snap setting.
    pub fn swipe_rest(&self, velocity: f64) -> f64 {
        let vh = self.viewport_height();
        let gap = self.settings.gap;
        let s = &self.strip;
        let sw = &self.settings.swipe;
        match sw.snap {
            SnapMode::Always => s.snap_target(s.render_scroll_y, velocity, vh, gap),
            SnapMode::Near => s.resting_scroll(s.render_scroll_y, velocity, vh, gap, sw.snap_distance, sw.momentum),
            SnapMode::Off => s.resting_scroll(s.render_scroll_y, velocity, vh, gap, 0.0, sw.momentum),
        }
    }
}
