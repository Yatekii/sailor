pub mod tile;
pub mod tile_id;
pub mod vector_tile;

use core::ops::Range;
use lyon::{
    math::*,
    path::Path,
    tessellation::{FillOptions, FillTessellator},
};
use varint::ZigZag;
use vector_tile::mod_Tile::*;

use crate::{
    drawing::{
        line_tesselator::{tesselate_line2, tesselate_points},
        mesh::MeshBuilder,
        vertex::{Vertex, VertexType},
    },
    math::TileId,
};

#[derive(Debug, Clone)]
pub struct Layer {
    pub name: String,
    pub id: u32,
    pub indices_range: Range<u32>,
    pub features: Vec<(u32, Range<u32>)>,
}

// fn _area(path: &Path) -> f32 {
//     let mut points = path.points().to_vec();
//     points.push(*points.first().expect("Path contains no points!"));
//     let mut area = 0f32;
//     for i in 0..points.len() - 1 {
//         area += points[i].x * points[i + 1].y;
//     }
//     for i in 0..points.len() - 1 {
//         area -= points[i + 1].x * points[i].y;
//     }
//     area + points[points.len() - 1].x * points[1].y - points[points.len() - 1].y * points[1].x
// }

fn parse_one_to_path(
    geometry_type: GeomType,
    geometry: &[u32],
    cursor: &mut usize,
    gcursor: &mut Point,
) -> Path {
    let mut builder = Path::builder();
    let mut in_subpath = false;

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
                    // A move-to starts a new sub-path; close the previous one first,
                    // otherwise lyon panics on `begin` while already in a sub-path.
                    if in_subpath {
                        builder.end(false);
                    }
                    builder.begin(*gcursor);
                    in_subpath = true;
                }

                if let GeomType::POINT = geometry_type {
                    builder.end(false);
                    return builder.build();
                }
            }
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
                    GeomType::LINESTRING => {
                        builder.end(false);
                        return builder.build();
                    }
                    _ => {}
                }
            }
            7 => {
                builder.close();
                in_subpath = false;
                match geometry_type {
                    GeomType::POINT => panic!("This is a bug. Please report it."),
                    GeomType::LINESTRING => panic!("This is a bug. Please report it."),
                    GeomType::POLYGON => {}
                    _ => panic!("This is a bug. Please report it."),
                }
            }
            _ => {
                panic!("This is a bug. Please report it.");
            }
        }
    }
    if in_subpath {
        builder.end(false);
    }
    match geometry_type {
        GeomType::POINT => panic!("This is a bug. Please report it."),
        GeomType::LINESTRING => panic!("This is a bug. Please report it."),
        GeomType::POLYGON => builder.build(),
        _ => panic!("This is a bug. Please report it."),
    }
}

pub fn geometry_commands_to_paths(geometry_type: GeomType, geometry: &[u32]) -> Vec<Path> {
    let mut cursor = 0;
    let mut c = point(0f32, 0f32);
    let mut paths = Vec::new();

    while cursor < geometry.len() {
        let path = parse_one_to_path(geometry_type, geometry, &mut cursor, &mut c);
        paths.push(path);
    }

    paths
}

pub fn paths_to_drawable(
    builder: &mut MeshBuilder,
    geometry_type: GeomType,
    paths: &[Path],
    extent: f32,
    tile_id: &TileId,
) {
    for path in paths {
        // println!("{path:?}");
        if geometry_type == GeomType::POLYGON {
            let offset = builder.buffers.vertices.len();
            builder.set_current_extent(extent);
            builder.set_current_vertex_type(VertexType::Polygon);
            let mut tessellator = FillTessellator::new();
            let _ = tessellator
                .tessellate_path(path, &FillOptions::tolerance(0.0000001), builder)
                .map_err(|e| {
                    log::error!("Broken path on tile {tile_id}.");
                    log::error!("{e:#?}");
                });
            set_normals(&mut builder.buffers.vertices[offset..], path, extent);
        }

        if geometry_type == GeomType::LINESTRING {
            builder.set_current_vertex_type(VertexType::Line);
            builder.set_current_extent(extent);
            tesselate_line2(path, builder, extent);
        }

        if geometry_type == GeomType::POINT {
            // Dots reuse the line vertex type so `line-width` sizes them in pixels.
            builder.set_current_vertex_type(VertexType::Line);
            builder.set_current_extent(extent);
            tesselate_points(path, builder, extent);
        }
    }
}

// TODO: Very buggy!
// Creates normals that are NaN because the vector normalization is called on a vector that has length 0.
// Why is the vector (0, 0)? Point subtraction yields it, so maybe wrong 2 points?
fn set_normals(vertices: &mut [Vertex], path: &Path, extent: f32) {
    let points = path.points();
    let len = points.len();
    if len < 3 {
        let first_vector = points[1] - points[0];
        let normal = Vector::new(-first_vector.y, first_vector.x);
        set_normal(vertices, &points[0], normal);

        let normal = Vector::new(-first_vector.y, first_vector.x);
        set_normal(vertices, &points[1], normal);
        return;
    }

    let first_vector = points[1] - points[0];
    let mut previous_normal = Vector::new(first_vector.y, -first_vector.x).normalize();

    // println!("len: {len}");

    let normal = calculate_normals(
        &points[len - 1],
        &points[0],
        &points[1],
        &previous_normal,
        extent,
    );
    set_normal(vertices, &points[0], normal);

    for point_tuple in points.windows(3) {
        let normal = calculate_normals(
            &point_tuple[0],
            &point_tuple[1],
            &point_tuple[2],
            &previous_normal,
            extent,
        );
        previous_normal = normal;
        set_normal(vertices, &point_tuple[1], normal);
    }

    let normal = calculate_normals(
        &points[len - 2],
        &points[len - 1],
        &points[0],
        &previous_normal,
        extent,
    );
    set_normal(vertices, &points[len - 1], normal);
}

fn set_normal(vertices: &mut [Vertex], position: &Point, normal: Vector) {
    let vertex = vertices
        .iter_mut()
        .find(|v| v.position[0] == position.x as i16 && v.position[1] == position.y as i16);
    if let Some(vertex) = vertex {
        // println!("{}", normal.length());
        let len = normal.length();
        if len < 4000.0 {
            // println!("Sml: {:?} - {}", normal, len);
        } else if len.is_nan() {
            // panic!("NaN: {:?}", normal);
        }
        vertex.normal = [normal.x as i16, normal.y as i16];
    } else {
        println!("vertex {}/{} was not found", position.x, position.y);
    }
}

fn calculate_normals(
    p1: &Point,
    p2: &Point,
    p3: &Point,
    previous_normal: &Vector,
    extent: f32,
) -> Vector {
    let v1 = *p1 - *p2;
    let v2 = *p3 - *p2;

    let normal1 = (v1.normalize() + v2.normalize()).normalize();

    // println!(
    //     "norm hehe: {:?} {:?} {:?}",
    //     v1,
    //     v1.normalize(),
    //     v2.normalize()
    // );

    let normal = if normal1.dot(*previous_normal) < 0.0 {
        -normal1
    } else {
        normal1
    } * extent;

    if normal.length().is_nan() {
        // panic!();
    }

    if normal.x as i16 == 3125
        && normal.y as i16 == -2647
        && p2.x as i16 == 3585
        && p2.y as i16 == 2773
    {
        println!("{v1:?}");
        println!("{v2:?}");
        println!("{normal1:?}");
        println!("{p1:?}");
        println!("{p3:?}");
    }
    normal
}
