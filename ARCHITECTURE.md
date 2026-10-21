# Architecture

## Two crates

`oz-core` holds every decision about where windows go. It knows nothing about Wayland: it works on window ids (`WinId`), rectangles in logical pixels (origin top left, y down) and plain data. Everything in it is deterministic and unit-tested, so layout behaviour can be changed and checked with `cargo test -p oz-core` on any machine.

`oz-compositor` is the Wayland side. It owns the Smithay state (protocols, seat, outputs, the `Space` of mapped windows), turns input into `oz-core` calls, and applies the resulting layout to real windows.

```
input (keys, pointer, swipes) ──► Oz::run / gesture handlers ──► Strip (oz-core) edits + scroll target
                                                                       │
frame callback ──► Oz::tick (eases render_scroll_y) ──► Strip::layout ──► configure sizes, map/unmap in Space ──► render
```

## The strip

`Strip` is a list of `Row`s; each row has a height and one or more windows. Rows stack top to bottom with `gap` between them. `scroll_y` is where the view should be, `render_scroll_y` where it is on screen; animation moves the second towards the first. `Strip::layout` turns the strip and a scroll position into a `LayoutItem` per window:

- `Placement::Frame(rect)`: show the window there.
- `Placement::Overflow(rect)`: the window is cut by an edge of the scroll area; draw it whole at `rect`, clipped to the area.
- `Placement::Park`: out of view; don't draw it.

Widths inside a row come from per-window weights (`width_weights`); a window alone in a narrowed row also has an offset (`width_offsets`). Row heights are absolute pixels; presets come from `Settings::heights` as fractions of the viewport.

## Differences from the macOS version

On macOS, Oz is an ordinary app moving other apps' windows through the Accessibility API. It cannot draw a window partly off screen above the menu bar, cannot clip a window to the scroll area, and every move is a slow cross-process request. A large part of that code works around this: snapshot stand-ins drawn while the strip moves, live capture of windows sliding off the top, per-app write queues, checks that a window really landed.

As a compositor, Oz draws every window itself. None of those workarounds are needed:

- Sliding off an edge is just drawing the window at a position outside the scroll area with a clip rectangle. `TopOverflow::Slide` is the default here.
- Scrolling moves where windows are drawn, which is instant. Sizes only change when rows change, so clients only get a configure then.
- Touchpad swipes come from libinput as gesture events (`GestureSwipeBegin/Update/End`). The macOS version's private multitouch recognizer is not needed.
- Window ids are handed out by the compositor; on restart, windows are matched by app id and title (`WindowMatcher`), as on macOS.

## Porting notes

Each `oz-core` module corresponds to a file of the Swift `OzCore`, with the same algorithms and the same test values:

| Rust | Swift |
| --- | --- |
| `strip.rs` | `Strip.swift` |
| `drag_drop.rs` | `DragDrop.swift` |
| `regions.rs` | `OutsideRegions.swift` |
| `state.rs` | `StripState.swift` |
| `preset.rs` | `Preset.swift` |
| `settings.rs` | `Settings.swift` |

`StripState` serializes to the same JSON as the macOS `layout.json` entries. `GestureRecognizer.swift` is not ported (libinput does that job).

The macOS app's `WindowManager.swift` is the reference for behaviour that still has to be built on the compositor side: dragging windows between rows and into regions, mouse resizing, the scroll area editor, undo, presets.
