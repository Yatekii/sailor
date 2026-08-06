use crate::math::TileId;

use super::css::Selector;

use lyon::math::Point;
use std::collections::HashMap;

/// Classifies an object as one of three possible types.
#[derive(Debug, Clone)]
pub enum ObjectType {
    Polygon,
    Line,
    Point,
}

/// Represents any object on the map.
#[derive(Debug, Clone)]
pub struct Object {
    /// The CSS selector that fully describes the object.
    selector: Selector,
    /// All the points that belong to the object.
    /// If this is a polygon, the points describe the outline in order.
    /// If this is a line, the points describe the line in order.
    /// For a point there is only one point contained.
    points: Vec<Point>,
    /// All the OSM tags that are attached to this object.
    tags: HashMap<String, String>,
    /// The object type.
    _object_type: ObjectType,
    pub title: Option<String>,
    pub id: u32,
    pub tile_id: TileId,
}

impl Object {
    /// Creates a new object with no tags.
    pub fn new(
        selector: Selector,
        points: Vec<Point>,
        object_type: ObjectType,
        tile_id: TileId,
        id: u32,
        title: Option<String>,
    ) -> Self {
        Self {
            selector,
            points,
            tags: HashMap::new(),
            _object_type: object_type,
            title,
            tile_id,
            id,
        }
    }

    /// Creates a new object with an initial set of tags.
    pub fn new_with_tags(
        selector: Selector,
        points: Vec<Point>,
        tags: HashMap<String, String>,
        object_type: ObjectType,
        tile_id: TileId,
        id: u32,
        title: Option<String>,
    ) -> Self {
        Self {
            selector,
            points,
            tags,
            _object_type: object_type,
            title,
            tile_id,
            id,
        }
    }

    /// Returns the set of points contained in the object.
    pub fn points(&self) -> &Vec<Point> {
        &self.points
    }

    /// Returns the set of tags contained in the object.
    pub fn tags(&self) -> &HashMap<String, String> {
        &self.tags
    }

    /// Returns the selector describing the object.
    pub fn selector(&self) -> &Selector {
        &self.selector
    }

    /// Returns the estimated memory size used by the object.
    pub fn size(&self) -> usize {
        use deepsize::DeepSizeOf;
        self.selector.size()
            + self.tags.deep_size_of()
            + self.points.capacity() * std::mem::size_of::<Point>()
            + std::mem::size_of::<ObjectType>()
    }
}
