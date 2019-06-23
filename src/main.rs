mod vector_tile;
mod render;
mod drawing;

#[macro_use]
extern crate glium;
extern crate lyon;
// #[macro_use]
// extern crate pfh;

use structopt::StructOpt;

use glium::glutin::dpi::LogicalSize;

use crate::vector_tile::*;
use lyon::math::{
    vector,
};
use crate::drawing::vertex::vertex;
use crate::vector_tile::math::TileId;

fn main() {
    let mut painter = drawing::Painter::init();

    let z = 8;
    let tile_id = math::deg2tile(47.3769, 8.5417, z);
    let tile_coordinate = math::deg2num(47.3769, 8.5417, z);
    dbg!(tile_id);
    let zurich = math::num_to_global_space(&tile_coordinate);

    dbg!(zurich);

    let data = vector_tile::fetch_tile_data(&tile_id);

    let mut layers = crate::vector_tile::vector_tile_to_mesh(&tile_id, &data);
    let id = tile_id - TileId::new(z, 0, 1);
    layers.extend(crate::vector_tile::vector_tile_to_mesh(&id, &vector_tile::fetch_tile_data(&id)));


    painter.set_buffers(&layers[0].mesh.vertices, &layers[0].mesh.indices);

    // painter.set_buffers(&vec![
    //     vertex(0.0 / 256.0, -0.5 / 256.0),
    //     vertex(0.5 / 256.0, 0.5 / 256.0),
    //     vertex(-0.5 / 256.0, 0.5 / 256.0)
    // ], &vec![2, 1, 0]);

    // dbg!(vec![vertex(0.0, -0.5), vertex(0.5, 0.5), vertex(-0.5, 0.5)]);

    let mut cache = crate::vector_tile::cache::TileCache::new();
    let mut screen = math::Screen::new(zurich, 600, 600);

    cache.fetch_tiles(&screen);
    let layers = cache
        .get_tiles(&screen)
        .into_iter()
        .flat_map(|tile| tile.layers.into_iter())
        .collect::<Vec<_>>();

    dbg!(&layers.len());
    
    let mut v = vec![];
    let mut i = vec![];

    for rl in layers {
        if rl.name == "water" {
            let l = v.len();
            v.extend(rl.mesh.vertices);
            i.extend(rl.mesh.indices.iter().map(|i| i + l as u16).collect::<Vec<u16>>());
        }
    }

    painter.set_buffers(&v, &i);

    dbg!(&i.len());

    loop {
        painter.update_view();
    }
}

fn mainf() {
    // let matches = CLI::from_args();
    // match matches {
    //     CLI::ImportMap { path } => osm::import::region(path),
    //     CLI::InspectTile { path } => tile::vector_tile_to_vbo(path),
    // }

    let z = 8;
    let tile_id = math::deg2tile(47.3769, 8.5417, z);
    let tile_coordinate = math::deg2num(47.3769, 8.5417, z);
    dbg!(tile_id);
    let zurich = math::num_to_global_space(&tile_coordinate);

    let mut cache = crate::vector_tile::cache::TileCache::new();
    let mut css_cache = crate::render::css::RulesCache::try_load_from_file("config/style.css")
        .expect("Could not load map style.");

    // let zurich: lyon::math::Point = lyon::math::point(0.525754,0.35115147);
    // let (x, y) = crate::vector_tile::math::deg2num(40.7128, 74.0060, z); // NY

    // let data = vector_tile::fetch_tile_data(&tile_id);

    // let mut layers = crate::vector_tile::vector_tile_to_mesh(&tile_id, &data);
    // layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x+1, y, &vector_tile::fetch_tile_data(z, x+1, y)));
    // layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x, y+1, &vector_tile::fetch_tile_data(z, x, y+1)));
    // layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x+1, y+1, &vector_tile::fetch_tile_data(z, x+1, y+1)));
    // layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x, y+2, &vector_tile::fetch_tile_data(z, x, y+2)));
    // layers.extend(crate::vector_tile::vector_tile_to_mesh(z, x+1, y+2, &vector_tile::fetch_tile_data(z, x+1, y+2)));

    let mut events_loop = glium::glutin::EventsLoop::new();
    let context = glium::glutin::ContextBuilder::new().with_vsync(true);
    let window = glium::glutin::WindowBuilder::new()
        .with_dimensions(LogicalSize { width: 600.0, height: 600.0 })
        .with_decorations(true)
        .with_title("lyon + glium basic example");
    let display = glium::Display::new(window, context, &events_loop).unwrap();
    let program = glium::Program::from_source(&display, VERTEX_SHADER, FRAGMENT_SHADER, None).unwrap();

    let mut painter = crate::render::painter::Painter::new(&display, &program);
    let mut pan = zurich.clone();

    let mut screen = math::Screen::new(pan, 600, 600);

    let mut mouse_down = false;
    let mut last_pos = glium::glutin::dpi::LogicalPosition::new(0.0, 0.0);

    let mut status = true;
    loop {
        if !status {
            break;
        }

        css_cache.update();

        painter.paint(&mut cache, &mut css_cache, &screen, z, pan);

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

                        last_pos = position;
                        if mouse_down {
                            pan -= delta;
                            screen.move_center(&delta);
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