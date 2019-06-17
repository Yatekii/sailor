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
    VertexCtor,
};

use crate::vector_tile::mod_Tile::GeomType;

fn area(path: &Path) -> f32 {
    let points = path.points();
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

        println!("{:?}", gcursor);

        let count = value >> 3;
        match value & 0x07 {
            1 => {
                for _ in 0..count {
                    let x = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    let y = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    *gcursor += vector(x, y);
                    builder.move_to(*gcursor * 4.0 - vector(1.0, 1.0));
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
                    builder.line_to(*gcursor * 4.0 - vector(1.0, 1.0));
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
                            println!("KEKKEKEKEKEKEK");
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

    println!("parsing geometry");

    let mut c = point(0f32, 0f32);

    if geometry_type == GeomType::POLYGON {
        while cursor < geometry.len() {
            println!("REAL {}", cursor);
            let path = parse_one_to_path(geometry_type, geometry, extent, &mut cursor, &mut c);
            
            let mut tessellator = FillTessellator::new();
            let mut tmesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();
            tessellator
                .tessellate_path(
                    &path,
                    &FillOptions::tolerance(0.01),
                    &mut BuffersBuilder::new(&mut tmesh, VertexCtor),
                )
                .expect("Failed to tesselate path.");

            mesh.vertices.extend(tmesh.vertices);
            mesh.indices.extend(tmesh.indices);
        }
    }
    

    println!(
        " -- fill: {} vertices {} indices",
        mesh.vertices.len(),
        mesh.indices.len()
    );

    mesh
}