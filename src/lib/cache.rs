use super::*;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread::{spawn, JoinHandle};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CacheStats {
    cached_tiles: usize,
    loading_tiles: usize,
    cached_objects: usize,
    cached_features: usize,
    cached_vertices: usize,
    total_stats: TileStats,
}

/// A cache structure to hold all loaded `Tile`s.
pub struct TileCache {
    cache: HashMap<TileId, Arc<RwLock<Tile>>>,
    loaders: Vec<(u64, JoinHandle<Option<Tile>>, TileId)>,
    channel: (Sender<u64>, Receiver<u64>),
    cache_location: String,
    id: u64,
}

impl TileCache {
    /// Create a new `TileCache`.
    pub fn new(cache_location: String) -> Self {
        Self {
            cache: HashMap::new(),
            loaders: vec![],
            channel: channel(),
            cache_location,
            id: 0,
        }
    }

    /// Check loaders for loaded tiles and insert them if there is any.
    pub fn finalize_loaded_tiles(&mut self) {
        // Get all pending messages and work them.
        for id in self.channel.1.try_iter() {
            let potential_loader = self.loaders.iter().enumerate().find(|(_, l)| l.0 == id);

            // Try finalizing the complete loader.
            if let Some((i, _)) = potential_loader {
                let loader = self.loaders.remove(i);
                match loader.1.join() {
                    Ok(tile) => {
                        if let Some(tile) = tile {
                            self.cache.insert(loader.2, Arc::new(RwLock::new(tile)));
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "Failed to join tile loader thread for {}. Reason:\r\n{:?}",
                            loader.2,
                            e
                        );
                    }
                }
            }
        }
    }

    /// Request a tile from the cache.
    pub fn request_tile(
        &mut self,
        tile_id: &TileId,
        feature_collection: Arc<RwLock<FeatureCollection>>,
        selection_tags: &[String],
    ) {
        let id = self.id;
        self.id += 1;

        // Find the corresponding loader to the requested tile if there is any.
        let loader = self.loaders.iter().find(|l| l.2 == *tile_id);

        // Check if tile is not in the cache yet and is not currently being loaded.
        if !self.cache.contains_key(tile_id) && loader.is_none() {
            // Clone values to be moved into the thread.
            let tile_id_clone = *tile_id;
            let tx = self.channel.0.clone();

            // Make sure we load all tags we want to include.
            let selection_tags = selection_tags.to_vec();
            let cache_location = self.cache_location.clone();

            // Store a new loader.
            self.loaders.push((
                id,
                // Spawn a new loader.
                spawn(move|| {
                    // Try fetch and work the tile data.
                    if let Some(data) = fetch_tile_data(cache_location, &tile_id_clone) {
                        // Create a new Tile from the fetched data.
                        let tile = Tile::from_mbvt(&tile_id_clone, &data, feature_collection, selection_tags);
                        // Signalize that the end of the tile loading process could not be signalized.
                       if tx.send(id).is_err() { log::debug!("Could not send the tile load message. This most likely happened because the app was terminated.") }
                        Some(tile)
                    } else {
                        None
                    }
                }),
                *tile_id,
            ));
        }
    }

    /// Get a `Tile` from the `TileCache`.
    ///
    /// Returns `None` if the tile is not in the cache.
    /// The user has to request the loading of the `Tile` on their own.
    pub fn try_get_tile(&self, tile_id: &TileId) -> Option<Arc<RwLock<Tile>>> {
        self.cache.get(tile_id).cloned()
    }

    pub fn get_stats(&self) -> CacheStats {
        let mut total_stats = TileStats::new();
        for tile in &self.cache {
            let read_tile = tile.1.read().unwrap();
            total_stats += *read_tile.stats();
        }
        CacheStats {
            cached_tiles: self.cache.len(),
            loading_tiles: self.loaders.len(),
            cached_objects: 0,
            cached_features: 0,
            cached_vertices: 0,
            total_stats,
        }
    }
}
