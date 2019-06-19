use crate::vector_tile::*;
use quick_protobuf::{MessageRead, BytesReader};
use std::time;

use lyon_tess2 as tess2;
use tess2::path::Path;
use lyon::math::*;
use tess2::tessellation::geometry_builder::{
    VertexBuffers,
    BuffersBuilder,
};
use tess2::{
    FillTessellator,
    FillOptions,
};
use varint::ZigZag;

use crate::render::{
    Vertex,
    LayerVertexCtor,
};

use crate::vector_tile::mod_Tile::GeomType;

pub fn vector_tile_to_mesh(z: u32, x: u32, y: u32, data: &Vec<u8>) -> Vec<crate::render::Layer> {
    let t = time::Instant::now();

    // we can build a bytes reader directly out of the bytes
    let mut reader = BytesReader::from_bytes(&data);

    let tile = Tile::from_reader(&mut reader, &data).expect("Cannot read Tile object.");

    let mut layers = vec![];

    for layer in &tile.layers {
        let mut mesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        for feature in &layer.features {
            let mut tmesh = geometry_commands_to_drawable(
                z,
                x,
                y,
                feature.type_pb,
                &feature.geometry,
                tile.layers[0].extent
            );
            for index in 0..tmesh.indices.len() {
                tmesh.indices[index] += mesh.vertices.len() as u16;
            }
            mesh.vertices.extend(tmesh.vertices);
            mesh.indices.extend(tmesh.indices);
        }

        const GREEN: [f32; 3] = [0.035, 0.678, 0.431f32];
        const BLUE: [f32; 3] = [0.239, 0.824, 1.0f32];
        const YELLOW: [f32; 3] = [1.0, 0.894, 0.408];

        match &layer.name.to_string()[..] {
            "water" | "park" | "landcover" | "landuse" => {
                layers.push(crate::render::Layer::new(
                    layer.name.to_string(),
                    mesh,
                    match &layer.name[..] {
                        "water" => BLUE,
                        "landcover" => GREEN,
                        "landuse" => YELLOW,
                        "park" => GREEN,
                        _ => panic!("This is a bug. Please report it."),
                    }
                ))
            },
            _ => {}
        }
    }

    println!("Took {} ms.", t.elapsed().as_millis());

    layers
}

fn area(path: &Path) -> f32 {
    let mut points = path.points().to_vec();
    points.push(points.first().expect("Path contains no points!").clone());
    let mut area = 0f32;
    for i in 0..points.len() - 1 {
        area += points[i].x * points[i + 1].y;
    }
    for i in 0..points.len() - 1 {
        area -= points[i + 1].x * points[i].y;
    }
    area + points[points.len() - 1].x * points[1].y - points[points.len() - 1].y * points[1].x
}

fn parse_one_to_path(z: u32, x: u32, y: u32, geometry_type: GeomType, geometry: &Vec<u32>, extent: u32, cursor: &mut usize, gcursor: &mut Point) -> Path {
    let mut builder = Path::builder();

    while *cursor < geometry.len() {
        let value = geometry[*cursor];
        *cursor += 1;

        let count = value >> 3;
        match value & 0x07 {
            1 => {
                for _ in 0..count {
                    let dx = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    let dy = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    *gcursor += vector(dx, dy);
                    builder.move_to(math::tile_to_global_space(z, x, y, *gcursor));
                    // println!("{}", math::tile_to_global_space(z, x, y, *gcursor)); // (10720.039,7120.0513)
                }
                match geometry_type {
                    GeomType::POINT => return builder.build(),
                    _ => {},
                }
            },
            2 => {
                for _ in 0..count {
                    let dx = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    let dy = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    *gcursor += vector(dx, dy);
                    builder.line_to(math::tile_to_global_space(z, x, y, *gcursor));
                    // println!("{}", math::tile_to_global_space(z, x, y, *gcursor));
                }
                match geometry_type {
                    GeomType::POINT => panic!("This is a bug. Please report it."),
                    GeomType::LINESTRING => return builder.build(),
                    _ => {},
                }
            },
            7 => {
                builder.close();
                match geometry_type {
                    GeomType::POINT => panic!("This is a bug. Please report it."),
                    GeomType::LINESTRING => panic!("This is a bug. Please report it."),
                    GeomType::POLYGON => {
                        let path = builder.build();
                        if area(&path) < 0f32 {
                            return path;
                        } else {
                            return path;
                        }
                    },
                    _ => panic!("This is a bug. Please report it."),
                }
            },
            _ => {
                panic!("This is a bug. Please report it.");
            },
        }
    }
    panic!("This is a bug. Please report it.");
}

fn geometry_commands_to_drawable(z: u32, x: u32, y: u32, geometry_type: GeomType, geometry: &Vec<u32>, extent: u32) -> VertexBuffers<Vertex, u16> {
    let mut mesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();
    let mut cursor = 0;

    let mut c = math::tile_to_global_space(z, x, y, point(0f32, 0f32));

    if geometry_type == GeomType::POLYGON {
        while cursor < geometry.len() {
            let path = parse_one_to_path(z, x, y, geometry_type, geometry, extent, &mut cursor, &mut c);
            
            let mut tessellator = FillTessellator::new();
            let mut receiver: VertexBuffers<Point, u16> = VertexBuffers::new();
            let mut tmesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();
            tessellator
                .tessellate_path(
                    &path,
                    &FillOptions::tolerance(0.01),
                    &mut lyon::tessellation::geometry_builder::simple_builder(&mut receiver),
                )
                .expect("Failed to tesselate path.");

            tmesh.vertices = receiver.vertices.iter().map(|p| Vertex { position: [p.x, p.y] }).collect::<Vec<_>>();
            tmesh.indices = receiver.indices;

            for index in 0..tmesh.indices.len() {
                tmesh.indices[index] += mesh.vertices.len() as u16;
            }
            mesh.vertices.extend(tmesh.vertices);
            mesh.indices.extend(tmesh.indices);
        }
    }

    mesh
}