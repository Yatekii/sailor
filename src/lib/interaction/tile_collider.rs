use parry2d::{bounding_volume::Aabb, math::Vec2, partitioning::Bvh, utils::point_in_poly2d};
use std::{
    sync::{Arc, RwLock},
    thread::spawn,
};

use crate::object::Object;

pub struct TileCollider {
    objects: Vec<Vec<Vec2>>,
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
        // Broad phase only checks with the aabbs of the individual polys.
        for poly_id in self
            .bvh
            .leaves(|node| node.aabb().contains_local_point(*cursor_point))
        {
            let poly = &self.objects[poly_id as usize];
            // Narrow phase checks that the point is in the polygon indeed.
            if point_in_poly2d(*cursor_point, poly) {
                hovered_objects.push(poly_id as usize);
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
        spawn(move || {
            if let Ok(objects) = objects.read() {
                match collider_clone.write() {
                    Ok(mut collider) => {
                        for object_id in 0..objects.len() {
                            let object = &objects[object_id];
                            if object.points().len() >= 2 {
                                let polygon = object
                                    .points()
                                    .iter()
                                    .map(|p| Vec2::new(p.x, p.y))
                                    .collect::<Vec<Vec2>>();
                                let id = collider.objects.len();
                                collider.objects.push(polygon.clone());
                                collider.bvh.insert(Aabb::from_points(polygon), id as u32);
                            }
                        }
                    }
                    Err(_e) => log::error!(
                        "Could not aquire collider lock. Not loading the objects of this tile."
                    ),
                }
            }
        });
    }
}
