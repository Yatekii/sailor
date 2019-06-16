mod postgis;
mod vector_tile;
mod render;

#[macro_use]
extern crate glium;
extern crate lyon;

use structopt::StructOpt;

use glium::Surface;
use glium::glutin::dpi::LogicalSize;

#[derive(StructOpt)]
#[structopt(
    name = "Sailor",
    about = "Sailing weather and route tooling",
    author = "Noah Hüsser <yatekii@yatekii.ch>"
)]
enum CLI {
    #[structopt(name = "import_map")]
    ImportMap {
        path: String,
    },
    #[structopt(name = "inspect_tile")]
    InspectTile {
        path: String,
    },
}

fn main() {
    // let matches = CLI::from_args();
    // match matches {
    //     CLI::ImportMap { path } => osm::import::region(path),
    //     CLI::InspectTile { path } => tile::vector_tile_to_vbo(path),
    // }

    let mesh = crate::vector_tile::vector_tile_to_mesh("data/test.pbf");

    let mut events_loop = glium::glutin::EventsLoop::new();
    let context = glium::glutin::ContextBuilder::new().with_vsync(true);
    let window = glium::glutin::WindowBuilder::new()
        .with_dimensions(LogicalSize { width: 400.0, height: 400.0 })
        .with_decorations(true)
        .with_title("lyon + glium basic example");
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    let vertex_buffer = glium::VertexBuffer::new(&display, &mesh.vertices).unwrap();
    let indices = glium::IndexBuffer::new(
        &display,
        glium::index::PrimitiveType::TrianglesList,
        &mesh.indices,
    ).unwrap();
    let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None)
        .unwrap();

    let mut status = true;
    loop {
        if !status {
            break;
        }

        let mut target = display.draw();
        target.clear_color(0.8, 0.8, 0.8, 1.0);
        target.draw(
            &vertex_buffer,
            &indices,
            &program,
            &glium::uniforms::EmptyUniforms,
            &Default::default(),
        ).unwrap();

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