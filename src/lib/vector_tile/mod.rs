mod vector_tile;
mod tile;
mod tile_id;

pub use vector_tile::*;
pub use tile::*;
pub use tile_id::*;

use core::ops::Range;
use lyon::{
    path::Path,
    math::*,
    tessellation::{
        FillTessellator,
        FillOptions,
    },
};
use varint::ZigZag;
use vector_tile::mod_Tile::*;
use super::*;

#[derive(Debug, Clone)]
pub struct Layer {
    pub name: String,
    pub id: u32,
    pub indices_range: Range<u32>,
    pub features: Vec<(u32, Range<u32>)>,
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

fn parse_one_to_path(geometry_type: GeomType, geometry: &Vec<u32>, cursor: &mut usize, gcursor: &mut Point) -> Path {
    let mut builder = Path::builder();

    while *cursor < geometry.len() {
        let value = geometry[*cursor];
        *cursor += 1;

        let count = value >> 3;
        match value & 0x07 {
            1 => {
                for _ in 0..count {
                    let dx = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32;
                    *cursor += 1;
                    let dy = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32;
                    *cursor += 1;
                    *gcursor += vector(dx, dy);
                    builder.move_to(*gcursor);
                }
                match geometry_type {
                    GeomType::POINT => return builder.build(),
                    _ => {},
                }
            },
            2 => {
                for _ in 0..count {
                    let dx = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32;
                    *cursor += 1;
                    let dy = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32;
                    *cursor += 1;
                    *gcursor += vector(dx, dy);
                    builder.line_to(*gcursor);
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
                    GeomType::POLYGON => {},
                    _ => panic!("This is a bug. Please report it."),
                }
            },
            _ => {
                panic!("This is a bug. Please report it.");
            },
        }
    }
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
}

pub fn geometry_commands_to_paths(geometry_type: GeomType, geometry: &Vec<u32>) -> Vec<Path> {
    let mut cursor = 0;
    let mut c = point(0f32, 0f32);
    let mut paths = Vec::new();

    while cursor < geometry.len() {
        let path = parse_one_to_path(geometry_type, geometry, &mut cursor, &mut c);
        paths.push(path);
    }

    paths
}

pub fn paths_to_drawable<'a, 'l>(
    builder: &'a mut MeshBuilder<'l>,
    geometry_type: GeomType,
    paths: &Vec<Path>,
    extent: f32,
    z: u32,
) {
    for path in paths {
        if geometry_type == GeomType::POLYGON {
            builder.set_current_extent(extent);
            builder.set_current_vertex_type(VertexType::Polygon);
            let mut tessellator = FillTessellator::new();
            let _ = tessellator
                .tessellate_path(
                    path,
                    &FillOptions::tolerance(0.0001).with_normals(true),
                    builder,
                ).map_err(|_e| { log::error!("Broken path."); });
        }

        if geometry_type == GeomType::LINESTRING {
            builder.set_current_vertex_type(VertexType::Line);
            builder.set_current_extent(extent);
            tesselate_line2(path, builder, z);
        }
    }
}