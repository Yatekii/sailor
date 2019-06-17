use lyon::path::Path;
use lyon::math::*;
use lyon::tessellation::geometry_builder::{
    VertexBuffers,
    BuffersBuilder,
};
use lyon::tessellation::{
    FillTessellator,
    FillOptions,
};
use varint::ZigZag;

use crate::render::{
    Vertex,
    LayerVertexCtor,
};

use crate::vector_tile::mod_Tile::GeomType;

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

fn parse_one_to_path(geometry_type: GeomType, geometry: &Vec<u32>, extent: u32, cursor: &mut usize, gcursor: &mut Point) -> Path {
    let mut builder = Path::builder();

    while *cursor < geometry.len() {
        let value = geometry[*cursor];
        *cursor += 1;

        let count = value >> 3;
        match value & 0x07 {
            1 => {
                for _ in 0..count {
                    let x = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    let y = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    *gcursor += vector(x, y);
                    builder.move_to((*gcursor - vector(0.5, 0.5)) * 2.0);
                }
                match geometry_type {
                    GeomType::POINT => return builder.build(),
                    _ => {},
                }
            },
            2 => {
                for _ in 0..count {
                    let x = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    let y = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    *gcursor += vector(x, y);
                    builder.line_to((*gcursor - vector(0.5, 0.5)) * 2.0);
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

pub fn geometry_commands_to_drawable(geometry_type: GeomType, geometry: &Vec<u32>, extent: u32) -> VertexBuffers<Vertex, u16> {
    let mut mesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();
    let mut cursor = 0;

    let mut c = point(0f32, 0f32);

    if geometry_type == GeomType::POLYGON {
        while cursor < geometry.len() {
            let path = parse_one_to_path(geometry_type, geometry, extent, &mut cursor, &mut c);
            
            let mut tessellator = FillTessellator::new();
            let mut tmesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();
            tessellator
                .tessellate_path(
                    &path,
                    &FillOptions::tolerance(0.01),
                    &mut BuffersBuilder::new(&mut tmesh, LayerVertexCtor),
                )
                .expect("Failed to tesselate path.");

            for index in 0..tmesh.indices.len() {
                tmesh.indices[index] += mesh.vertices.len() as u16;
            }
            mesh.vertices.extend(tmesh.vertices);
            mesh.indices.extend(tmesh.indices);
        }
    }

    mesh
}

#[cfg(test)]
mod test {
    use super::{ GeomType, point, parse_one_to_path };
    #[test]
    fn transform_geometry() {
        let path = parse_one_to_path(GeomType::POLYGON, &vec![9, 1248, 2270, 250, 16, 36, 64, 15, 98, 264, 4, 74, 120, 162, 25, 48, 34, 25, 25, 32, 43, 11, 11, 28, 6, 88, 70, 90, 45, 76, 68, 108, 3, 266, 31, 50, 35, 11, 38, 28, 97, 15, 27, 45, 19, 7, 5, 22, 2, 19, 27, 1, 9, 131, 30, 187, 19, 173, 143, 375, 27, 177, 69, 147, 70, 49, 15], 4096, &mut 0, &mut point(0.0, 0.0));
        dbg!(path);
    }
}