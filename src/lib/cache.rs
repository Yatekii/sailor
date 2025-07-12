use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread::{spawn, JoinHandle};

use crate::feature::collection::FeatureCollection;
use crate::fetch::fetch_tile_data;
use crate::math::TileId;
use crate::vector_tile::tile::{Tile, TileStats};

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CacheStats {
    pub cached_tiles: usize,
    pub loading_tiles: usize,
    pub cached_objects: usize,
    pub cached_features: usize,
    pub cached_vertices: usize,
    pub total_stats: TileStats,
}

/// A cache structure to hold all loaded `Tile`s.
pub struct TileCache {
    /// The cache that holds all the tiles that were loaded to memory.
    cache: HashMap<TileId, Tile>,
    /// The loader thread handles of all active loaders.
    loaders: Vec<(TileId, JoinHandle<Option<Tile>>)>,
    /// The back-channel to signalize the loader when a loader thread finished.
    channel: (Sender<TileId>, Receiver<TileId>),
    /// The directory where all the PBF files for the tiles are stored.
    cache_location: String,
}

impl TileCache {
    /// Create a new `TileCache`.
    pub fn new(cache_location: String) -> Self {
        Self {
            cache: HashMap::new(),
            loaders: vec![],
            channel: channel(),
            cache_location,
        }
    }

    /// Check loaders for loaded tiles and insert them into the cache if there is any that finished loading.
    pub fn finalize_loaded_tiles(&mut self) {
        // Get all pending messages and work them.
        for id in self.channel.1.try_iter() {
            let potential_loader = self.loaders.iter().enumerate().find(|(_, l)| l.0 == id);

            // Try finalizing the complete loader.
            if let Some((i, _)) = potential_loader {
                let loader = self.loaders.remove(i);
                if loader.1.is_finished() {
                    match loader.1.join() {
                        Ok(tile) => {
                            if let Some(tile) = tile {
                                self.cache.insert(loader.0, tile);
                            }
                        }
                        Err(e) => {
                            log::error!("Loading tile {} failed. Reason:\r\n{:?}", loader.0, e);
                        }
                    }
                } else {
                    log::error!(
                        "Failed to join tile loader thread for {}. Dropping thread.",
                        loader.0,
                    );
                }
            }
        }
    }

    /// Load a specific tile into the cache if it is not present yet.
    ///
    /// This function does not actually render the loaded tile it only loads it from disk and preprocesses data.
    /// Use [`Self::finalize_loaded_data`] to load the tiles to the GPU and display them.
    pub fn load_tile(
        &mut self,
        tile_id: &TileId,
        feature_collection: Arc<RwLock<FeatureCollection>>,
        selection_tags: &[String],
    ) {
        // Find the corresponding loader to the requested tile if there is any.
        let loader = self.loaders.iter().find(|l| l.0 == *tile_id);

        // Check if tile is not in the cache yet and is not currently being loaded.
        if !self.cache.contains_key(tile_id) && loader.is_none() {
            // Make sure we load all tags we want to include.
            let selection_tags = selection_tags.to_vec();
            let cache_location = self.cache_location.clone();

            // Spawn a new loader.
            let handle = {
                let tile_id = *tile_id;
                let tx = self.channel.0.clone();
                spawn(move || {
                    // Try fetch and work the tile data.
                    let data = fetch_tile_data(Path::new(&cache_location), &tile_id)?;

                    // Create a new Tile from the fetched data.
                    let tile = Tile::from_mbvt(&tile_id, &data, feature_collection, selection_tags);

                    // Signal the end of the tile loading process.
                    if tx.send(tile_id).is_err() {
                        log::debug!("Could not send the tile load message. This most likely happened because the application process was terminated.")
                    }

                    Some(tile)
                })
            };

            // Store a new loader.
            self.loaders.push((*tile_id, handle));
        }
    }

    /// Get a `Tile` from the `TileCache`.
    ///
    /// Panics if the tile is not in the cache.
    /// The user has to request the loading of the `Tile` on their own.
    #[track_caller]
    pub fn get_tile(&self, tile_id: &TileId) -> &Tile {
        self.cache.get(tile_id).unwrap()
    }

    /// Get a `Tile` from the `TileCache`.
    ///
    /// Returns `None` if the tile is not in the cache.
    /// The user has to request the loading of the `Tile` on their own.
    pub fn try_get_tile(&self, tile_id: &TileId) -> Option<&Tile> {
        self.cache.get(tile_id)
    }

    /// Get a `Tile` from the `TileCache`.
    ///
    /// Panics if the tile is not in the cache.
    /// The user has to request the loading of the `Tile` on their own.
    #[track_caller]
    pub fn get_tile_mut<'a>(&'a mut self, tile_id: &TileId) -> &'a mut Tile {
        self.cache.get_mut(tile_id).unwrap()
    }

    /// Get a `Tile` from the `TileCache`.
    ///
    /// Returns `None` if the tile is not in the cache.
    /// The user has to request the loading of the `Tile` on their own.
    pub fn try_get_tile_mut(&mut self, tile_id: &TileId) -> Option<&mut Tile> {
        self.cache.get_mut(tile_id)
    }

    /// Gets the latest stats from the cache.
    pub fn get_stats(&self) -> CacheStats {
        let mut total_stats = TileStats::new();
        for tile in self.cache.values() {
            total_stats += *tile.stats();
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
