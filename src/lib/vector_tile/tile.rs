use crate::*;
use lyon::{
    path::Path,
    tessellation::{geometry_builder::VertexBuffers, FillOptions, FillTessellator},
};
use quick_protobuf::{BytesReader, MessageRead};
use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, RwLock};
use vector_tile::mod_Tile::GeomType;

fn format_size(value: &usize, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    write!(
        f,
        "{:.1}B",
        size_format::SizeFormatterSI::new(*value as u64)
    )
}

#[derive(Clone, Copy, Derivative)]
#[derivative(Debug)]
pub struct TileStats {
    pub objects: usize,
    pub features: usize,
    pub vertices: usize,
    pub indices: usize,
    #[derivative(Debug(format_with = "format_size"))]
    pub size: usize,
}

impl TileStats {
    pub fn new() -> Self {
        Self {
            objects: 0,
            features: 0,
            vertices: 0,
            indices: 0,
            size: 0,
        }
    }
}

impl std::ops::Add for TileStats {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Self {
            objects: self.objects + rhs.objects,
            features: self.features + rhs.features,
            vertices: self.vertices + rhs.vertices,
            indices: self.indices + rhs.indices,
            size: self.size + rhs.size,
        }
    }
}

impl std::ops::AddAssign for TileStats {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

pub struct Tile {
    tile_id: TileId,
    mesh: VertexBuffers<Vertex, u32>,
    extent: u16,
    objects: Arc<RwLock<Vec<Object>>>,
    features: Vec<(u32, Range<u32>)>,
    collider: Arc<RwLock<TileCollider>>,
    text: Vec<((f32, f32), String)>,
    stats: TileStats,
}

pub fn layer_num(name: &str) -> u32 {
    match name {
        "landcover" => 0,
        "water" => 1,
        "waterway" => 2,
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
    /// Create a new tile form a MBVT pbf file.
    ///
    /// Creates all the data necessecary to render the MBVT.
    /// This includes vertex and index buffers.
    pub fn from_mbvt(
        tile_id: &TileId,
        pbf_data: &Vec<u8>,
        feature_collection: Arc<RwLock<FeatureCollection>>,
        selection_tags: Vec<String>,
    ) -> Self {
        // Read tile data from the pbf data.
        let mut reader = BytesReader::from_bytes(&pbf_data);
        let tile = super::vector_tile::Tile::from_reader(&mut reader, &pbf_data)
            .expect("Cannot read Tile object.");

        let mut objects = Vec::new();
        let mut mesh: VertexBuffers<Vertex, u32> = VertexBuffers::with_capacity(10_000, 10_000);
        let mut builder = MeshBuilder::new(&mut mesh, LayerVertexCtor::new(tile_id, 1.0));
        let extent = tile.layers[0].extent as u16;
        let mut features = vec![];
        let mut text = vec![];

        // Add a background feature to the tile data.
        let (mut current_feature_id, object, range) =
            Self::create_background_feature(&mut builder, feature_collection.clone(), extent);
        features.push((current_feature_id, range));
        objects.push(object);

        // Transform all features of the tile.
        for layer in tile.layers {
            let mut map: std::collections::HashMap<Selector, Vec<(GeomType, Vec<Path>)>> =
                HashMap::new();

            // Preevaluate the selectors and group features by the selector they belong to.
            for feature in &layer.features {
                let (selector, tags) = Self::classify(&layer, &feature, &selection_tags);

                let paths = geometry_commands_to_paths(feature.type_pb, &feature.geometry);

                if let Some(tag) = tags.get("name:en") {
                    let point = paths[0].points()[0];
                    text.push((
                        (point.x / extent as f32, point.y / extent as f32),
                        tag.clone(),
                    ));
                }

                // If we have a valid object at hand, insert it into the object list

                let object_type = match feature.type_pb {
                    GeomType::POLYGON => Some(ObjectType::Polygon),
                    GeomType::LINESTRING => Some(ObjectType::Line),
                    GeomType::POINT => Some(ObjectType::Point),
                    _ => None,
                };

                if let Some(ot) = object_type {
                    objects.push(Object::new_with_tags(
                        selector.clone(),
                        paths[0].points().iter().cloned().collect(),
                        tags,
                        ot,
                    ));
                }

                if let Some(value) = map.get_mut(&selector) {
                    value.push((feature.type_pb, paths));
                } else {
                    map.insert(selector.clone(), vec![(feature.type_pb, paths)]);
                }
            }

            // Transform all the features on a per selector basis.
            let mut inner_features = vec![];
            for (selector, features) in map {
                let index_start_before = builder.get_current_index();
                for feature in features {
                    // Set the current feature id.
                    current_feature_id = {
                        // Scope the lock guard real tight to ensure it's released quickly.
                        let mut feature_collection = feature_collection.write().unwrap();
                        feature_collection.ensure_feature(&selector)
                    };
                    builder.set_current_feature_id(current_feature_id);

                    paths_to_drawable(
                        &mut builder,
                        feature.0,
                        &feature.1,
                        layer.extent as f32,
                        tile_id.z,
                    );
                }

                inner_features.push((
                    current_feature_id,
                    index_start_before..builder.get_current_index(),
                ));
            }

            features.extend(inner_features);
        }

        let collider = Arc::new(RwLock::new(TileCollider::new()));
        let collider_keep = collider.clone();
        let objects = Arc::new(RwLock::new(objects));
        let objects_keep = objects.clone();

        let stats = {
            let objects = objects
                .read()
                .expect("Failed to read initial objects. This is a bug. Please report it.");

            let feature_size = std::mem::size_of::<(u32, std::ops::Range<u32>)>();
            let vertex_size = std::mem::size_of::<Vertex>();
            let index_size = std::mem::size_of::<u32>();

            TileStats {
                objects: objects.len(),
                features: features.len(),
                vertices: mesh.vertices.len(),
                indices: mesh.indices.len(),
                size: objects.iter().map(|o| o.size()).sum::<usize>()
                    + features.capacity() * feature_size
                    + mesh.vertices.capacity() * vertex_size
                    + mesh.indices.capacity() * index_size,
            }
        };

        // spawn(move|| {
        //     if let Ok(objects) = objects_keep.read() {
        //         match collider_keep.write() {
        //             Ok(mut collider) => {
        //                 for object_id in 0..objects.len() {
        //                     if objects[object_id].points().len() >= 2 {
        //                         collider.add_object(object_id, &objects[object_id]);
        //                     }
        //                 }
        //                 collider.update();
        //             },
        //             Err(_e) => log::error!("Could not aquire collider lock. Not loading the objects of this tile."),
        //         }
        //     }
        // });

        Self {
            tile_id: tile_id.clone(),
            mesh,
            extent,
            objects,
            features,
            collider,
            text,
            stats,
        }
    }

    pub fn extent(&self) -> u16 {
        self.extent
    }

    pub fn collider(&self) -> Arc<RwLock<TileCollider>> {
        self.collider.clone()
    }

    pub fn objects(&self) -> Arc<RwLock<Vec<Object>>> {
        self.objects.clone()
    }

    pub fn mesh(&self) -> &VertexBuffers<Vertex, u32> {
        &self.mesh
    }

    pub fn features(&self) -> &Vec<(u32, Range<u32>)> {
        &self.features
    }

    pub fn text(&self) -> &Vec<((f32, f32), String)> {
        &self.text
    }

    pub fn stats(&self) -> &TileStats {
        &self.stats
    }

    pub fn tile_id(&self) -> TileId {
        self.tile_id
    }

    /// Creates a rectangle the size of a tile to be used as the background of a tile.
    ///
    /// Could also display a texture in the future (speak swisstopo).
    fn create_background_feature<'l>(
        builder: &mut MeshBuilder<'l>,
        feature_collection: Arc<RwLock<FeatureCollection>>,
        extent: u16,
    ) -> (u32, Object, Range<u32>) {
        // Create a new background type selector.
        let selector = Selector::new().with_type("background");

        // Create a rectangular path.
        let mut path_builder = Path::builder();
        path_builder.move_to((-10.0, -10.0).into());
        path_builder.line_to((-10.0, extent as f32 + 10.0).into());
        path_builder.line_to((extent as f32 + 10.0, extent as f32 + 10.0).into());
        path_builder.line_to((extent as f32 + 10.0, -10.0).into());
        path_builder.close();
        let path = path_builder.build();

        // Set the current feature id.
        let current_feature_id = {
            // Scope the lock guard real tight to ensure it's released quickly.
            let mut feature_collection = feature_collection.write().unwrap();
            feature_collection.ensure_feature(&selector)
        };
        builder.set_current_feature_id(current_feature_id);

        // Remember buffer index before.
        let index_start_before = builder.get_current_index();

        // Tesselate path.
        FillTessellator::new()
            .tessellate_path(
                &path,
                &FillOptions::tolerance(0.0001).with_normals(true),
                builder,
            )
            .expect("This is a bug. Please report it.");

        let object = Object::new(
            selector,
            path.points().iter().cloned().collect(),
            ObjectType::Polygon,
        );

        (
            current_feature_id,
            object,
            index_start_before..builder.get_current_index(),
        )
    }

    /// Create a selector and a list of tags form MBVT information.
    fn classify(
        layer: &vector_tile::mod_Tile::Layer,
        feature: &vector_tile::mod_Tile::Feature,
        selection_tags: &Vec<String>,
    ) -> (Selector, HashMap<String, String>) {
        let mut selector = Selector::new()
            .with_type("layer".to_string())
            .with_any("name".to_string(), layer.name.to_string());

        let mut tags = HashMap::new();

        for tag in feature.tags.chunks(2) {
            let key = layer.keys[tag[0] as usize].to_string();
            let value = layer.values[tag[1] as usize].clone();
            match &key[..] {
                "class" => {
                    selector
                        .classes
                        .push(value.string_value.clone().unwrap().to_string());
                }
                _ => {
                    if selection_tags.contains(&key) {
                        selector = selector.with_any(key.clone(), value.to_string());
                    } else {
                        tags.insert(key.clone(), value.to_string());
                    }
                }
            }
        }

        (selector, tags)
    }
}

impl<'a> std::string::ToString for vector_tile::mod_Tile::Value<'a> {
    fn to_string(&self) -> String {
        // We can make the safe assumption that only ever one property is Some(_).
        // So we just unwrap all the strings and concat it into one.
        self.string_value
            .clone()
            .map_or(String::new(), |v| v.to_string())
            + &self.float_value.map_or(String::new(), |v| v.to_string())
            + &self.double_value.map_or(String::new(), |v| v.to_string())
            + &self.int_value.map_or(String::new(), |v| v.to_string())
            + &self.uint_value.map_or(String::new(), |v| v.to_string())
            + &self.sint_value.map_or(String::new(), |v| v.to_string())
            + &self.bool_value.map_or(String::new(), |v| v.to_string())
    }
}
