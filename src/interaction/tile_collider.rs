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
        Rotation,
        Translation
    },
    shape::{
        ShapeHandle,
        Polyline,
    },
};
use crate::vector_tile::object::Object;

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
        self.world
            .interferences_with_point(
                point,
                &CollisionGroups::new()
            )
            .map(|co| *co.1.data())
            .collect()
    }
}