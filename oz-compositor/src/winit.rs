//! Running nested in a window of the current session (Wayland or X11), for development.

use std::time::{Duration, Instant};

use smithay::{
    backend::{
        renderer::{damage::OutputDamageTracker, element::surface::WaylandSurfaceRenderElement, gles::GlesRenderer},
        winit::{self, WinitEvent},
    },
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::EventLoop,
    utils::{Rectangle, Transform},
};

use crate::Oz;

pub fn init(event_loop: &mut EventLoop<Oz>, state: &mut Oz) -> Result<(), Box<dyn std::error::Error>> {
    let (mut backend, winit) = winit::init()?;
    let mode = Mode { size: backend.window_size(), refresh: 60_000 };
    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Oz".into(),
            model: "Winit".into(),
        },
    );
    let _global = output.create_global::<Oz>(&state.display_handle);
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));

    let mut damage_tracker = OutputDamageTracker::from_output(&output);
    let mut last_frame = Instant::now();

    event_loop.handle().insert_source(winit, move |event, _, state| match event {
        WinitEvent::Resized { size, .. } => {
            output.change_current_state(Some(Mode { size, refresh: 60_000 }), None, None, None);
            // New viewport: rows keep their heights, the view stays within the strip.
            let target = state.strip.scroll_y;
            state.scroll_to(target);
            state.layout_dirty = true;
        }
        WinitEvent::Input(event) => state.process_input_event(event),
        WinitEvent::Redraw => {
            let now = Instant::now();
            state.tick((now - last_frame).as_secs_f64().min(0.1));
            last_frame = now;

            let size = backend.window_size();
            let damage = Rectangle::from_size(size);
            {
                let (renderer, mut framebuffer) = backend.bind().unwrap();
                smithay::desktop::space::render_output::<_, WaylandSurfaceRenderElement<GlesRenderer>, _, _>(
                    &output,
                    renderer,
                    &mut framebuffer,
                    1.0,
                    0,
                    [&state.space],
                    &[],
                    &mut damage_tracker,
                    [0.07, 0.09, 0.08, 1.0],
                )
                .unwrap();
            }
            backend.submit(Some(&[damage])).unwrap();

            state.space.elements().for_each(|window| {
                window
                    .send_frame(&output, state.start_time.elapsed(), Some(Duration::ZERO), |_, _| Some(output.clone()))
            });
            state.space.refresh();
            state.popups.cleanup();
            let _ = state.display_handle.flush_clients();
            backend.window().request_redraw();
        }
        WinitEvent::CloseRequested => state.loop_signal.stop(),
        _ => (),
    })?;
    Ok(())
}
