use crate::vector_tile::*;
use quick_protobuf::{MessageRead, BytesReader};
use std::time;

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

use crate::drawing::vertex::{
    Vertex,
    LayerVertexCtor
};

use crate::vector_tile::mod_Tile::GeomType;

use crate::render::css::RulesCache;
use crate::render::css::Selector;
use crate::render::css::CSSValue;

#[derive(Debug, Clone)]
pub struct Layer {
    pub name: String,
    pub mesh: VertexBuffers<Vertex, u16>,
}

impl Layer {
    pub fn new(name: String, mesh: VertexBuffers<Vertex, u16>) -> Self {
        Self {
            name,
            mesh,
        }
    }

    pub fn with_style(mut self, cache: &RulesCache) -> Self {
        let rules = cache.get_matching_rules(
            &Selector::new()
                .with_type("layer".to_string())
                .with_any("name".to_string(), self.name.clone())
        );
        let background_color = rules
            .iter()
            .filter_map(|r| r.kvs.get("background-color"))
            .last();

            // if let Some(color) = background_color {
            //     match color {
            //         CSSValue::Color(bg) => {
            //             self.color = [bg.r as f32 / 255.0, bg.g as f32 / 255.0, bg.b as f32 / 255.0];
            //         },
            //         CSSValue::String(string) => {
            //             match &string[..] {
            //                 "red" => self.color = [1.0, 0.0, 0.0],
            //                 "blue" => self.color = [0.0, 0.0, 1.0],
            //                 "green" => self.color = [0.0, 1.0, 0.0],
            //                 // Other CSS colors to come later.
            //                 _ => {},
            //             }
            //         },
            //         _ => {},
            //     }
            // }

        self
    }
}

pub fn vector_tile_to_mesh(tile_id: &math::TileId, data: &Vec<u8>) -> Vec<crate::vector_tile::transform::Layer> {
    let t = time::Instant::now();

    // we can build a bytes reader directly out of the bytes
    let mut reader = BytesReader::from_bytes(&data);

    let tile = Tile::from_reader(&mut reader, &data).expect("Cannot read Tile object.");

    let mut layers = vec![];

    for (i, layer) in tile.layers.iter().enumerate() {
        let mut mesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        for feature in &layer.features {
            let mut tmesh = geometry_commands_to_drawable(
                tile_id,
                i as u32,
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

        layers.push(crate::vector_tile::transform::Layer {
            name: layer.name.to_string(),
            mesh: mesh,
        });
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

fn parse_one_to_path(geometry_type: GeomType, geometry: &Vec<u32>, extent: u32, cursor: &mut usize, gcursor: &mut Point) -> Path {
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
                    builder.move_to(*gcursor);
                    // println!("{}", math::tile_to_global_space(z, x, y, *gcursor)); // (1.7231445,0.74121094)
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
                    builder.line_to(*gcursor);
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

fn geometry_commands_to_drawable(tile_id: &math::TileId, layer_id: u32, geometry_type: GeomType, geometry: &Vec<u32>, extent: u32) -> VertexBuffers<Vertex, u16> {
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
                    &FillOptions::tolerance(0.0001),
                    &mut BuffersBuilder::new(&mut tmesh, LayerVertexCtor { tile_id: tile_id.clone(), layer_id }),
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