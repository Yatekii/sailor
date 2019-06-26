mod vector_tile;
mod drawing;
mod app_state;
mod css;

extern crate lyon;

use crate::vector_tile::*;
use lyon::math::{
    vector,
};
use crate::drawing::vertex::vertex;
use crate::vector_tile::math::TileId;

fn main() {
    pretty_env_logger::init();

    let z = 8;
    let tile_id = math::deg2tile(47.3769, 8.5417, z);
    let tile_coordinate = math::deg2num(47.3769, 8.5417, z);
    dbg!(tile_id);
    let zurich = math::num_to_global_space(&tile_coordinate);

    let size = 600;

    let mut events_loop = wgpu::winit::EventsLoop::new();

    let mut painter = drawing::Painter::init(&events_loop, size, size);

    let mut app_state = app_state::AppState::new("config/style.css", zurich.clone(), size, size, z);

    dbg!(zurich);
    let mut status = true;
    let mut mouse_down = false;
    let mut last_pos = wgpu::winit::dpi::LogicalPosition::new(0.0, 0.0);

    loop {
        use wgpu::winit::{Event, WindowEvent, ElementState, MouseButton, KeyboardInput, VirtualKeyCode};
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
                    WindowEvent::CursorMoved { position, .. } => {
                        let mut delta = vector(0.0, 0.0);
                        delta.x = (position.x - last_pos.x) as f32;
                        delta.y = (position.y - last_pos.y) as f32;
                        
                        let world_to_px = 1.0 / 2.0f32.powi(z as i32) / size as f32 * 2.0;
                        delta *= world_to_px;

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

        painter.load_tiles(&mut app_state);
        painter.update_uniforms(&mut app_state);
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