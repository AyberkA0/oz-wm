//! The platform-independent heart of Oz: windows on an endless vertical strip of rows, the
//! viewport scrolling over it, and everything that is pure geometry and bookkeeping (where a
//! dragged window lands, the regions around the scroll area, saved layouts and presets).
//!
//! Nothing here talks to a display server. The compositor (`oz-compositor`) feeds it window
//! ids and sizes and applies the frames it computes. Coordinates are logical pixels with the
//! origin at the top left of the output layout, y growing downwards.
//!
//! Ported from the macOS version's `OzCore` (Swift), behaviour and tests included.

pub mod drag_drop;
pub mod geometry;
pub mod preset;
pub mod regions;
pub mod settings;
pub mod state;
pub mod strip;

pub use drag_drop::{DragScroll, DropSlot};
pub use geometry::{Point, Rect};
pub use preset::{Preset, PresetApp};
pub use regions::OutsideRegions;
pub use settings::{DoubleTapSettings, InsertPosition, Settings, SnapMode, SwipeSettings, TopOverflow};
pub use state::{StateRow, StateWindow, StripState, WindowDescription, WindowMatcher};
pub use strip::{Direction, LayoutItem, LayoutParams, Placement, Row, Strip, WinId};
