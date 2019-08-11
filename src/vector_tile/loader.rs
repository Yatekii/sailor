// use crate::app_state::AppState;
// use crate::drawing::drawable_tile::DrawableTile;
// use crate::vector_tile::math::TileId;
// use std::collections::BTreeMap;

// pub struct TileLoader {
//     loaded_tiles: BTreeMap<TileId, DrawableTile>
// }

// impl TileLoader {
//     fn load_tiles(&mut self, app_state: &mut AppState) {
//         let tile_field = app_state.screen.get_tile_boundaries_for_zoom_level(app_state.zoom, 1);

//         // Remove old bigger tiles which are not in the FOV anymore.
//         let old_tile_field = app_state.screen.get_tile_boundaries_for_zoom_level(app_state.zoom - 1.0, 2);

//         let key_iter: Vec<_> = self.loaded_tiles.keys().copied().collect();
//         for key in key_iter {
//             if key.z == (app_state.zoom - 1.0) as u32 {
//                 if !old_tile_field.contains(&key) {
//                     self.loaded_tiles.remove(&key);
//                 }
//             } else {
//                 if !tile_field.contains(&key) {
//                     self.loaded_tiles.remove(&key);
//                 }
//             }
//         }

//         app_state.tile_cache.fetch_tiles();
//         for tile_id in tile_field.iter() {
//             if !self.loaded_tiles.contains_key(&tile_id) {
//                 app_state.tile_cache.request_tile(&tile_id, self.feature_collection.clone());
                
//                 let tile_cache = &mut app_state.tile_cache;
//                 if let Some(tile) = tile_cache.try_get_tile(&tile_id) {

//                     let drawable_tile = DrawableTile::load_from_tile_id(
//                         &self.device,
//                         tile_id,
//                         &tile,
//                     );

//                     self.loaded_tiles.insert(
//                         tile_id.clone(),
//                         drawable_tile
//                     );

//                     // Remove old bigger tile when all 4 smaller tiles are loaded.
//                     let mut count = 0;
//                     let num_x = (tile_id.x / 2) * 2;
//                     let num_y = (tile_id.y / 2) * 2;
//                     for tile_id in &[
//                         TileId::new(tile_id.z, num_x, num_y),
//                         TileId::new(tile_id.z, num_x + 1, num_y),
//                         TileId::new(tile_id.z, num_x + 1, num_y + 1),
//                         TileId::new(tile_id.z, num_x, num_y + 1),
//                     ] {
//                         if !tile_field.contains(tile_id) {
//                             count += 1;
//                             continue;
//                         }
//                         if self.loaded_tiles.contains_key(tile_id) {
//                             count += 1;
//                         }
//                     }
//                     if count == 4 {
//                         self.loaded_tiles.remove(&TileId::new(tile_id.z - 1, num_x / 2, num_y / 2));
//                     }

//                     // Remove old smaller tiles when all 4 smaller tiles are loaded.
//                     for tile_id in &[
//                         TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2),
//                         TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2),
//                         TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2 + 1),
//                         TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2 + 1),
//                     ] {
//                         self.loaded_tiles.remove(tile_id);
//                     }
//                 } else {
//                     log::trace!("Could not read tile {} from cache.", tile_id);
//                 }
//             }
//         }

//         let mut feature_collection = self.feature_collection.write().unwrap();
//         feature_collection.load_styles(app_state.zoom, &mut app_state.css_cache);
//     }
// }