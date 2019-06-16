use lyon::extra::rust_logo::build_logo_path;
use lyon::path::builder::*;
use lyon::path::Path;
use lyon::math::*;
use lyon::tessellation::geometry_builder::{VertexConstructor, VertexBuffers, BuffersBuilder};
use lyon::tessellation::{FillTessellator, FillOptions};
use lyon::tessellation;
use varint::ZigZag;

use crate::render::{
    Vertex,
    VertexCtor,
};

use crate::vector_tile::mod_Tile::GeomType;

pub enum Integer {
    Command,
    Parameter,
}

pub enum Command {
    MoveTo(u32, u32),
    LineTo(u32, u32),
    ClosePath,
}

pub struct Parameter(u32);

fn parse_one_to_path(geometry_type: GeomType, geometry: &Vec<u32>, extent: u32, cursor: &mut usize) -> Path {
    let mut builder = Path::builder();

    while *cursor < geometry.len() {
        let value = geometry[*cursor];
        *cursor += 1;

        let mut c = point(0f32, 0f32);

        println!("Extent: {}", extent);

        let count = value >> 3;
        match value & 0x07 {
            1 => {
                for i in 0..count {
                    println!("{:?}, {:?}", ZigZag::<i32>::zigzag(&geometry[*cursor]), *cursor);
                    let x = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    println!("{:?}, {:?}", ZigZag::<i32>::zigzag(&geometry[*cursor]), *cursor);
                    let y = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    c += vector(x, y);
                    builder.move_to(c - vector(1.0, 1.0));
                }
                match geometry_type {
                    GeomType::POINT => return builder.build(),
                    _ => {},
                }
            },
            2 => {
                for i in 0..count {
                    println!("{:?}, {:?}", ZigZag::<i32>::zigzag(&geometry[*cursor]), *cursor);
                    let x = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    println!("{:?}, {:?}", ZigZag::<i32>::zigzag(&geometry[*cursor]), *cursor);
                    let y = ZigZag::<i32>::zigzag(&geometry[*cursor]) as f32 / extent as f32;
                    *cursor += 1;
                    c += vector(x, y);
                    builder.line_to(c - vector(1.0, 1.0));
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
                    GeomType::POLYGON => return builder.build(),
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

    if geometry_type == GeomType::POLYGON {
        while cursor < geometry.len() {
            println!("REAL {}", cursor);
            let path = parse_one_to_path(geometry_type, geometry, extent, &mut cursor);
            
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