mod vector_tile;
mod render;

#[macro_use]
extern crate glium;
extern crate lyon;

use structopt::StructOpt;

use glium::Surface;
use glium::glutin::dpi::LogicalSize;

use crate::vector_tile::*;
use lyon::math::{
    point,
    vector,
};

#[derive(StructOpt)]
#[structopt(
    name = "Sailor",
    about = "Sailing weather and route tooling",
    author = "Noah Hüsser <yatekii@yatekii.ch>"
)]
enum CLI {
    #[structopt(name = "import_map")]
    ImportMap {
        _path: String,
    },
    #[structopt(name = "inspect_tile")]
    InspectTile {
        _path: String,
    },
}

fn main() {
    // let matches = CLI::from_args();
    // match matches {
    //     CLI::ImportMap { path } => osm::import::region(path),
    //     CLI::InspectTile { path } => tile::vector_tile_to_vbo(path),
    // }

    let z = 8;
    let (x, y) = dbg!(math::deg2tile(47.3769, 8.5417, z));
    let num = math::deg2num(47.3769, 8.5417, z);
    dbg!(num);
    let tile = math::tile_to_global_space(z, x, y, lyon::math::point(0.0, 0.0));
    dbg!(tile);
    let zurich = math::num_to_global_space(z, num.x, num.y);
    dbg!(zurich);
    // let zurich: lyon::math::Point = lyon::math::point(0.525754,0.35115147);
    // let (x, y) = crate::vector_tile::math::deg2num(40.7128, 74.0060, z); // NY

    let data = vector_tile::fetch_tile_data(z, x, y);
    let mut layers = crate::vector_tile::vector_tile_to_mesh(z, x, y, &data);
    layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x+1, y, &vector_tile::fetch_tile_data(z, x+1, y)));
    layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x, y+1, &vector_tile::fetch_tile_data(z, x, y+1)));
    layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x+1, y+1, &vector_tile::fetch_tile_data(z, x+1, y+1)));
    layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x, y+2, &vector_tile::fetch_tile_data(z, x, y+2)));
    layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x+1, y+2, &vector_tile::fetch_tile_data(z, x+1, y+2)));

    let mut events_loop = glium::glutin::EventsLoop::new();
    let context = glium::glutin::ContextBuilder::new().with_vsync(true);
    let window = glium::glutin::WindowBuilder::new()
        .with_dimensions(LogicalSize { width: 600.0, height: 600.0 })
        .with_decorations(true)
        .with_title("lyon + glium basic example");
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    for layer in &mut layers {
        layer.load(&display)
    }
    
    let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();
    let mut pan = zurich.clone() * -1.0;

    let mut mouse_down = false;
    let mut last_pos = glium::glutin::dpi::LogicalPosition::new(0.0, 0.0);

    let mut status = true;
    loop {
        if !status {
            break;
        }

        let mut target = display.draw();
        target.clear_color(0.8, 0.8, 0.8, 1.0);
        for layer in &layers {
            layer.draw(&mut target, &program, pan);
        }

        target.finish().unwrap();

        events_loop.poll_events(|event| {
            use glium::glutin::{Event, WindowEvent, ElementState, MouseButton};
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Destroyed => { status = false }
                    WindowEvent::KeyboardInput {
                        input: glium::glutin::KeyboardInput { virtual_keycode: Some(glium::glutin::VirtualKeyCode::Escape), .. },
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

                        let world_to_px = 1.0 / 2.0f32.powi(z as i32) / 600.0 * 2.0;
                        delta *= world_to_px;

                        println!("{:?}", delta);

                        last_pos = position;
                        if mouse_down {
                            pan += delta;
                        }
                    }
                    _ => (),
                }
                _ => (),
            }
        });
    }
}

pub static VERTEX_SHADER: &'static str = include_str!("render/shader/vertex.glsl");
pub static FRAGMENT_SHADER: &'static str = include_str!("render/shader/fragment.glsl");