mod app_state;
mod config;
mod drawing;
mod stats;

use crate::config::CONFIG;
use lyon::math::vector;
use osm::*;
use winit::{
    dpi::LogicalPosition,
    event::{
        ElementState, Event, KeyboardInput, MouseButton, MouseScrollDelta, VirtualKeyCode,
        WindowEvent,
    },
    event_loop::ControlFlow,
};

fn main() {
    log::set_max_level(CONFIG.general.log_level.to_level_filter());
    pretty_env_logger::init();

    let z = 8.0;
    let tile_coordinate = deg2num(47.3769, 8.5417, z as u32);
    let zurich = num_to_global_space(&tile_coordinate);

    let width = 1600;
    let height = 1000;

    let event_loop = winit::event_loop::EventLoop::new();
    let hdpi_factor = event_loop
        .available_monitors()
        .next()
        .expect("No monitors found")
        .scale_factor();

    let mut app_state =
        app_state::AppState::new("config/style.css", zurich, width, height, z, hdpi_factor);

    let mut painter = drawing::Painter::init(&event_loop, width, height, &app_state);
    let mut hud = drawing::ui::HUD::new(&painter.window, &mut painter.device, &mut painter.queue);

    let mut mouse_down = false;
    let mut last_pos = winit::dpi::LogicalPosition::new(0.0, 0.0);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = if cfg!(feature = "metal-auto-capture") {
            ControlFlow::Exit
        } else {
            ControlFlow::Poll
        };
        let (route_mouse, route_keyboard) = hud.interact(&painter.window, &event);
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::Destroyed => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::Resized(physical_size) => {
                    app_state.screen.width = physical_size.width;
                    app_state.screen.height = physical_size.height;
                    painter.resize(physical_size.width, physical_size.height);
                }
                WindowEvent::KeyboardInput {
                    input:
                        KeyboardInput {
                            virtual_keycode: Some(keycode),
                            ..
                        },
                    ..
                } => {
                    if route_keyboard {
                        match keycode {
                            VirtualKeyCode::Escape => {
                                *control_flow = ControlFlow::Exit;
                            }
                            VirtualKeyCode::Tab => app_state.advance_selected_object(),
                            _ => {}
                        }
                    }
                }
                WindowEvent::CloseRequested => {
                    *control_flow = ControlFlow::Exit;
                }
                WindowEvent::MouseInput { state, button, .. } => {
                    if route_mouse {
                        if let MouseButton::Left = button {
                            match state {
                                ElementState::Pressed => {
                                    mouse_down = true;
                                }
                                ElementState::Released => {
                                    mouse_down = false;
                                    app_state.update_selected_hover_objects();
                                }
                            }
                        }
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    if route_mouse {
                        match delta {
                            MouseScrollDelta::LineDelta(_, y) => app_state.zoom += 0.1 * y,
                            MouseScrollDelta::PixelDelta(LogicalPosition { y, .. }) => {
                                app_state.zoom += 0.001 * y as f32
                            }
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let logical_position = position.to_logical(painter.get_hidpi_factor());
                    let size = app_state.screen.get_tile_size() as f32;
                    let mut delta = vector(
                        (logical_position.x - last_pos.x) as f32,
                        (logical_position.y - last_pos.y) as f32,
                    );
                    let zoom_x = (app_state.screen.width as f32)
                        / size
                        / 2f32.powf(app_state.zoom)
                        / size
                        / 2.0
                        / 1.3;
                    let zoom_y = (app_state.screen.height as f32)
                        / size
                        / 2f32.powf(app_state.zoom)
                        / size
                        / 2.0
                        / 1.3;
                    delta.x *= zoom_x;
                    delta.y *= zoom_y;

                    last_pos = logical_position;

                    if route_mouse {
                        if mouse_down {
                            app_state.screen.center -= delta;
                        }

                        app_state.update_hovered_objects((
                            logical_position.x as f32,
                            logical_position.y as f32,
                        ))
                    }
                }
                _ => (),
            },
            Event::MainEventsCleared => {
                painter.update_shader();
                app_state.load_tiles();
                painter.paint(&mut hud, &mut app_state);

                app_state.stats.capture_frame();
                if CONFIG.general.display_framerate {
                    println!(
                        "Frametime {:.2} at zoom {:.2}",
                        app_state.stats.get_average(),
                        app_state.zoom
                    );
                }
            }
            _ => (),
        }
    });
}
