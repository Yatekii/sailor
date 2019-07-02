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

pub fn layer_num(name: &str) -> u32 {
    19 - match name {	
        "water" => 0,
        "waterway" => 1,
        "landcover" => 2,
        "landuse" => 3,
        "mountain_peak" => 4,
        "park" => 5,
        "boundary" => 6,
        "aeroway" => 7,
        "transportation" => 8,
        "building" => 9,
        "water_name" => 10,
        "transportation_name" => 11,
        "place" => 12,
        "housenumber" => 13,
        "poi" => 14,
        "aerodrome_label" => 15,
        _ => 19,
    }
}

impl Tile {
    pub fn from_mbvt(tile_id: &math::TileId, data: &Vec<u8>) -> Self {
        // let t = std::time::Instant::now();

        // we can build a bytes reader directly out of the bytes
        let mut reader = BytesReader::from_bytes(&data);

        let tile = crate::vector_tile::Tile::from_reader(&mut reader, &data).expect("Cannot read Tile object.");
        // dbg!(t.elapsed().as_millis());

        let mut layers = Vec::with_capacity(tile.layers.len());
        let mut mesh: VertexBuffers<Vertex, u32> = VertexBuffers::with_capacity(100_000, 100_000);
        let mut builder = MeshBuilder::new(&mut mesh, LayerVertexCtor::new(tile_id));

        for layer in &tile.layers {
            let index_start_before = builder.get_current_index();
            let layer_id = layer_num(&layer.name);
            builder.set_current_layer_id(layer_id);
            for feature in &layer.features {
                geometry_commands_to_drawable(
                    &mut builder,
                    feature.type_pb,
                    &feature.geometry,
                    tile.layers[0].extent
                );
            }

            layers.push(crate::vector_tile::transform::Layer {
                name: layer.name.to_string(),
                id: layer_id,
                indices_range: index_start_before..builder.get_current_index(),
            });
        }

        Self {
            tile_id: tile_id.clone(),
            layers,
            mesh,
        }
    }
}