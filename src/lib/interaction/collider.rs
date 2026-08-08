use parry2d::math::Vec2;
use std::{
    ops::Deref,
    sync::{Arc, RwLock},
};

use crate::{
    math::{Coord, Gpu, Camera, TileId},
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
        camera: &Camera,
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
            let inv = camera.tile_to_screen(tile_id).inverse();
            // The point in GPU coordinates.
            let screen_point = Coord::<Gpu>::new(
                point.0 / (camera.width / 2f32) - 1.0,
                point.1 / (camera.height / 2f32) - 1.0,
            );
            let global_point = inv.apply(screen_point);
            let tile_point = Vec2::new(global_point.x(), global_point.y()) * *extent;

            // Pick radius in tile-local units: map a fixed pixel tolerance through
            // the same inverse transform so points/lines stay clickable at any zoom.
            const PICK_TOLERANCE_PX: f32 = 6.0;
            let offset_point = Coord::<Gpu>::new(
                (point.0 + PICK_TOLERANCE_PX) / (camera.width / 2f32) - 1.0,
                point.1 / (camera.height / 2f32) - 1.0,
            );
            let offset_global = inv.apply(offset_point);
            let offset_tile = Vec2::new(offset_global.x(), offset_global.y()) * *extent;
            let radius = (offset_tile - tile_point).length();

            if tile_point.x >= 0.0
                && tile_point.x <= *extent
                && tile_point.y >= 0.0
                && tile_point.y <= *extent
            {
                if let Ok(collider) = collider.try_read()
                    && let Ok(objects) = objects.try_read()
                {
                    collider.get_hovered_objects(&tile_point, radius, &mut object_ids);

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
