use std::{
    sync::{Arc, RwLock},
    time::Duration,
};

use osm::{feature::collection::FeatureCollection, math::TileId, vector_tile::tile::Tile};

fn main() {
    let tile_id = TileId::new(8, 142, 93);
    let data = include_bytes!("../data/8_142_93.pbf");
    let feature_collection = Arc::new(RwLock::new(FeatureCollection::new()));

    let mut tiles = vec![];
    for _ in 0..60 {
        let tile = Tile::from_mbvt(&tile_id, data, feature_collection.clone(), vec![]);
        tiles.push(tile);
    }

    loop {
        std::thread::sleep(Duration::from_secs(1));
    }
}
