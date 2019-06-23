use std::collections::HashMap;
use crate::vector_tile::math;
use rayon::prelude::*;

pub struct TileCache {
    cache: HashMap<math::TileId, Tile>,
}

impl TileCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn fetch_tile(&mut self, tile_id: &math::TileId) {
        if !self.cache.contains_key(&tile_id) {
            let data = crate::vector_tile::fetch_tile_data(&tile_id);
            let layers = crate::vector_tile::vector_tile_to_mesh(&tile_id, &data);
            self.cache.insert(tile_id.clone(), Tile { layers: layers });
        }
    }

    pub fn try_get_tile(&mut self, tile_id: &math::TileId) -> Option<&Tile> {
        self.cache.get(&tile_id)
    }

    pub fn fetch_tiles(&mut self, screen: &math::Screen) {
        let tile_field = screen.get_tile_boundaries_for_zoom_level(8).iter().collect::<Vec<_>>();
        let data = tile_field.par_iter().filter_map(|tile_id| {
            if !self.cache.contains_key(&tile_id) {
                let data = crate::vector_tile::fetch_tile_data(&tile_id);
                let layers = crate::vector_tile::vector_tile_to_mesh(&tile_id, &data);
                return Some((tile_id.clone(), layers));
            }
            None
        }).collect::<Vec<_>>();

        for d in data {
            self.cache.insert(d.0.clone(), Tile { layers: d.1 });
        }
    }

    pub fn get_tiles(&mut self, screen: &math::Screen) -> Vec<Tile> {
        let tile_field = screen.get_tile_boundaries_for_zoom_level(8);
        tile_field.iter().map(|tile_id| {
            let tile = self.cache.get(&tile_id).unwrap();
            tile.clone()
        }).collect::<Vec<Tile>>()
    }
}

#[derive(Debug, Clone)]
pub struct Tile {
    pub layers: Vec<crate::vector_tile::transform::Layer>
}

#[test]
fn test_cache() {
    let mut cache = TileCache::new();
    let z = 8;
    let tile_coordinate = math::deg2num(47.3769, 8.5417, z);
    let zurich = math::num_to_global_space(&tile_coordinate);
    let mut screen = math::Screen::new(zurich, 600, 600);

    cache.fetch_tiles(&screen);
}