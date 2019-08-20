use crate::drawing::feature_collection::FeatureCollection;
use crate::drawing::feature::Feature;
use std::sync::{
    Arc,
    RwLock,
};
use crate::vector_tile::transform::{
    geometry_commands_to_paths,
    paths_to_drawable
};
use crate::vector_tile::math::TileId;
use crate::drawing::mesh::MeshBuilder;
use crate::vector_tile::*;
use quick_protobuf::{MessageRead, BytesReader};
use crate::vector_tile::mod_Tile::GeomType;
use crate::vector_tile::object::ObjectType;
use lyon::tessellation::geometry_builder::{
    VertexBuffers,
};

use crate::drawing::vertex::{
    Vertex,
    LayerVertexCtor,
};

use lyon::path::Path;

pub struct Tile {
    pub tile_id: TileId,
    pub layers: Vec<crate::vector_tile::transform::Layer>,
    pub mesh: VertexBuffers<Vertex, u32>,
    pub extent: u16,
    pub objects: Vec<object::Object>,
    pub collider: crate::interaction::tile_collider::TileCollider,
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
    pub fn from_mbvt(
        tile_id: &math::TileId,
        data: &Vec<u8>,
        feature_collection: Arc<RwLock<FeatureCollection>>
    ) -> Self {
        // let t = std::time::Instant::now();

        // we can build a bytes reader directly out of the bytes
        let mut reader = BytesReader::from_bytes(&data);

        let tile = crate::vector_tile::Tile::from_reader(&mut reader, &data).expect("Cannot read Tile object.");
        // dbg!(t.elapsed().as_millis());

        let mut layers = Vec::with_capacity(tile.layers.len());
        let mut objects = Vec::new();
        let mut mesh: VertexBuffers<Vertex, u32> = VertexBuffers::with_capacity(100_000, 100_000);
        let mut builder = MeshBuilder::new(&mut mesh, LayerVertexCtor::new(tile_id, 1.0));
        let extent = tile.layers[0].extent as u16;

        for layer in tile.layers {
            let mut index_start_before = builder.get_current_index();
            let layer_id = layer_num(&layer.name);
            let mut current_feature_id = 0;

            let mut map: std::collections::HashMap<
                crate::css::Selector,
                Vec<(vector_tile::mod_Tile::GeomType, Vec<Path>)>
            > = std::collections::HashMap::new();

            // Preevaluate the selectors and group features by the selector they belong to.
            for feature in layer.features {
                let mut selector = crate::css::Selector::new()
                    .with_type("layer".to_string())
                    .with_any("name".to_string(), layer.name.to_string());

                let mut tags = std::collections::HashMap::<String, String>::new();
                
                for tag in feature.tags.chunks(2) {
                    let key = layer.keys[tag[0] as usize].to_string();
                    let value = layer.values[tag[1] as usize].clone();
                    match &key[..] {
                        "class" => {
                            selector.classes.push(value.string_value.clone().unwrap().to_string());
                        },
                        "subclass" => {
                            selector = selector.with_any(
                                key.clone(),
                                value.string_value.clone().unwrap().to_string()
                            )
                        },
                        _ => (),
                    }

                    tags.insert(key, {
                        value.string_value.map_or(String::new(), |v| v.to_string())
                     + &value.float_value.map_or(String::new(), |v| v.to_string())
                     + &value.double_value.map_or(String::new(), |v| v.to_string())
                     + &value.int_value.map_or(String::new(), |v| v.to_string())
                     + &value.uint_value.map_or(String::new(), |v| v.to_string())
                     + &value.sint_value.map_or(String::new(), |v| v.to_string())
                     + &value.bool_value.map_or(String::new(), |v| v.to_string())
                    });
                }

                let paths = geometry_commands_to_paths(
                    feature.type_pb,
                    &feature.geometry
                );

                let object_type = match feature.type_pb {
                    GeomType::POLYGON => Some(ObjectType::Polygon),
                    GeomType::LINESTRING => Some(ObjectType::Line),
                    GeomType::POINT => Some(ObjectType::Point),
                    _ => None,
                };
                
                object_type.map(|ot| objects.push(object::Object::new(
                    selector.clone(),
                    tags,
                    paths[0].points().iter().cloned().collect(),
                    ot
                )));

                if let Some(value) = map.get_mut(&selector) {
                    value.push((feature.type_pb, paths));
                } else {
                    map.insert(selector.clone(), vec![(feature.type_pb, paths)]);
                }
            }

            // Transform all the features on a per selector basis.
            let mut features = vec![];
            for (selector, fs) in map {
                index_start_before = builder.get_current_index();
                for feature in fs {
                    {
                        let mut feature_collection = feature_collection.write().unwrap();
                        current_feature_id = if let Some(feature_id) = feature_collection.get_feature_id(&selector) {
                            feature_id
                        } else {
                            feature_collection.add_feature(Feature::new(selector.clone(), layer_id))
                        };
                        builder.set_current_feature_id(current_feature_id);
                    }

                    paths_to_drawable(
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

        let mut collider = crate::interaction::tile_collider::TileCollider::new();

        for object_id in 0..objects.len() {
            if objects[object_id].points.len() >= 2 {
                collider.add_object(object_id, &objects[object_id]);
            }
        }
        collider.update();

        Self {
            tile_id: tile_id.clone(),
            layers,
            mesh,
            extent,
            objects,
            collider,
        }
    }
}