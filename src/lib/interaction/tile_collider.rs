use ncollide2d::{
    world::{
        CollisionWorld,
    },
    pipeline::object::{
        CollisionGroups,
        GeometricQueryType,
    },
    math::{
        Isometry,
        Point,
        Vector,
    },
    query::{
        Ray,
    },
    shape::{
        ShapeHandle,
        Polyline,
        Segment,
    },
};
pub use crate::*;

pub struct TileCollider {
    world: CollisionWorld<f32, usize>,
}

impl TileCollider {
    pub fn new() -> Self  {
        Self {
            world: CollisionWorld::new(0.02),
        }
    }

    pub fn add_object(&mut self, id: usize, object: &Object) {
        let polygon = Polyline::new(
            object.points
                .iter()
                .map(|p| Point::new(p.x, p.y))
                .collect::<Vec<Point<f32>>>(),
            None
        );

        self.world.add(
            Isometry::identity(),
            ShapeHandle::new(polygon),
            CollisionGroups::new(),
            GeometricQueryType::Contacts(0.02, 0.02),
            id
        );
    }

    pub fn update(&mut self) {
        self.world.update()
    }

    pub fn get_hovered_objects(&self, point: &Point<f32>) -> Vec<usize> {
        let mut object_ids = vec![];
        let mut interferences = vec![];
        self.world.broad_phase
            .interferences_with_point(
                point,
                &mut interferences
            );

        let ray = Ray::new(point.clone(), Vector::x());
        for handle in interferences {
            if let Some(co) = self.world.collision_object(*handle) {
                if let Some(polyline) = co.shape().downcast_ref::<Polyline<f32>>() {
                    let mut winding_number = 0;
                    let points = polyline.points();
                    for edge in polyline.edges() {
                        let segment = Segment::new(points[edge.indices.x], points[edge.indices.y]);
                        use ncollide2d::query::RayCast;
                        if segment.intersects_ray(&Isometry::identity(), &ray) {
                            winding_number += 1;
                        }
                    }

                    if winding_number % 2 == 1 {
                        // We found a general polygon that contains our mouse pointer.
                        object_ids.push(*co.data());
                    }
                }
            }
        }

        object_ids
    }
}