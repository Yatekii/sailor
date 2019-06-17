use crate::vector_tile::*;
use quick_protobuf::{MessageRead, BytesReader};
use lyon::tessellation::geometry_builder::VertexBuffers;
use crate::render::Vertex;
use std::time;

pub fn vector_tile_to_mesh(data: &Vec<u8>) -> Vec<crate::render::Layer> {
    let t = time::Instant::now();

    // we can build a bytes reader directly out of the bytes
    let mut reader = BytesReader::from_bytes(&data);

    let tile = Tile::from_reader(&mut reader, &data).expect("Cannot read Tile object.");

    let mut layers = vec![];

    for layer in &tile.layers {
        let mut mesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();

        for feature in &layer.features {
            let mut tmesh = crate::vector_tile::geometry_commands_to_drawable(feature.type_pb, &feature.geometry, tile.layers[0].extent);
            for index in 0..tmesh.indices.len() {
                tmesh.indices[index] += mesh.vertices.len() as u16;
            }
            mesh.vertices.extend(tmesh.vertices);
            mesh.indices.extend(tmesh.indices);
        }
        layers.push(crate::render::Layer {
            name: layer.name.to_string(),
            id: 0,
            mesh: mesh,
        })
    }

    println!("Took {} ms.", t.elapsed().as_millis());

    layers
}