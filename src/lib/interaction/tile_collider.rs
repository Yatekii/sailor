use ncollide2d::{
    math::{Isometry, Point, Vector},
    pipeline::{
        object::{CollisionGroups, GeometricQueryType},
        CollisionObjectSlabHandle,
    },
    query::Ray,
    shape::{Polyline, Segment, ShapeHandle},
    world::CollisionWorld,
};
use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
    thread::spawn,
};

use crate::object::Object;

pub struct TileCollider {
    world: CollisionWorld<f32, usize>,
    objects: HashMap<usize, CollisionObjectSlabHandle>,
}

impl TileCollider {
    pub fn new() -> Self {
        Self {
            world: CollisionWorld::new(0.02),
            objects: HashMap::new(),
        }
    }

    pub fn add_object(&mut self, id: usize, object: &Object) {
        let polygon = Polyline::new(
            object
                .points()
                .iter()
                .map(|p| Point::new(p.x, p.y))
                .collect::<Vec<Point<f32>>>(),
            None,
        );

        if !self.objects.contains_key(&id) {
            self.objects.insert(
                id,
                self.world
                    .add(
                        Isometry::identity(),
                        ShapeHandle::new(polygon),
                        CollisionGroups::new(),
                        GeometricQueryType::Contacts(0.02, 0.02),
                        id,
                    )
                    .0,
            );
        }
    }

    pub fn update(&mut self) {
        self.world.update()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get_hovered_objects(&self, point: &Point<f32>, hovered_objects: &mut Vec<usize>) {
        let mut interferences = Vec::with_capacity(100);
        self.world
            .broad_phase
            .interferences_with_point(point, &mut interferences);

        let ray = Ray::new(*point, Vector::x());
        for handle in interferences {
            if let Some(co) = self.world.collision_object(*handle) {
                if let Some(polyline) = co.shape().downcast_ref::<Polyline<f32>>() {
                    let mut winding_number = 0;
                    let points = polyline.points();
                    for edge in polyline.edges() {
                        let segment = Segment::new(points[edge.indices.x], points[edge.indices.y]);
                        use ncollide2d::query::RayCast;
                        if segment.intersects_ray(&Isometry::identity(), &ray, f32::MAX) {
                            // TODO is toi f32::MAX here correct?
                            winding_number += 1;
                        }
                    }

                    if winding_number % 2 == 1 {
                        // We found a general polygon that contains our mouse pointer.
                        hovered_objects.push(*co.data());
                    }
                }
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
                            if objects[object_id].points().len() >= 2 {
                                collider.add_object(object_id, &objects[object_id]);
                            }
                        }
                        collider.update();
                    }
                    Err(_e) => log::error!(
                        "Could not aquire collider lock. Not loading the objects of this tile."
                    ),
                }
            }
        });
    }
}
