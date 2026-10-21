//! Keyboard shortcuts, the pointer and touchpad swipes.
//!
//! Shortcuts use Super where the macOS version uses Control-Option (`⌃⌥`), with the same keys.

use oz_core::{Direction, WinId};
use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, GestureBeginEvent, GestureEndEvent,
        GestureSwipeUpdateEvent, InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent,
        PointerButtonEvent, PointerMotionEvent,
    },
    input::{
        keyboard::{FilterResult, Keysym},
        pointer::{AxisFrame, ButtonEvent, MotionEvent},
    },
    utils::{Logical, Point, SERIAL_COUNTER},
};

use crate::{config, state::Swipe, Oz};

/// What a shortcut does (the macOS version's command names in comments).
#[derive(Clone, Copy, Debug)]
pub enum Action {
    Focus(Direction),  // focusLeft / Down / Up / Right
    Move(Direction),   // moveLeft / Down / Up / Right
    JoinAbove,         // joinUp
    JoinBelow,         // joinDown
    SlurpIn,           // slurpIn
    BarfOut,           // barfOut
    CycleWidth(bool),  // cycleWidth
    Equalize,          // equalizeRow
    CycleHeight(bool), // cycleHeight
    Center,            // center
    AlignTop,          // alignTop
    ScrollHalf(f64),   // scrollDown / scrollUp
    FirstRow,          // focusFirst
    LastRow,           // focusLast
    GoToRow(usize),    // focusRow n
    Terminal,          // newTerminal
    Close,             // close
    Quit,
}

fn action_for(sym: Keysym, shift: bool) -> Option<Action> {
    use Direction::*;
    let dir = |d| if shift { Action::Move(d) } else { Action::Focus(d) };
    Some(match sym {
        Keysym::h | Keysym::Left => dir(Left),
        Keysym::j | Keysym::Down => dir(Down),
        Keysym::k | Keysym::Up => dir(Up),
        Keysym::l | Keysym::Right => dir(Right),
        Keysym::bracketleft => Action::JoinAbove,
        Keysym::bracketright => Action::JoinBelow,
        Keysym::i => Action::SlurpIn,
        Keysym::o => Action::BarfOut,
        Keysym::w => Action::CycleWidth(!shift),
        Keysym::equal => Action::Equalize,
        Keysym::r => Action::CycleHeight(!shift),
        Keysym::c => Action::Center,
        Keysym::t => Action::AlignTop,
        Keysym::d => Action::ScrollHalf(0.5),
        Keysym::u => Action::ScrollHalf(-0.5),
        Keysym::Home => Action::FirstRow,
        Keysym::End => Action::LastRow,
        Keysym::Return => Action::Terminal,
        Keysym::q if shift => Action::Close,
        Keysym::e if shift => Action::Quit,
        s if (Keysym::_1.raw()..=Keysym::_9.raw()).contains(&s.raw()) => {
            Action::GoToRow((s.raw() - Keysym::_1.raw()) as usize)
        }
        _ => return None,
    })
}

