mod vector_tile;
mod render;

#[macro_use]
extern crate glium;
extern crate lyon;

use structopt::StructOpt;

use glium::Surface;
use glium::glutin::dpi::LogicalSize;

use crate::vector_tile::*;

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

    let mut events_loop = glium::glutin::EventsLoop::new();
    let context = glium::glutin::ContextBuilder::new().with_vsync(true);
    let window = glium::glutin::WindowBuilder::new()
        .with_dimensions(LogicalSize { width: 400.0, height: 400.0 })
        .with_decorations(true)
        .with_title("lyon + glium basic example");
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    for layer in &mut layers {
        layer.load(&display)
    }
    
    let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();

    let mut status = true;
    loop {
        if !status {
            break;
        }

        let pan = zurich.clone() * -1.0;
        

        let mut target = display.draw();
        target.clear_color(0.8, 0.8, 0.8, 1.0);
        for layer in &layers {
            layer.draw(&mut target, &program, pan);
        }

        target.finish().unwrap();

        events_loop.poll_events(|event| {
            use glium::glutin::{Event, WindowEvent};
            match event {
                Event::WindowEvent { event, .. } => match event {
                    WindowEvent::Destroyed => { status = false }
                    WindowEvent::KeyboardInput {
                        input: glium::glutin::KeyboardInput { virtual_keycode: Some(glium::glutin::VirtualKeyCode::Escape), .. },
                        ..
                    } => { status = false }
                    _ => (),
                }
                _ => (),
            }
        });
    }
}

pub static VERTEX_SHADER: &'static str = include_str!("render/shader/vertex.glsl");
pub static FRAGMENT_SHADER: &'static str = include_str!("render/shader/fragment.glsl");