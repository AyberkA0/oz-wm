# Oz for Wayland

Oz is a vertically scrolling tiling window manager. Windows are arranged in rows on a strip that is taller than the screen, and the screen is a view onto that strip: a three-finger swipe scrolls it up and down. New windows open as a row of their own next to the focused one, nothing is ever resized to make room, and several windows can share a row side by side.

This repository is the Linux port, a Wayland compositor built on [Smithay](https://github.com/Smithay/smithay). The original macOS version is [oz-wm](https://github.com/AyberkA0/oz-wm); its behaviour is the reference for this one.

> Status: early. The layout engine is complete and tested; the compositor runs nested in a window and tiles, scrolls, focuses and rearranges windows from the keyboard and touchpad. See [ROADMAP.md](ROADMAP.md) for what is next.

## Layout

| Crate | What it is |
| --- | --- |
| [`oz-core`](oz-core) | The layout engine, with no windowing system: the strip, its rows and widths, scrolling and snapping, where a dragged window lands, the regions around the scroll area, saved layouts and presets, settings. A line-by-line port of the macOS version's `OzCore`, with its tests. |
| [`oz-compositor`](oz-compositor) | The compositor (binary `oz`): Wayland protocols, input, rendering, and the glue that applies `oz-core`'s layout to real windows. |

[ARCHITECTURE.md](ARCHITECTURE.md) explains how the pieces fit together and how this differs from the macOS version.

## Building

Requires Rust 1.80 or later and, for the compositor, the usual Wayland compositor libraries. On Debian or Ubuntu:

```sh
sudo apt install pkg-config libwayland-dev libxkbcommon-dev libegl1-mesa-dev libgles2-mesa-dev \
  libgbm-dev libdrm-dev libinput-dev libudev-dev libseat-dev
```

On Fedora:

```sh
sudo dnf install wayland-devel libxkbcommon-devel mesa-libEGL-devel mesa-libgbm-devel \
  libdrm-devel libinput-devel systemd-devel libseat-devel
```

Then:

```sh
cargo build --release
cargo test
```

The layout engine builds anywhere, including macOS: `cargo test -p oz-core`.

## Running

From inside an existing Wayland or X11 session:

```sh
cargo run --release -- -c foot
```

This opens Oz in a window and starts `foot` inside it (without `-c`, it starts `$TERMINAL`, or `foot`). Programs started from that terminal open in Oz.

## Using it

Shortcuts use Super where the macOS version uses Control-Option, with the same keys.

| Keys | Action | | Keys | Action |
| --- | --- | --- | --- | --- |
| `Super h j k l` / arrows | focus left / down / up / right | | `Super Shift h j k l` / arrows | move the window |
| `Super 1…9` | go to row n | | `Super Home` / `End` | first / last row |
| `Super [` | join the row above | | `Super ]` | join the row below |
| `Super i` | pull the window below into this row | | `Super o` | move the window out of its row |
| `Super w` / `Super Shift w` | cycle the width | | `Super =` | equal widths / full width |
| `Super r` / `Super Shift r` | cycle the height | | `Super c` / `Super t` | center / align to top |
| `Super u` / `Super d` | scroll half a screen | | `Super Return` | open a terminal |
| `Super Shift q` | close the window | | `Super Shift e` | quit Oz |

A three-finger swipe on a touchpad scrolls the strip (with a DRM/libinput backend; the nested window gets no touchpad gestures from the host), as does the scroll wheel with Super held.

## Configuration

`~/.config/oz/config.json` takes the same settings as the macOS version's `oz.set({...})`:

```json
{
  "gap": 12,
  "outerGap": 12,
  "defaultHeight": 0.5,
  "newWindowPosition": "below",
  "topOverflow": "slide",
  "swipe": { "fingers": 3, "sensitivity": 1.5, "natural": true, "snap": "near" }
}
```

Every key is optional; see [`oz-core/src/settings.rs`](oz-core/src/settings.rs) for the full list and defaults.
