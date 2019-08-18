use lyon::math::Point;
use std::collections::HashMap;
use crate::css::Selector;

#[derive(Debug, Clone)]
pub enum ObjectType {
    Polygon,
    Line,
    Point,
}

#[derive(Debug, Clone)]
pub struct Object {
    pub selector: Selector,
    pub tags: HashMap<String, String>,
    pub points: Vec<Point>,
    pub object_type: ObjectType,
}

impl Object {
    pub fn new(
        selector: Selector,
        tags: HashMap<String, String>,
        points: Vec<Point>,
        object_type: ObjectType
    ) -> Self {
        Self {
            selector,
            tags,
            points,
            object_type
        }
    }
}