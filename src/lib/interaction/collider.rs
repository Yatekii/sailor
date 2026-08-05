use nalgebra::Point2 as Point;
use nalgebra::base::Vector4;
use parry2d::math::Vec2;
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
    /// Get the hovered objects
    ///
    /// * point: The pointer in logical coordinates (divided by DPI ratio)
    pub fn get_hovered_objects(
        visible_tiles: &[VisibleTile],
        screen: &Screen,
        zoom: f32,
        point: (f32, f32),
    ) -> Vec<Object> {
        let mut object_ids = Vec::with_capacity(200);
        let mut return_objects = Vec::with_capacity(200);

        for VisibleTile {
            tile_id,
            extent,
            collider,
            objects,
        } in visible_tiles
        {
            let matrix = screen.tile_to_screen(zoom, tile_id);
            let matrix = nalgebra_glm::inverse(&matrix);
            // The point in GPU coordinates.
            let screen_point = Point::new(
                point.0 / (screen.width / 2f32) - 1.0,
                point.1 / (screen.height / 2f32) - 1.0,
            );
            let global_point = matrix * Vector4::new(screen_point.x, screen_point.y, 0.0, 1.0);
            let tile_point = Vec2::new(global_point.x, global_point.y) * *extent;

            if tile_point.x >= 0.0
                && tile_point.x <= *extent
                && tile_point.y >= 0.0
                && tile_point.y <= *extent
            {
                if let Ok(collider) = collider.try_read()
                    && let Ok(objects) = objects.try_read()
                {
                    collider.get_hovered_objects(&tile_point, &mut object_ids);

                    for object_id in &object_ids {
                        // The collider is loaded asynchronously, so it can briefly disagree
                        // with the object list; skip stale indices instead of panicking.
                        if let Some(object) = objects.deref().get(*object_id) {
                            return_objects.push(object.clone())
                        }
                    }

                    object_ids.clear();
                }

                break;
            }
        }

        // We return an empty vector.
        return_objects
    }
}

pub struct VisibleTile {
    pub tile_id: TileId,
    pub extent: f32,
    pub collider: Arc<RwLock<TileCollider>>,
    pub objects: Arc<RwLock<Vec<Object>>>,
}

impl std::fmt::Debug for VisibleTile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VisibleTile")
            .field("tile_id", &self.tile_id)
            .field("extent", &self.extent)
            .field("collider", &self.collider.read().unwrap().len())
            .field("objects", &self.objects.read().unwrap().len())
            .finish()
    }
}
