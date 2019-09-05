use lyon::math::Point;
use std::collections::HashMap;
use super::*;

#[derive(Debug, Clone)]
pub enum ObjectType {
    Polygon,
    Line,
    Point,
}

#[derive(Debug, Clone)]
pub struct Object {
    selector: Selector,
    points: Vec<Point>,
    tags: HashMap<String, String>,
    object_type: ObjectType,
}

impl Object {
    pub fn new(
        selector: Selector,
        points: Vec<Point>,
        object_type: ObjectType
    ) -> Self {
        Self {
            selector,
            points,
            tags: HashMap::new(),
            object_type
        }
    }

    pub fn new_with_tags(
        selector: Selector,
        points: Vec<Point>,
        tags: HashMap<String, String>,
        object_type: ObjectType
    ) -> Self {
        Self {
            selector,
            points,
            tags,
            object_type
        }
    }

    pub fn points(&self) -> &Vec<Point> {
        &self.points
    }

    pub fn tags(&self) -> &HashMap<String, String> {
        &self.tags
    }

    pub fn selector(&self) -> &Selector {
        &self.selector
    }

    pub fn size(&self) -> usize {
        use parity_util_mem::MallocSizeOfExt;
        self.selector.size()
      + self.tags.malloc_size_of() + std::mem::size_of_val(&self.tags)
      + self.tags.iter().map(|(k, v)| k.len() + v.len()).sum::<usize>()
      + self.points.capacity() * std::mem::size_of::<Point>() + 8
      + std::mem::size_of::<ObjectType>()
    }
}