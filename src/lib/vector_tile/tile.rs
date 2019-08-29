use std::sync::{
    Arc,
    RwLock,
};
use std::collections::HashMap;
use std::ops::Range;
use quick_protobuf::{
    MessageRead,
    BytesReader
};
use vector_tile::mod_Tile::GeomType;
use lyon::{
    tessellation::{
        geometry_builder::{
            VertexBuffers,
        },
        FillTessellator,
        FillOptions,
    },
    path::Path,
};
use crate::*;


pub struct Tile {
    pub tile_id: TileId,
    pub mesh: VertexBuffers<Vertex, u32>,
    pub extent: u16,
    pub objects: Vec<Object>,
    pub features: Vec<(u32, Range<u32>)>,
    pub collider: TileCollider,
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
        tile_id: &TileId,
        data: &Vec<u8>,
        feature_collection: Arc<RwLock<FeatureCollection>>,
        selection_tags: Vec<String>
    ) -> Self {
        // let t = std::time::Instant::now();

        // we can build a bytes reader directly out of the bytes
        let mut reader = BytesReader::from_bytes(&data);

        let tile = super::vector_tile::Tile::from_reader(&mut reader, &data).expect("Cannot read Tile object.");

        let mut objects = Vec::new();
        let mut mesh: VertexBuffers<Vertex, u32> = VertexBuffers::with_capacity(100_000, 100_000);
        let mut builder = MeshBuilder::new(&mut mesh, LayerVertexCtor::new(tile_id, 1.0));
        let extent = tile.layers[0].extent as u16;
        let mut features = vec![];
        let mut current_feature_id;

        // Add a background rectangle to each tile
        let selector = Selector::new().with_type("background");

        let mut path_builder = Path::builder();
        path_builder.move_to((-10.0, -10.0).into());
        path_builder.line_to((-10.0, extent as f32 + 10.0).into());
        path_builder.line_to((extent as f32 + 10.0, extent as f32 + 10.0).into());
        path_builder.line_to((extent as f32 + 10.0, -10.0).into());
        path_builder.close();
        let path = path_builder.build();
        let index_start_before = builder.get_current_index();
        {
            let mut feature_collection = feature_collection.write().unwrap();
            current_feature_id = if let Some(feature_id) = feature_collection.get_feature_id(&selector) {
                feature_id
            } else {
                feature_collection.add_feature(Feature::new(selector.clone(), 0))
            };
            builder.set_current_feature_id(current_feature_id);
        }

        FillTessellator::new().tessellate_path(
            &path,
            &FillOptions::tolerance(0.0001).with_normals(true),
            &mut builder,
        ).expect("This is a bug. Please report it.");

        features.push((current_feature_id, index_start_before..builder.get_current_index()));

        objects.push(Object::new(
            selector,
            path.points().iter().cloned().collect(),
            ObjectType::Polygon
        ));

        // Transform all features of the tile.

        for layer in tile.layers {
            let layer_id = layer_num(&layer.name);

            let mut map: std::collections::HashMap<Selector, Vec<(GeomType, Vec<Path>)>> = HashMap::new();

            // Preevaluate the selectors and group features by the selector they belong to.
            for feature in layer.features {
                let mut selector = Selector::new()
                    .with_type("layer".to_string())
                    .with_any("name".to_string(), layer.name.to_string());
                
                let mut tags = std::collections::HashMap::new();

                for tag in feature.tags.chunks(2) {
                    let key = layer.keys[tag[0] as usize].to_string();
                    let value = layer.values[tag[1] as usize].clone();
                    if selection_tags.contains(&key) {
                        match &key[..] {
                            "class" => {
                                selector.classes.push(value.string_value.clone().unwrap().to_string());
                            },
                            _ => {
                                selector = selector.with_any(key.clone(), {
                                    value.string_value.map_or(String::new(), |v| v.to_string())
                                    + &value.float_value.map_or(String::new(), |v| v.to_string())
                                    + &value.double_value.map_or(String::new(), |v| v.to_string())
                                    + &value.int_value.map_or(String::new(), |v| v.to_string())
                                    + &value.uint_value.map_or(String::new(), |v| v.to_string())
                                    + &value.sint_value.map_or(String::new(), |v| v.to_string())
                                    + &value.bool_value.map_or(String::new(), |v| v.to_string())
                                });
                            },
                        }
                    } else {
                        tags.insert(key.clone(), {
                            value.string_value.map_or(String::new(), |v| v.to_string())
                            + &value.float_value.map_or(String::new(), |v| v.to_string())
                            + &value.double_value.map_or(String::new(), |v| v.to_string())
                            + &value.int_value.map_or(String::new(), |v| v.to_string())
                            + &value.uint_value.map_or(String::new(), |v| v.to_string())
                            + &value.sint_value.map_or(String::new(), |v| v.to_string())
                            + &value.bool_value.map_or(String::new(), |v| v.to_string())
                        });
                    }
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
                
                object_type.map(|ot| objects.push(Object::new_with_tags(
                    selector.clone(),
                    paths[0].points().iter().cloned().collect(),
                    tags,
                    ot
                )));

                if let Some(value) = map.get_mut(&selector) {
                    value.push((feature.type_pb, paths));
                } else {
                    map.insert(selector.clone(), vec![(feature.type_pb, paths)]);
                }
            }

            // Transform all the features on a per selector basis.
            let mut inner_features = vec![];
            for (selector, fs) in map {
                let index_start_before = builder.get_current_index();
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

                inner_features.push((current_feature_id, index_start_before..builder.get_current_index()));
            }

            features.extend(inner_features);
        }

        let mut collider = TileCollider::new();

        for object_id in 0..objects.len() {
            if objects[object_id].points.len() >= 2 {
                collider.add_object(object_id, &objects[object_id]);
            }
        }
        collider.update();

        Self {
            tile_id: tile_id.clone(),
            mesh,
            extent,
            objects,
            features,
            collider,
        }
    }
}