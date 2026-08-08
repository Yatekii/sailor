use parry2d::{
    bounding_volume::{Aabb, BoundingVolume},
    math::Vec2,
    partitioning::{Bvh, BvhBuildStrategy},
};
use std::sync::{Arc, RwLock};

use crate::geometry::{Geometry, Polygon};
use crate::object::Object;
use crate::platform::spawn;

/// The shape a collision object is hit-tested against. Polygons test containment;
/// points and lines have no area, so they test within a pick radius instead.
enum Shape {
    Polygon(Polygon),
    Point(Vec<Vec2>),
    Line(Vec<Vec2>),
}

/// One collidable object: the index of the source object plus its shape.
struct CollisionObject {
    object_id: usize,
    shape: Shape,
}

/// Distance from `p` to segment `a`-`b`.
fn segment_distance(p: Vec2, a: Vec2, b: Vec2) -> f32 {
    let ab = b - a;
    let len2 = ab.length_squared();
    let t = if len2 > 0.0 {
        ((p - a).dot(ab) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    (p - (a + ab * t)).length()
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

    /// `radius` (tile-local units) is the pick tolerance for area-less points and
    /// lines; polygons ignore it and test true containment.
    pub fn get_hovered_objects(
        &self,
        cursor_point: &Vec2,
        radius: f32,
        hovered_objects: &mut Vec<usize>,
    ) {
        let cursor = *cursor_point;
        // Broad phase: any leaf whose aabb comes within the pick radius of the cursor.
        let query = Aabb::from_points([
            cursor - Vec2::new(radius, radius),
            cursor + Vec2::new(radius, radius),
        ]);
        for leaf in self.bvh.leaves(|node| node.aabb().intersects(&query)) {
            let object = &self.objects[leaf as usize];
            let hit = match &object.shape {
                Shape::Polygon(polygon) => polygon.contains(cursor),
                Shape::Point(points) => {
                    points.iter().any(|p| (*p - cursor).length() <= radius)
                }
                Shape::Line(points) => points
                    .windows(2)
                    .any(|w| segment_distance(cursor, w[0], w[1]) <= radius),
            };
            if hit {
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

#[cfg(test)]
mod tests {
    use super::{Vec2, segment_distance};

    #[test]
    fn segment_distance_cases() {
        let a = Vec2::new(0.0, 0.0);
        let b = Vec2::new(10.0, 0.0);
        // On the segment.
        assert_eq!(segment_distance(Vec2::new(5.0, 0.0), a, b), 0.0);
        // Perpendicular off the middle.
        assert_eq!(segment_distance(Vec2::new(5.0, 3.0), a, b), 3.0);
        // Past an endpoint clamps to the endpoint distance, not the infinite line.
        assert_eq!(segment_distance(Vec2::new(-4.0, 0.0), a, b), 4.0);
        // Degenerate segment (a == b) is the distance to the point.
        assert_eq!(segment_distance(Vec2::new(3.0, 4.0), a, a), 5.0);
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
                    let (aabb, shape) = match object.geometry() {
                        Geometry::Polygon(polygon) => {
                            if polygon.is_empty() {
                                continue;
                            }
                            (polygon.aabb(), Shape::Polygon(polygon.clone()))
                        }
                        Geometry::Point(points) => {
                            if points.is_empty() {
                                continue;
                            }
                            (
                                Aabb::from_points(points.iter().copied()),
                                Shape::Point(points.clone()),
                            )
                        }
                        Geometry::Line(points) => {
                            if points.len() < 2 {
                                continue;
                            }
                            (
                                Aabb::from_points(points.iter().copied()),
                                Shape::Line(points.clone()),
                            )
                        }
                    };
                    aabbs.push(aabb);
                    collision_objects.push(CollisionObject { object_id, shape });
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
