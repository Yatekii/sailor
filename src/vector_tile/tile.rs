use crate::vector_tile::transform::geometry_commands_to_drawable;
use crate::vector_tile::math::TileId;
use crate::drawing::mesh::MeshBuilder;
use crate::vector_tile::*;
use quick_protobuf::{MessageRead, BytesReader};

use lyon::tessellation::geometry_builder::{
    VertexBuffers,
};

use crate::drawing::vertex::{
    Vertex,
    LayerVertexCtor,
};

#[derive(Debug, Clone)]
pub struct Tile {
    pub tile_id: TileId,
    pub layers: Vec<crate::vector_tile::transform::Layer>,
    pub mesh: VertexBuffers<Vertex, u32>,
}

impl Tile {
    pub fn from_mbvt(tile_id: &math::TileId, data: &Vec<u8>) -> Self {
        // let t = std::time::Instant::now();

        // we can build a bytes reader directly out of the bytes
        let mut reader = BytesReader::from_bytes(&data);

        let tile = crate::vector_tile::Tile::from_reader(&mut reader, &data).expect("Cannot read Tile object.");
        // dbg!(t.elapsed().as_millis());

        let mut layer_id = 0;
        let mut layers = Vec::with_capacity(tile.layers.len());
        let mut mesh: VertexBuffers<Vertex, u32> = VertexBuffers::with_capacity(4_000_000, 4_000_000);
        let mut builder = MeshBuilder::new(&mut mesh, LayerVertexCtor::new(tile_id));

        for (i, layer) in tile.layers.iter().enumerate() {
            builder.set_current_layer_id(i as u32);
            for feature in &layer.features {
                geometry_commands_to_drawable(
                    &mut builder,
                    tile_id,
                    i as u32,
                    feature.type_pb,
                    &feature.geometry,
                    tile.layers[0].extent
                );
            }

            layers.push(crate::vector_tile::transform::Layer {
                name: layer.name.to_string(),
            });
        }

        // dbg!(t.elapsed().as_millis());

        Self {
            tile_id: tile_id.clone(),
            layers,
            mesh,
        }
    }
}