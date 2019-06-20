use std::collections::HashMap;
use crate::vector_tile::math;

pub struct TileCache {
    cache: HashMap<math::TileId, Tile>,
}

impl TileCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn fetch_tiles(&mut self, screen: &math::Screen) {
        let tile_field = screen.get_tile_boundaries_for_zoom_level(8);
        tile_field.iter().for_each(|tile_id| {
            if !self.cache.contains_key(&tile_id) {
                dbg!(&tile_id);
                let data = crate::vector_tile::fetch_tile_data(&tile_id);
                let layers = crate::vector_tile::vector_tile_to_mesh(&tile_id, &data);
                self.cache.insert(tile_id, Tile { layers });
            }
        })
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
    pub layers: Vec<crate::render::Layer>
}