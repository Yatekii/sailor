use quick_protobuf::Reader;

use crate::vector_tile::*;
use quick_protobuf::{MessageRead, BytesReader};
use lyon::tessellation::geometry_builder::{VertexConstructor, VertexBuffers, BuffersBuilder};
use crate::render::Vertex;

use std::fs::File;
use std::io::Read;

pub fn vector_tile_to_mesh(path: impl Into<String>) -> VertexBuffers<Vertex, u16> {
    let mut f = File::open(path.into()).expect("Unable to open file.");
    let mut buffer = Vec::new();
    // read the whole file
    f.read_to_end(&mut buffer).expect("Unable to read file.");

    // we can build a bytes reader directly out of the bytes
    let mut reader = BytesReader::from_bytes(&buffer);

    let tile = Tile::from_reader(&mut reader, &buffer).expect("Cannot read Tile object.");

    println!("Layer {:?}", tile.layers[0].name);

    let mut mesh: VertexBuffers<Vertex, u16> = VertexBuffers::new();

    for feature in &tile.layers[0].features {
        let tmesh = crate::vector_tile::geometry_commands_to_drawable(feature.type_pb, &feature.geometry, tile.layers[0].extent);
        mesh.vertices.extend(tmesh.vertices);
        mesh.indices.extend(tmesh.indices);
    }

    println!("File contains {} layers.", tile.layers.len());
    // println!("Layer 1: {:#?}", tile.layers[0]);

    mesh
}