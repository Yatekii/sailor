use parry2d::{
    bounding_volume::Aabb,
    math::Vec2,
    partitioning::{Bvh, BvhBuildStrategy},
};
use std::sync::{Arc, RwLock};

use crate::geometry::{Geometry, Polygon};
use crate::object::Object;
use crate::platform::spawn;

/// One collidable object: the index of the source object plus the polygon we
/// hit-test against. Non-polygon objects don't produce a collision object.
struct CollisionObject {
    object_id: usize,
    polygon: Polygon,
}

pub struct TileCollider {
    // One entry per collidable object. The BVH leaf index addresses this vector,
    // and each entry carries the id of the object it came from.
    objects: Vec<CollisionObject>,
    bvh: Bvh,
}

impl TileCollider {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            bvh: Bvh::new(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get_hovered_objects(&self, cursor_point: &Vec2, hovered_objects: &mut Vec<usize>) {
        // Broad phase against the object aabbs, narrow phase against the polygon itself.
        for leaf in self
            .bvh
            .leaves(|node| node.aabb().contains_local_point(*cursor_point))
        {
            let object = &self.objects[leaf as usize];
            if object.polygon.contains(*cursor_point) {
                hovered_objects.push(object.object_id);
            }
        }
    }
}

impl Default for TileCollider {
    fn default() -> Self {
        Self::new()
    }
}

pub trait TileColliderLoader {
    fn load(&mut self, objects: Arc<RwLock<Vec<Object>>>);
}

impl TileColliderLoader for Arc<RwLock<TileCollider>> {
    fn load(&mut self, objects: Arc<RwLock<Vec<Object>>>) {
        let collider_clone = self.clone();
        spawn(async move {
            if let Ok(objects) = objects.read() {
                let mut collision_objects: Vec<CollisionObject> = Vec::new();
                let mut aabbs: Vec<Aabb> = Vec::new();
                for (object_id, object) in objects.iter().enumerate() {
                    // Only polygons enclose an area. Points (e.g. multipoint housenumber
                    // features) and lines would otherwise be treated as fake polygons.
                    let Geometry::Polygon(polygon) = object.geometry() else {
                        continue;
                    };

                    if polygon.is_empty() {
                        continue;
                    }

                    aabbs.push(polygon.aabb());
                    collision_objects.push(CollisionObject {
                        object_id,
                        polygon: polygon.clone(),
                    });
                }
                // Build the tree in one shot; incremental `insert` leaves the tree unbalanced
                // and `leaves` then yields internal node indices instead of leaf data.
                let bvh = Bvh::from_leaves(BvhBuildStrategy::Binned, &aabbs);
                match collider_clone.write() {
                    Ok(mut collider) => {
                        collider.objects = collision_objects;
                        collider.bvh = bvh;
                    }
                    Err(_e) => log::error!(
                        "Could not aquire collider lock. Not loading the objects of this tile."
                    ),
                }
            }
        });
    }
}
