use parry2d::{
    bounding_volume::Aabb,
    math::Vec2,
    partitioning::{Bvh, BvhBuildStrategy},
    utils::point_in_poly2d,
};
use std::sync::{Arc, RwLock};

use crate::object::Object;
use crate::platform::spawn;

pub struct TileCollider {
    objects: Vec<Vec<Vec2>>,
    // Original object id for each collider entry (objects without enough points are skipped,
    // so collider indices are compacted and don't match the object list one-to-one).
    object_ids: Vec<usize>,
    bvh: Bvh,
}

impl TileCollider {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            object_ids: Vec::new(),
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
        // Broad phase only checks with the aabbs of the individual polys.
        for poly_id in self
            .bvh
            .leaves(|node| node.aabb().contains_local_point(*cursor_point))
        {
            let poly = &self.objects[poly_id as usize];
            // Narrow phase checks that the point is in the polygon indeed.
            if point_in_poly2d(*cursor_point, poly) {
                hovered_objects.push(self.object_ids[poly_id as usize]);
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
                let mut polygons: Vec<Vec<Vec2>> = Vec::new();
                let mut object_ids: Vec<usize> = Vec::new();
                let mut aabbs: Vec<Aabb> = Vec::new();
                for (object_id, object) in objects.iter().enumerate() {
                    if object.points().len() >= 2 {
                        let polygon: Vec<Vec2> = object
                            .points()
                            .iter()
                            .map(|p| Vec2::new(p.x, p.y))
                            .collect();
                        aabbs.push(Aabb::from_points(polygon.iter().copied()));
                        polygons.push(polygon);
                        object_ids.push(object_id);
                    }
                }
                // Build the tree in one shot; incremental `insert` leaves the tree unbalanced
                // and `leaves` then yields internal node indices instead of leaf data.
                let bvh = Bvh::from_leaves(BvhBuildStrategy::Binned, &aabbs);
                match collider_clone.write() {
                    Ok(mut collider) => {
                        collider.objects = polygons;
                        collider.object_ids = object_ids;
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
