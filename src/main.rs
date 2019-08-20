mod vector_tile;
mod drawing;
mod app_state;
mod interaction;
mod css;
mod stats;
mod config;

extern crate lyon;
extern crate nalgebra_glm as glm;
#[macro_use] extern crate lazy_static;
#[macro_use] extern crate serde_derive;


use crate::vector_tile::*;
use lyon::math::{
    vector,
};

use crate::config::CONFIG;

fn main() {
    log::set_max_level(CONFIG.general.log_level.to_level_filter());
    pretty_env_logger::init();

    let z = 8.0;
    let tile_coordinate = math::deg2num(47.3769, 8.5417, z as u32);
    let zurich = math::num_to_global_space(&tile_coordinate);

    let width = 1600;
    let height = 1000;

    let mut events_loop = wgpu::winit::EventsLoop::new();
    let hdpi_factor = events_loop.get_available_monitors().next().expect("No monitors found").get_hidpi_factor();

    let mut app_state = app_state::AppState::new("config/style.css", zurich.clone(), width, height, z, hdpi_factor);

    let mut painter = drawing::Painter::init(&events_loop, width, height, &app_state);
    let mut hud = drawing::ui::HUD::new(&painter.window, &mut painter.device);

    let mut status = true;
    let mut mouse_down = false;
    let mut last_pos = wgpu::winit::dpi::LogicalPosition::new(0.0, 0.0);

    loop {
        use wgpu::winit::{Event, WindowEvent, ElementState, MouseButton, MouseScrollDelta, KeyboardInput, VirtualKeyCode};
        events_loop.poll_events(|event| {
            let route = hud.interact(&painter.window, &event);
            if !route {
                match event.clone() {
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
                            input: KeyboardInput { virtual_keycode: Some(keycode), .. },
                            ..
                        } => {
                            match keycode {
                                VirtualKeyCode::Escape => status = false,
                                VirtualKeyCode::Tab => app_state.advance_selected_object(),
                                _ => {}
                            }
                        },
                        WindowEvent::CloseRequested => { status = false },
                        WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                            mouse_down = true;
                        },
                        WindowEvent::MouseInput { state: ElementState::Released, button: MouseButton::Left, .. } => {
                            mouse_down = false;
                            app_state.update_selected_hover_objects();
                        },
                        WindowEvent::MouseWheel { delta, .. } => {
                            match delta {
                                MouseScrollDelta::LineDelta(_x, y) => app_state.zoom += 0.1 * y,
                                _ => ()
                            }
                        },
                        WindowEvent::CursorMoved { position, .. } => {
                            let size = app_state.screen.get_tile_size() as f32;
                            let mut delta = vector((position.x - last_pos.x) as f32, (position.y - last_pos.y) as f32);
                            let zoom_x = (app_state.screen.width as f32) / size / 2f32.powf(app_state.zoom) / size / 2.0 / 1.3;
                            let zoom_y = (app_state.screen.height as f32) / size / 2f32.powf(app_state.zoom) / size / 2.0 / 1.3;
                            delta.x *= zoom_x;
                            delta.y *= zoom_y;

                            last_pos = position;
                            if mouse_down {
                                app_state.screen.center -= delta;
                            }

                            app_state.update_hovered_objects((position.x as f32, position.y as f32))
                        }
                        _ => (),
                    }
                    _ => (),
                }
            }
        });

        painter.update_shader();
        painter.update_styles(app_state.zoom.max(14.0), &mut app_state.css_cache);
        painter.paint(&mut hud, &mut app_state);

        app_state.stats.capture_frame();
        if CONFIG.general.display_framerate {
            println!("Frametime {:.2} at zoom {:.2}", app_state.stats.get_average(), app_state.zoom);
        }

        if !status {
            break;
        }
    }
}