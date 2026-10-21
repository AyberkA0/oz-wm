use smithay::{
    delegate_xdg_shell,
    desktop::{find_popup_root_surface, get_popup_toplevel_coords, PopupKind, Window},
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::{wl_seat, wl_surface::WlSurface},
    },
    utils::Serial,
    wayland::{
        compositor::with_states,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState, XdgToplevelSurfaceData,
        },
    },
};

use crate::Oz;

impl XdgShellHandler for Oz {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    /// A new window: a row of its own next to the focused one (where `newWindowPosition` says),
    /// focused and scrolled into view.
    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let id = self.add_window(Window::new_wayland_window(surface));
        let height = self.new_row_height();
        let anchor = self.strip.focused;
        self.strip.insert(id, height, anchor, self.settings.new_window_position);
        self.layout_dirty = true;
        // Sizes the window before its first configure goes out (on its first commit).
        self.apply_layout();
        self.focus(Some(id));
        self.reveal_focused();
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let Some(id) = self.id_of(surface.wl_surface()) else { return };
        let was_focused = self.strip.focused == Some(id);
        let neighbor = self.strip.remove(id);
        if let Some(window) = self.windows.remove(&id) {
            self.space.unmap_elem(&window);
        }
        if was_focused {
            self.focus(neighbor);
        }
        let vh = self.viewport_height();
        let clamped = self.strip.clamp_scroll(self.strip.scroll_y, vh, self.settings.gap);
        self.scroll_to(clamped);
        self.layout_dirty = true;
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        if let Err(e) = self.popups.track_popup(PopupKind::Xdg(surface)) {
            tracing::warn!("popup not tracked: {e}");
        }
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    /// Dragging a window by its title bar. In Oz this picks it up to drop it elsewhere in the
    /// strip (`Strip::drop_slot` has the logic); the grab that does it is on the roadmap.
    fn move_request(&mut self, _surface: ToplevelSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}

    /// Resizing from an edge: changes the row height or the window's width share. On the roadmap.
    fn resize_request(
        &mut self,
        _surface: ToplevelSurface,
        _seat: wl_seat::WlSeat,
        _serial: Serial,
        _edges: xdg_toplevel::ResizeEdge,
    ) {
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}
}
delegate_xdg_shell!(Oz);

/// On every commit: the first configure of new toplevels and popups.
pub fn handle_commit(state: &mut Oz, surface: &WlSurface) {
    if let Some(window) = state.windows.values().find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface)) {
        let initial_configure_sent = with_states(surface, |states| {
            states.data_map.get::<XdgToplevelSurfaceData>().unwrap().lock().unwrap().initial_configure_sent
        });
        if !initial_configure_sent {
            window.toplevel().unwrap().send_configure();
        }
    }

    state.popups.commit(surface);
    if let Some(PopupKind::Xdg(ref xdg)) = state.popups.find_popup(surface) {
        if !xdg.is_initial_configure_sent() {
            // The initial configure is always allowed.
            xdg.send_configure().expect("initial configure failed");
        }
    }
}

impl Oz {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else { return };
        let Some(window) = self.windows.values().find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == &root)) else {
            return;
        };
        let (Some(output_geo), Some(window_geo)) = (self.scroll_area(), self.space.element_geometry(window)) else {
            return;
        };
        // The target geometry is relative to the popup's parent.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;
        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}
