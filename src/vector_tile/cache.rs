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
    loaders: Vec<(u64, JoinHandle<std::vec::Vec<crate::vector_tile::transform::Layer>>, TileId)>,
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

    pub fn fetch_tile(&mut self, tile_id: &math::TileId) {
        match self.channel.1.try_recv() {
            Ok(id) => {
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
                    if let Ok(layers) = loader.1.join() {
                        self.cache.insert(loader.2, Tile { layers: layers });
                    } else {
                        log::error!("Failed to join tile loader thread. This could be a bug.");
                    }
                }
            },
            Err(_) => ()
        }
        
        // dbg!(self.loaders.len());

        let id = self.id;
        self.id += 1;

        let loader = self.loaders.iter().filter(|l| l.2 == *tile_id).next();
        
        if !self.cache.contains_key(&tile_id) && loader.is_none() {
            let tile_id_clone = tile_id.clone();
            let tx = self.channel.0.clone();
            self.loaders.push((
                id,
                spawn(move|| {
                    let data = crate::vector_tile::fetch_tile_data(&tile_id_clone);
                    let layers = crate::vector_tile::vector_tile_to_mesh(&tile_id_clone, &data);
                    tx.send(id).unwrap();
                    layers
                }),
                tile_id.clone(),
            ));
        }
    }

    pub fn try_get_tile(&mut self, tile_id: &math::TileId) -> Option<&Tile> {
        self.cache.get(&tile_id)
    }
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub layers: Vec<crate::vector_tile::transform::Layer>
}