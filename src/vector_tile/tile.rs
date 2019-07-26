use crate::drawing::layer_collection::LayerCollection;
use crate::drawing::feature::Feature;
use std::sync::{
    Arc,
    RwLock,
};
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
    pub extent: u16,
}

pub fn layer_num(name: &str) -> u32 {
    match name {
        "landcover" => 0,
        "water" => 1,
        "waterway" => 2,
        "landuse" => 3,
        // "mountain_peak" => 4,
        "park" => 5,
        "boundary" => 6,
        // "aeroway" => 7,
        "transportation" => 8,
        "building" => 9,
        // "water_name" => 10,
        // "transportation_name" => 11,
        // "place" => 12,
        // "housenumber" => 13,
        // "poi" => 14,
        // "aerodrome_label" => 15,
        _ => 19,
    }
}

impl Tile {
    pub fn from_mbvt(tile_id: &math::TileId, data: &Vec<u8>, layer_collection: Arc<RwLock<LayerCollection>>) -> Self {
        // let t = std::time::Instant::now();

        // we can build a bytes reader directly out of the bytes
        let mut reader = BytesReader::from_bytes(&data);

        let tile = crate::vector_tile::Tile::from_reader(&mut reader, &data).expect("Cannot read Tile object.");
        // dbg!(t.elapsed().as_millis());

        let mut layers = Vec::with_capacity(tile.layers.len());
        let mut mesh: VertexBuffers<Vertex, u32> = VertexBuffers::with_capacity(100_000, 100_000);
        let mut builder = MeshBuilder::new(&mut mesh, LayerVertexCtor::new(tile_id, 1.0));
        let extent = tile.layers[0].extent as u16;

        for layer in tile.layers {
            let mut index_start_before = builder.get_current_index();
            let layer_id = layer_num(&layer.name);
            let mut current_feature_id = 0;

            let mut map: std::collections::HashMap<
                crate::css::Selector,
                Vec<(vector_tile::mod_Tile::GeomType, std::vec::Vec<u32>)>
            > = std::collections::HashMap::new();

            // Preevaluate the selectors and group features by the selector they belong to.
            for feature in layer.features {
                let mut selector = crate::css::Selector::new()
                    .with_type("layer".to_string())
                    .with_any("name".to_string(), layer.name.to_string());
                
                for tag in feature.tags.chunks(2) {
                    let key = layer.keys[tag[0] as usize].to_string();
                    
                    match &key[..] {
                        "class" | "subclass" => {
                            let value = layer.values[tag[1] as usize].clone();
                            selector = selector.with_any(key, value.string_value.unwrap().to_string())
                        },
                        _ => (),
                    }
                }

                if let Some(value) = map.get_mut(&selector) {
                    value.push((feature.type_pb, feature.geometry));
                } else {
                    map.insert(selector, vec![(feature.type_pb, feature.geometry)]);
                }
            }

            // Transform all the features on a per selector basis.
            let mut features = vec![];
            for (selector, fs) in map {
                index_start_before = builder.get_current_index();
                for feature in fs {
                    {
                        let mut layer_collection = layer_collection.write().unwrap();
                        if !layer_collection.is_layer_set(layer_id) {
                            layer_collection.set_layer(layer_id);
                        }
                        current_feature_id = if let Some(feature_id) = layer_collection.get_feature_id(&selector) {
                            feature_id
                        } else {
                            layer_collection.add_feature(Feature::new(selector.clone()))
                        };
                        builder.set_current_feature_id(current_feature_id);
                    }

                    geometry_commands_to_drawable(
                        &mut builder,
                        feature.0,
                        &feature.1,
                        layer.extent as f32,
                        tile_id.z
                    );
                }

                features.push((current_feature_id, index_start_before..builder.get_current_index()));
            }

            // Add the layer info to the layer set.
            layers.push(crate::vector_tile::transform::Layer {
                name: layer.name.to_string(),
                id: layer_id,
                indices_range: index_start_before..builder.get_current_index(),
                features,
            });
        }

        Self {
            tile_id: tile_id.clone(),
            layers,
            mesh,
            extent,
        }
    }
}