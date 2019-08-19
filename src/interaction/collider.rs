use ncollide2d::math::Point;
use crate::vector_tile::object::Object;
use crate::app_state::AppState;
use nalgebra::base::Vector4;

pub struct Collider {
}

impl Collider {
    pub fn get_hovered_objects<'a>(app_state: &'a AppState, point: (f32, f32)) -> Vec<&'a Object> {
        let mut objects = vec![];
        let tile_field = app_state.screen.get_tile_boundaries_for_zoom_level(app_state.zoom, 1);

        let tile_cache = &app_state.tile_cache;

        for tile_id in tile_field.iter() {
            if let Some(tile) = tile_cache.try_get_tile(&tile_id) {
                let matrix = app_state.screen.tile_to_global_space(
                    app_state.zoom,
                    &tile_id
                );
                let matrix = nalgebra_glm::inverse(&matrix);
                let screen_point = Point::new(
                    point.0 / (app_state.screen.width / 2) as f32 - 1.0,
                    point.1 / (app_state.screen.height / 2) as f32 - 1.0
                );
                let global_point = matrix * Vector4::new(screen_point.x, screen_point.y, 0.0, 1.0);
                let tile_point = Point::new(global_point.x, global_point.y) * tile.extent as f32;

                if tile_point.x >= 0.0 && tile_point.x <= tile.extent as f32
                && tile_point.y >= 0.0 && tile_point.y <= tile.extent as f32 {
                    let object_ids = tile.collider.get_hovered_objects(&tile_point);
                    for object_id in object_ids {
                        objects.push(&tile.objects[object_id])
                    }
                    return objects
                }
            } else {
                log::trace!("[Intersection pass] Could not read tile {} from cache.", tile_id);
            }
        }

        objects
    }
}