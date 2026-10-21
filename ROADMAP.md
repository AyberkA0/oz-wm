# Roadmap

Where the Wayland port stands against the macOS version, and what to build next, roughly in order.

## Feature parity

| Feature | macOS | Wayland |
| --- | --- | --- |
| Rows, side-by-side windows, width shares | yes | yes (`oz-core`, keyboard) |
| Keyboard focus and moves, join/slurp/barf | yes | yes |
| Cycle height / width, equalize, center, align top | yes | yes |
| Animated scrolling | yes | yes |
| Animated window moves (rows sliding into place) | yes | not yet |
| Three-finger swipe with glide and snapping | yes | code in place; needs the libinput backend to receive gestures |
| Scroll area per desktop, with editor | yes | not yet (whole output) |
| Clipping at the scroll area's edges | snapshots | not yet (needs a clip in the render pass) |
| Drag a window to a split line / beside a window | yes | logic in `oz-core` (`drop_slot`), grab not yet |
| Regions around the scroll area | yes | logic in `oz-core` (`OutsideRegions`), compositor side not yet |
| Liquid Glass drop preview and split line | yes | not yet (draw as solid render elements first) |
| Mouse resizing of rows and widths | yes | not yet |
| Maximize / fill the screen | yes | not yet |
| Undo | yes | not yet (`StripState` snapshots) |
| Saved layout across restarts | yes | not yet (`StripState` + `WindowMatcher` ready) |
| Desktop presets | yes | not yet (`Preset` ready) |
| Multiple workspaces / outputs | yes | not yet (one strip) |
| Floating windows, window rules | yes | not yet |
| Configuration | `config.js` | `config.json` (settings only, no scripting) |
| Settings window | yes | out of scope for now |

## Next steps

1. **TTY backend.** A udev/DRM/libinput backend (Smithay's `backend_udev`, `backend_drm`, `backend_libinput`, `backend_session_libseat`) so Oz runs as a real session and receives touchpad gestures. Smithay's Anvil example has a complete one to follow.
2. **Clipping.** Render windows through a clip to the scroll area so `Overflow` windows slide off cleanly (wrap the surface render elements, or use a cropped element).
3. **Animated moves.** Interpolate each window's frame from where it was to where `layout` puts it (`Rect::interpolated`), as `WindowManager.visualFrames` does on macOS.
4. **Dragging.** A pointer grab started from `move_request`: the window follows the pointer, `Strip::drop_slot` gives the split line, the drop inserts it. Autoscroll at the edges with `DragScroll::velocity`.
5. **Mouse resizing.** From `resize_request`: bottom edge sets the row height, side edges the width share (`set_width_fraction`, `set_lone_width`, `set_lone_offset`).
6. **Workspaces and outputs.** One `Strip` per (workspace, output), switching, moving windows between them.
7. **Persistence.** Save `StripState` per workspace to `$XDG_STATE_HOME/oz/layout.json` and restore with `WindowMatcher::find`; presets with `find_loosely`.
8. **Scroll area and regions.** Per-workspace scroll area (a simple editor overlay), outside regions with `OutsideRegions::compute` / `frames`.
9. **Undo, maximize, floating, rules.**
10. **Protocols for a usable desktop:** layer-shell (bars, launchers), xdg-decoration (prefer server-side off), xwayland, screencopy, idle inhibit, fractional scale.
