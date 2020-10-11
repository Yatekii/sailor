use nalgebra::base::Vector4;
use ncollide2d::math::Point;
use std::collections::BTreeMap;

use crate::*;

pub struct Collider {}

impl Collider {
    pub fn get_hovered_objects(
        visible_tiles: &BTreeMap<TileId, VisibleTile>,
        screen: &Screen,
        zoom: f32,
        point: (f32, f32),
    ) -> Vec<Object> {
        let mut return_objects = vec![];
        let tile_field = screen.get_tile_boundaries_for_zoom_level(zoom, 1);

        for tile_id in tile_field.iter() {
            if let Some(visible_tile) = visible_tiles.get(&tile_id) {
                let extent = visible_tile.extent() as f32;
                let matrix = screen.tile_to_global_space(zoom, &tile_id);
                let matrix = nalgebra_glm::inverse(&matrix);
                let screen_point = Point::new(
                    point.0 / (screen.width / 2) as f32 - 1.0,
                    point.1 / (screen.height / 2) as f32 - 1.0,
                );
                let global_point = matrix * Vector4::new(screen_point.x, screen_point.y, 0.0, 1.0);
                let tile_point = Point::new(global_point.x, global_point.y) * extent;

                if tile_point.x >= 0.0
                    && tile_point.x <= extent
                    && tile_point.y >= 0.0
                    && tile_point.y <= extent
                {
                    if let Ok(collider) = visible_tile.collider().try_read() {
                        if let Ok(objects) = visible_tile.objects().try_read() {
                            let object_ids = collider.get_hovered_objects(&tile_point);
                            for object_id in object_ids {
                                return_objects.push(objects[object_id].clone())
                            }
                        }
                    }
                    return return_objects;
                }
            } else {
                log::trace!(
                    "[Intersection pass] Could not read tile {} from cache.",
                    tile_id
                );
            }
        }

        return_objects
    }
}
