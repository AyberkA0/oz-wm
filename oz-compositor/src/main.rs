//! Oz on Wayland. For now it runs nested, in a window of the session you're in (the winit
//! backend), which is the quickest way to work on it; a DRM/libinput backend that runs on a
//! TTY comes next (see ROADMAP.md).
//!
//! `oz [-c <command>]`: starts the compositor and, inside it, `<command>` (default: a terminal).

mod config;
mod handlers;
mod input;
mod layout;
mod state;
mod winit;

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};

pub use state::Oz;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let mut event_loop: EventLoop<Oz> = EventLoop::try_new()?;
    let display: Display<Oz> = Display::new()?;
    let mut state = Oz::new(&mut event_loop, display, config::load());

    winit::init(&mut event_loop, &mut state)?;

    let mut args = std::env::args().skip(1);
    let command = match (args.next().as_deref(), args.next()) {
        (Some("-c" | "--command"), Some(command)) => command,
        _ => config::terminal(),
    };
    state.spawn(&command);

    event_loop.run(None, &mut state, |_| {})?;
    Ok(())
}
