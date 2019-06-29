mod vector_tile;
mod drawing;
mod app_state;
mod css;

extern crate lyon;
extern crate nalgebra_glm as glm;

use crate::vector_tile::*;
use lyon::math::{
    vector,
};

fn main() {
    pretty_env_logger::init();

    let z = 8.0;
    let tile_coordinate = math::deg2num(47.3769, 8.5417, z as u32);
    let zurich = math::num_to_global_space(&tile_coordinate);

    let size = 600;

    let mut events_loop = wgpu::winit::EventsLoop::new();

    let mut painter = drawing::Painter::init(&events_loop, size, size);

    let mut app_state = app_state::AppState::new("config/style.css", zurich.clone(), size, size, z);

    let mut status = true;
    let mut mouse_down = false;
    let mut last_pos = wgpu::winit::dpi::LogicalPosition::new(0.0, 0.0);

    loop {
        use wgpu::winit::{Event, WindowEvent, ElementState, MouseButton, MouseScrollDelta, KeyboardInput, VirtualKeyCode};
        events_loop.poll_events(|event| {
            match event {
                Event::WindowEvent {
                    event: WindowEvent::Resized(size),
                    ..
                } => {
                    let physical = size.to_physical(painter.get_hidpi_factor());
                    app_state.screen.width = physical.width.round() as u32;
                    app_state.screen.height = physical.height.round() as u32;
                    painter.resize(physical.width.round() as u32, physical.height.round() as u32);
                },
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Destroyed => { status = false }
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput { virtual_keycode: Some(VirtualKeyCode::Escape), .. },
                        ..
                    } | WindowEvent::CloseRequested => { status = false },
                    WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                        mouse_down = true;
                    },
                    WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                        mouse_down = false;
                    },
                    WindowEvent::MouseWheel { delta, .. } => {
                        match delta {
                            MouseScrollDelta::LineDelta(_x, y) => app_state.zoom += 0.1 * y,
                            _ => ()
                        }
                    },
                    WindowEvent::CursorMoved { position, .. } => {
                        let mut delta = vector((position.x - last_pos.x) as f32, (position.y - last_pos.y) as f32);
                        let zoom_x = (app_state.screen.width as f32) / 256.0 / 2f32.powf(app_state.zoom) / 256.0 / 2.0 / 1.3;
                        let zoom_y = (app_state.screen.height as f32) / 256.0 / 2f32.powf(app_state.zoom) / 256.0 / 2.0 / 1.3;
                        delta.x *= zoom_x;
                        delta.y *= zoom_y;

                        last_pos = position;
                        if mouse_down {
                            app_state.screen.center -= delta;
                        }
                    }
                    _ => (),
                }
                _ => (),
            }
        });

        painter.update_shader();
        painter.update_styles(app_state.zoom, &mut app_state.css_cache);
        // let t = std::time::Instant::now();
        painter.load_tiles(&mut app_state);
        // dbg!(t.elapsed().as_micros());
        // let t = std::time::Instant::now();
        painter.paint(&app_state);

        // Frame by frame stepping.
        // match std::io::stdin().read_line(&mut String::new()) {
        //     Ok(_goes_into_input_above) => {},
        //     Err(_no_updates_is_fine) => {},
        // }

        if !status {
            break;
        }
    }
}