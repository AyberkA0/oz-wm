use std::{collections::HashMap, ffi::OsString, sync::Arc, time::Instant};

use oz_core::{Settings, Strip, WinId};
use smithay::{
    desktop::{PopupManager, Space, Window, WindowSurfaceType},
    input::{Seat, SeatState},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle,
        },
    },
    utils::{Logical, Point, SERIAL_COUNTER},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::xdg::XdgShellState,
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

/// A swipe in progress: scroll position follows the fingers, velocity is kept for the glide.
pub struct Swipe {
    pub last_time: u32,
    pub velocity: f64,
}

pub struct Oz {
    pub start_time: Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,
    pub loop_signal: LoopSignal,

    pub settings: Settings,
    /// The strip of the (only, for now) workspace. Ids are ours, handed out per toplevel.
    pub strip: Strip,
    pub windows: HashMap<WinId, Window>,
    next_id: WinId,
    pub swipe: Option<Swipe>,
    /// The layout has to be applied again (a window came or went, a row changed size...).
    pub layout_dirty: bool,

    pub space: Space<Window>,
    pub popups: PopupManager,
    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Oz>,
    pub data_device_state: DataDeviceState,
    pub seat: Seat<Oz>,
}

impl Oz {
    pub fn new(event_loop: &mut EventLoop<Oz>, display: Display<Oz>, settings: Settings) -> Self {
        let dh = display.handle();
        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "seat0");
        seat.add_keyboard(Default::default(), 200, 25).expect("no keyboard");
        seat.add_pointer();

        let socket_name = Self::listen(display, event_loop);
        tracing::info!("listening on {}", socket_name.to_string_lossy());

        Self {
            start_time: Instant::now(),
            socket_name,
            display_handle: dh,
            loop_signal: event_loop.get_signal(),
            settings,
            strip: Strip::new(),
            windows: HashMap::new(),
            next_id: 1,
            swipe: None,
            layout_dirty: false,
            space: Space::default(),
            popups: PopupManager::default(),
            compositor_state,
            xdg_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            seat,
        }
    }

    fn listen(display: Display<Oz>, event_loop: &mut EventLoop<Oz>) -> OsString {
        let socket = ListeningSocketSource::new_auto().expect("no wayland socket");
        let name = socket.socket_name().to_os_string();
        let handle = event_loop.handle();
        handle
            .insert_source(socket, |stream, _, state| {
                if let Err(e) = state.display_handle.insert_client(stream, Arc::new(ClientState::default())) {
                    tracing::warn!("could not add a client: {e}");
                }
            })
            .expect("could not listen for clients");
        handle
            .insert_source(Generic::new(display, Interest::READ, Mode::Level), |_, display, state| {
                // Safety: the display is never dropped while the loop runs.
                unsafe {
                    display.get_mut().dispatch_clients(state).unwrap();
                }
                Ok(PostAction::Continue)
            })
            .expect("could not watch the display");
        name
    }

    /// Runs a command in the compositor's session.
    pub fn spawn(&self, command: &str) {
        let mut parts = command.split_whitespace();
        let Some(program) = parts.next() else { return };
        if let Err(e) =
            std::process::Command::new(program).args(parts).env("WAYLAND_DISPLAY", &self.socket_name).spawn()
        {
            tracing::warn!("could not start {command}: {e}");
        }
    }

    // Windows

    pub fn add_window(&mut self, window: Window) -> WinId {
        let id = self.next_id;
        self.next_id += 1;
        self.windows.insert(id, window);
        id
    }

    pub fn id_of(&self, surface: &WlSurface) -> Option<WinId> {
        self.windows.iter().find(|(_, w)| w.toplevel().is_some_and(|t| t.wl_surface() == surface)).map(|(&id, _)| id)
    }

    /// Gives a window keyboard focus (and the activated look); `None` clears it.
    pub fn focus(&mut self, id: Option<WinId>) {
        self.strip.focused = id;
        for (&wid, w) in &self.windows {
            if w.set_activated(Some(wid) == id) {
                if let Some(t) = w.toplevel() {
                    t.send_pending_configure();
                }
            }
        }
        let surface =
            id.and_then(|id| self.windows.get(&id)).and_then(|w| w.toplevel()).map(|t| t.wl_surface().clone());
        let keyboard = self.seat.get_keyboard().unwrap();
        keyboard.set_focus(self, surface, SERIAL_COUNTER.next_serial());
    }

    pub fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(WlSurface, Point<f64, Logical>)> {
        self.space.element_under(pos).and_then(|(window, location)| {
            window
                .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                .map(|(s, p)| (s, (p + location).to_f64()))
        })
    }

    pub fn window_under(&self, pos: Point<f64, Logical>) -> Option<WinId> {
        let (window, _) = self.space.element_under(pos)?;
        self.windows.iter().find(|(_, w)| *w == window).map(|(&id, _)| id)
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
