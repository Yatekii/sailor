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
    let (x, y) = crate::vector_tile::math::deg2num(47.3769, 8.5417, z);
    let zxy: String = format!("/{}/{}/{}", z, x, y);
    std::fs::create_dir_all(format!("cache{}", zxy)).expect("Could not create cache directories.");

    let request_url = format!("https://api.maptiler.com/tiles/v3{}.pbf?key=t2mP0OQnprAXkW20R6Wd", zxy);
    println!("{}", request_url);
    let mut resp = reqwest::get(&request_url).expect("Could not load tile.");
    if resp.status() != reqwest::StatusCode::OK {
        panic!("Tile request failed.");
    }
    let mut data: Vec<u8> = vec![];
    resp.copy_to(&mut data).expect("Could not read http response to buffer.");
    let pbf = format!("cache{}.pbf", zxy);
    let mut file = std::fs::File::create(&pbf).expect("Could not create pbf file.");

    use std::io::Write;
    file.write_all(&data[..]).expect("Could not write bytes.");

    let layers = crate::vector_tile::vector_tile_to_mesh(pbf);

    let mut events_loop = glium::glutin::EventsLoop::new();
    let context = glium::glutin::ContextBuilder::new().with_vsync(true);
    let window = glium::glutin::WindowBuilder::new()
        .with_dimensions(LogicalSize { width: 400.0, height: 400.0 })
        .with_decorations(true)
        .with_title("lyon + glium basic example");
    let display = glium::Display::new(window, context, &events_loop).unwrap();

    const GREEN: [f32; 3] = [0.035, 0.678, 0.431f32];
    const BLUE: [f32; 3] = [0.239, 0.824, 1.0f32];
    const YELLOW: [f32; 3] = [1.0, 0.894, 0.408];

    let data = layers.iter().filter_map(|l| {
        println!("{}", l.name);
        let vertex_buffer = glium::VertexBuffer::new(&display, &l.mesh.vertices).unwrap();
        let indices = glium::IndexBuffer::new(
            &display,
            glium::index::PrimitiveType::TrianglesList,
            &l.mesh.indices,
        ).unwrap();
        let uniforms = uniform! {
            layer_color: match &l.name[..] {
                "water" => BLUE,
                "waterway" => return None,
                "landcover" => GREEN,
                "landuse" => YELLOW,
                "mountain_peak" => return None,
                "park" => GREEN,
                "boundary" => return None,
                "transportation" => return None,
                "transportation_name" => return None,
                "place" => return None,
                _ => return None,
            }
        };
        Some((vertex_buffer, indices, uniforms))
    }).collect::<Vec<_>>();
    
    let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None)
        .unwrap();

    let mut status = true;
    loop {
        if !status {
            break;
        }

        let mut target = display.draw();
        target.clear_color(0.8, 0.8, 0.8, 1.0);
        for d in &data {
            target.draw(
                &d.0,
                &d.1,
                &program,
                &d.2,
                &Default::default(),
            ).unwrap();
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