impl Oz {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let pressed = event.state() == KeyState::Pressed;
                let keyboard = self.seat.get_keyboard().unwrap();
                let action = keyboard.input::<Action, _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |_, mods, handle| {
                        if !mods.logo {
                            return FilterResult::Forward;
                        }
                        match handle.raw_latin_sym_or_raw_current_sym().and_then(|s| action_for(s, mods.shift)) {
                            // The release of an intercepted press is intercepted too (no action).
                            Some(a) if pressed => FilterResult::Intercept(a),
                            _ => FilterResult::Forward,
                        }
                    },
                );
                if let Some(action) = action {
                    self.run(action);
                }
            }
            InputEvent::PointerMotion { event } => {
                let pointer = self.seat.get_pointer().unwrap();
                let pos = pointer.current_location() + event.delta();
                self.pointer_moved(self.clamp_to_output(pos), event.time_msec());
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let Some(area) = self.scroll_area() else { return };
                let pos = event.position_transformed(area.size) + area.loc.to_f64();
                self.pointer_moved(pos, event.time_msec());
            }
            InputEvent::PointerButton { event } => {
                let pointer = self.seat.get_pointer().unwrap();
                let serial = SERIAL_COUNTER.next_serial();
                if event.state() == ButtonState::Pressed && !pointer.is_grabbed() {
                    // Click to focus. Nothing is raised: tiled windows don't overlap.
                    let id = self.window_under(pointer.current_location());
                    if id.is_some() && id != self.strip.focused {
                        self.focus(id);
                        self.reveal_focused();
                    }
                }
                pointer.button(
                    self,
                    &ButtonEvent { button: event.button_code(), state: event.state(), serial, time: event.time_msec() },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event } => self.pointer_axis::<I>(event),
            InputEvent::GestureSwipeBegin { event } => {
                if self.settings.swipe.enabled && event.fingers() == self.settings.swipe.fingers {
                    self.swipe = Some(Swipe { last_time: event.time_msec(), velocity: 0.0 });
                }
            }
            InputEvent::GestureSwipeUpdate { event } => {
                let Some(swipe) = self.swipe.as_mut() else { return };
                let sw = &self.settings.swipe;
                // Natural: fingers up (negative delta) move the strip up, showing what's below.
                let dy = event.delta_y() * sw.sensitivity * if sw.natural { -1.0 } else { 1.0 };
                let dt = (event.time_msec().wrapping_sub(swipe.last_time)).max(1) as f64 / 1000.0;
                swipe.velocity = swipe.velocity * 0.6 + (dy / dt) * 0.4;
                swipe.last_time = event.time_msec();
                let vh = self.viewport_height();
                let s = &mut self.strip;
                // A little give past the ends, like rubber.
                let max = s.max_scroll(vh, self.settings.gap);
                let next = s.render_scroll_y + dy;
                s.render_scroll_y = if next < 0.0 || next > max { s.render_scroll_y + dy * 0.3 } else { next };
                s.scroll_y = s.render_scroll_y;
                self.layout_dirty = true;
            }
            InputEvent::GestureSwipeEnd { event } => {
                let Some(swipe) = self.swipe.take() else { return };
                let velocity = if event.cancelled() { 0.0 } else { swipe.velocity };
                let target = self.swipe_rest(velocity);
                self.scroll_to(target);
                if self.settings.swipe.focus {
                    let vh = self.viewport_height();
                    let row = self.strip.top_visible_row(target, vh, self.settings.gap);
                    let id = row.and_then(|r| self.strip.rows[r].windows.first().copied());
                    if id.is_some() {
                        self.focus(id);
                    }
                }
            }
            _ => {}
        }
    }

    fn clamp_to_output(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
        let Some(area) = self.scroll_area() else { return pos };
        let (x0, y0) = (area.loc.x as f64, area.loc.y as f64);
        let (x1, y1) = (x0 + area.size.w as f64 - 1.0, y0 + area.size.h as f64 - 1.0);
        (pos.x.clamp(x0, x1), pos.y.clamp(y0, y1)).into()
    }

    fn pointer_moved(&mut self, pos: Point<f64, Logical>, time: u32) {
        let pointer = self.seat.get_pointer().unwrap();
        let under = self.surface_under(pos);
        pointer.motion(self, under, &MotionEvent { location: pos, serial: SERIAL_COUNTER.next_serial(), time });
        pointer.frame(self);
    }

    /// Scroll wheel / two-finger scroll goes to the window under the pointer; with Super held it
    /// scrolls the strip instead.
    fn pointer_axis<I: InputBackend>(&mut self, event: I::PointerAxisEvent) {
        let amount = |axis| event.amount(axis).unwrap_or_else(|| event.amount_v120(axis).unwrap_or(0.0) * 15.0 / 120.0);
        let (h, v) = (amount(Axis::Horizontal), amount(Axis::Vertical));
        let super_held = self.seat.get_keyboard().unwrap().modifier_state().logo;
        if super_held {
            let target = self.strip.scroll_y + v * 2.0;
            self.scroll_to(target);
            return;
        }
        let source = event.source();
        let mut frame = AxisFrame::new(event.time_msec()).source(source);
        if h != 0.0 {
            frame = frame.value(Axis::Horizontal, h);
            if let Some(d) = event.amount_v120(Axis::Horizontal) {
                frame = frame.v120(Axis::Horizontal, d as i32);
            }
        }
        if v != 0.0 {
            frame = frame.value(Axis::Vertical, v);
            if let Some(d) = event.amount_v120(Axis::Vertical) {
                frame = frame.v120(Axis::Vertical, d as i32);
            }
        }
        if source == AxisSource::Finger {
            if event.amount(Axis::Horizontal) == Some(0.0) {
                frame = frame.stop(Axis::Horizontal);
            }
            if event.amount(Axis::Vertical) == Some(0.0) {
                frame = frame.stop(Axis::Vertical);
            }
        }
        let pointer = self.seat.get_pointer().unwrap();
        pointer.axis(self, frame);
        pointer.frame(self);
    }

    /// Runs a shortcut's action on the focused window.
    pub fn run(&mut self, action: Action) {
        let gap = self.settings.gap;
        let vh = self.viewport_height();
        let focused = self.strip.focused;
        let changed = match (action, focused) {
            (Action::Terminal, _) => {
                self.spawn(&config::terminal());
                false
            }
            (Action::Quit, _) => {
                self.loop_signal.stop();
                false
            }
            (Action::ScrollHalf(f), _) => {
                self.scroll_to(self.strip.scroll_y + vh * f);
                false
            }
            (Action::FirstRow, _) => self.focus_row(0),
            (Action::LastRow, _) => self.focus_row(self.strip.rows.len().saturating_sub(1)),
            (Action::GoToRow(n), _) => self.focus_row(n),
            (_, None) => false,
            (Action::Focus(d), Some(id)) => {
                if let Some(next) = self.strip.neighbor(id, d) {
                    self.focus(Some(next));
                    self.reveal_focused();
                }
                false
            }
            (Action::Move(d), Some(id)) => match d {
                Direction::Up | Direction::Down => {
                    let delta = if d == Direction::Down { 1 } else { -1 };
                    self.strip.join_if_fits(id, delta) || self.strip.move_vertical(id, delta)
                }
                Direction::Left => self.strip.move_horizontal(id, -1),
                Direction::Right => self.strip.move_horizontal(id, 1),
            },
            (Action::JoinAbove, Some(id)) => self.strip.join_vertical(id, -1),
            (Action::JoinBelow, Some(id)) => self.strip.join_vertical(id, 1),
            (Action::SlurpIn, Some(id)) => self.strip.slurp_in(id),
            (Action::BarfOut, Some(id)) => self.strip.barf_out(id),
            (Action::CycleWidth(forward), Some(id)) => self.cycle_width(id, forward),
            (Action::Equalize, Some(id)) => {
                if self.strip.rows.iter().any(|r| r.windows.len() == 1 && r.windows[0] == id) {
                    self.strip.set_lone_width(id, None);
                } else {
                    self.strip.equalize_row(id);
                }
                true
            }
            (Action::CycleHeight(forward), Some(id)) => {
                match self.strip.cycled_height(id, &self.settings.heights, vh, gap, forward) {
                    Some(h) => {
                        let (r, _) = self.strip.locate(id).unwrap();
                        self.strip.rows[r].height = h;
                        true
                    }
                    None => false,
                }
            }
            (Action::Center, Some(id)) => {
                self.scroll_to(self.strip.scroll_to_center(id, vh, gap));
                false
            }
            (Action::AlignTop, Some(id)) => {
                self.scroll_to(self.strip.scroll_to_top(id, gap));
                false
            }
            (Action::Close, Some(id)) => {
                if let Some(t) = self.windows.get(&id).and_then(|w| w.toplevel()) {
                    t.send_close();
                }
                false
            }
        };
        if changed {
            self.layout_dirty = true;
            self.reveal_focused();
        }
    }

    fn focus_row(&mut self, row: usize) -> bool {
        if let Some(&id) = self.strip.rows.get(row).and_then(|r| r.windows.first()) {
            self.focus(Some(id));
            self.reveal_focused();
        }
        false
    }

    /// Width presets: the share of a shared row, or a lone window's own width (1/3, 1/2, 2/3,
    /// full).
    fn cycle_width(&mut self, id: WinId, forward: bool) -> bool {
        let widths = self.settings.widths.clone();
        let Some((r, _)) = self.strip.locate(id) else { return false };
        if self.strip.rows[r].windows.len() > 1 {
            return match self.strip.cycled_width_fraction(id, &widths, forward) {
                Some(f) => self.strip.set_width_fraction(id, f),
                None => false,
            };
        }
        let mut presets = widths;
        presets.push(1.0);
        presets.sort_by(f64::total_cmp);
        let cur = self.strip.weight(id).min(1.0);
        let next = if forward {
            presets.iter().copied().find(|&x| x > cur + 0.01).unwrap_or(presets[0])
        } else {
            presets.iter().rev().copied().find(|&x| x < cur - 0.01).unwrap_or(*presets.last().unwrap())
        };
        self.strip.set_lone_width(id, Some(next));
        true
    }
}
