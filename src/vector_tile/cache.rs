use crate::drawing::feature_collection::FeatureCollection;
use std::sync::{
    Arc,
    RwLock,
};
use crate::vector_tile::tile::Tile;
use crate::vector_tile::math::TileId;
use std::collections::HashMap;
use crate::vector_tile::math;
use std::thread::{
    JoinHandle,
    spawn,
};
use std::sync::mpsc::{
    channel,
    Sender,
    Receiver,
};

pub struct TileCache {
    cache: HashMap<math::TileId, Tile>,
    loaders: Vec<(u64, JoinHandle<Option<Tile>>, TileId)>,
    channel: (Sender<u64>, Receiver<u64>),
    id: u64,
}

impl TileCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            loaders: vec![],
            channel: channel(),
            id: 0,
        }
    }

    pub fn fetch_tiles(&mut self) {
        for id in self.channel.1.try_iter() {
            let mut found = false;
            let mut i = 0;
            for loader in &self.loaders {
                if loader.0 == id {
                    found = true;
                    break;
                }
                i += 1;
            }
            if found {
                let loader = self.loaders.remove(i);
                match loader.1.join() {
                    Ok(tile) => {
                        if let Some(tile) = tile {
                            self.cache.insert(loader.2, tile);
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to join tile loader thread for {}. Reason:\r\n{:?}", loader.2, e);
                    }
                }
            }
        }
    }

    pub fn request_tile(&mut self, tile_id: &math::TileId, feature_collection: Arc<RwLock<FeatureCollection>>) {
        let id = self.id;
        self.id += 1;

        let loader = self.loaders.iter().filter(|l| l.2 == *tile_id).next();
        
        if !self.cache.contains_key(&tile_id) && loader.is_none() {
            let tile_id_clone = tile_id.clone();
            let tx = self.channel.0.clone();
            self.loaders.push((
                id,
                spawn(move|| {
                    if let Some(data) = crate::vector_tile::fetch_tile_data(&tile_id_clone) {
                        let tile = Tile::from_mbvt(&tile_id_clone, &data, feature_collection);
                        match tx.send(id) {
                            Err(_) => log::debug!("Could not send the tile load message. This most likely happened because the app was terminated."),
                            _ => (),
                        }
                        Some(tile)
                    } else {
                        None
                    }
                }),
                tile_id.clone(),
            ));
        }
    }

    pub fn try_get_tile(&mut self, tile_id: &math::TileId) -> Option<&Tile> {
        self.cache.get(&tile_id)
    }
}