use std::num::NonZeroUsize;
use std::path::Path;
use std::sync::{Arc, RwLock};

use lru::LruCache;

use crate::feature::collection::FeatureCollection;
use crate::fetch::fetch_tile_data;
use crate::math::TileId;
use crate::platform::{Task, spawn_task};
use crate::vector_tile::tile::{Tile, TileStats};

const MAX_CACHE_ENTRIES: NonZeroUsize = NonZeroUsize::new(20).unwrap();

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct CacheStats {
    pub cached_tiles: usize,
    pub visible_tiles: usize,
    pub loading_tiles: usize,
    pub tile_stats: TileStats,
}

/// A cache structure to hold all loaded `Tile`s.
pub struct TileCache {
    /// The cache that holds all the tiles that were loaded to memory.
    cache: LruCache<TileId, Tile>,
    /// The in-flight loader tasks, each resolving to its finished tile.
    loaders: Vec<(TileId, Task<Option<Tile>>)>,
    /// The directory where all the PBF files for the tiles are stored.
    cache_location: String,
}

impl TileCache {
    /// Create a new `TileCache`.
    pub fn new(cache_location: String) -> Self {
        Self {
            cache: LruCache::new(MAX_CACHE_ENTRIES),
            // We should rarely ever load many tiles at once.
            loaders: Vec::with_capacity(32),
            cache_location,
        }
    }

    /// Insert any tiles whose loader task finished since the last call into the cache.
    pub fn finalize_loaded_tiles(&mut self) {
        let mut i = 0;
        while i < self.loaders.len() {
            if let Some(tile) = self.loaders[i].1.try_take() {
                let (id, _) = self.loaders.remove(i);
                if let Some(tile) = tile {
                    self.cache.put(id, tile);
                }
            } else {
                i += 1;
            }
        }
    }

    /// Load a specific tile into the cache if it is not present yet.
    ///
    /// This function does not actually render the loaded tile it only loads it from disk and preprocesses data.
    /// Use [`Self::finalize_loaded_tiles`] to load the tiles to the GPU and display them.
    pub fn load_tile(
        &mut self,
        tile_id: &TileId,
        feature_collection: Arc<RwLock<FeatureCollection>>,
        selection_tags: &[String],
    ) {
        // Skip tiles that are already cached or currently being loaded.
        if self.cache.contains(tile_id) || self.loaders.iter().any(|(id, _)| id == tile_id) {
            return;
        }

        let selection_tags = selection_tags.to_vec();
        let cache_location = self.cache_location.clone();
        let tile_id = *tile_id;

        // Spawn a loader task that fetches and preprocesses the tile; its handle
        // resolves to the finished tile, which is picked up in `finalize`.
        let task = spawn_task(async move {
            fetch_tile_data(Path::new(&cache_location), &tile_id)
                .await
                .map(|data| Tile::from_mbvt(&tile_id, &data, feature_collection, selection_tags))
        });
        self.loaders.push((tile_id, task));
    }

    /// Get a `Tile` from the `TileCache`.
    ///
    /// Panics if the tile is not in the cache.
    /// The user has to request the loading of the `Tile` on their own.
    #[track_caller]
    pub fn get_tile(&self, tile_id: &TileId) -> &Tile {
        self.cache.peek(tile_id).unwrap()
    }

    /// Get a `Tile` from the `TileCache`.
    ///
    /// Returns `None` if the tile is not in the cache.
    /// The user has to request the loading of the `Tile` on their own.
    pub fn try_get_tile(&self, tile_id: &TileId) -> Option<&Tile> {
        self.cache.peek(tile_id)
    }

    /// Marks an item as least recently used.
    pub fn promote(&mut self, tile_id: &TileId) {
        self.cache.promote(tile_id);
    }

    /// Get a `Tile` from the `TileCache`.
    ///
    /// Panics if the tile is not in the cache.
    /// The user has to request the loading of the `Tile` on their own.
    #[track_caller]
    pub fn get_tile_mut<'a>(&'a mut self, tile_id: &TileId) -> &'a mut Tile {
        self.cache.peek_mut(tile_id).unwrap()
    }

    /// Get a `Tile` from the `TileCache`.
    ///
    /// Returns `None` if the tile is not in the cache.
    /// The user has to request the loading of the `Tile` on their own.
    pub fn try_get_tile_mut(&mut self, tile_id: &TileId) -> Option<&mut Tile> {
        self.cache.peek_mut(tile_id)
    }

    /// Gets the latest stats from the cache.
    pub fn get_stats(&self, visible_tiles: &[TileId]) -> CacheStats {
        let mut total_stats = TileStats::new();
        for (_, tile) in self.cache.iter() {
            total_stats += *tile.stats();
        }
        CacheStats {
            cached_tiles: self.cache.len(),
            visible_tiles: visible_tiles.len(),
            loading_tiles: self.loaders.len(),
            tile_stats: total_stats,
        }
    }
}
