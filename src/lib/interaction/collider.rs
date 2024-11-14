use nalgebra::base::Vector4;
use ncollide2d::math::Point;
use std::{
    ops::Deref,
    sync::{Arc, RwLock},
};

use crate::{
    math::{Screen, TileId},
    object::Object,
};

use super::tile_collider::TileCollider;

pub struct Collider {}

impl Collider {
    pub fn get_hovered_objects(
        visible_tiles: &[VisibleTile],
        screen: &Screen,
        zoom: f32,
        point: (f32, f32),
    ) -> Vec<Object> {
        let mut return_objects = vec![];

        for VisibleTile {
            tile_id,
            extent,
            collider,
            objects,
        } in visible_tiles
        {
            let matrix = screen.tile_to_global_space(zoom, tile_id);
            let matrix = nalgebra_glm::inverse(&matrix);
            let screen_point = Point::new(
                point.0 / (screen.width / 2) as f32 - 1.0,
                point.1 / (screen.height / 2) as f32 - 1.0,
            );
            let global_point = matrix * Vector4::new(screen_point.x, screen_point.y, 0.0, 1.0);
            let tile_point = Point::new(global_point.x, global_point.y) * *extent;

            if tile_point.x >= 0.0
                && tile_point.x <= *extent
                && tile_point.y >= 0.0
                && tile_point.y <= *extent
            {
                if let Ok(collider) = collider.try_read() {
                    if let Ok(objects) = objects.try_read() {
                        let object_ids = collider.get_hovered_objects(&tile_point);
                        for object_id in object_ids {
                            return_objects.push((objects.deref())[object_id].clone())
                        }
                    }
                }
                return return_objects;
            }
        }

        return_objects
    }
}

pub struct VisibleTile {
    pub tile_id: TileId,
    pub extent: f32,
    pub collider: Arc<RwLock<TileCollider>>,
    pub objects: Arc<RwLock<Vec<Object>>>,
}
