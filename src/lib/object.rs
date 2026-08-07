use crate::geometry::Geometry;
use crate::math::TileId;

use super::css::Selector;

use std::collections::HashMap;

/// Represents any object on the map.
#[derive(Debug, Clone)]
pub struct Object {
    /// The CSS selector that fully describes the object.
    selector: Selector,
    /// The geometry (polygon / line / point) of the object.
    geometry: Geometry,
    /// All the OSM tags that are attached to this object.
    tags: HashMap<String, String>,
    pub title: Option<String>,
    /// Index of this object within its tile (collider -> object lookup).
    pub id: u32,
    pub tile_id: TileId,
    /// Stable feature id from the source tile (the OSM id). Shared by every part
    /// of a multipolygon and by the same feature across tiles. 0 when absent.
    pub feature_id: u64,
    /// Dense per-tile slot of this object's feature, written into the vertices
    /// and used by the shader to highlight the whole feature. Shared by all parts.
    pub feature_slot: u32,
    /// Which part of the feature this object is (0 for the first / only part).
    pub part: u32,
}

impl Object {
    /// Creates a new object with no tags.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        selector: Selector,
        geometry: Geometry,
        tile_id: TileId,
        id: u32,
        title: Option<String>,
        feature_id: u64,
        feature_slot: u32,
        part: u32,
    ) -> Self {
        Self {
            selector,
            geometry,
            tags: HashMap::new(),
            title,
            tile_id,
            id,
            feature_id,
            feature_slot,
            part,
        }
    }

    /// Creates a new object with an initial set of tags.
    #[allow(clippy::too_many_arguments)]
    pub fn new_with_tags(
        selector: Selector,
        geometry: Geometry,
        tags: HashMap<String, String>,
        tile_id: TileId,
        id: u32,
        title: Option<String>,
        feature_id: u64,
        feature_slot: u32,
        part: u32,
    ) -> Self {
        Self {
            selector,
            geometry,
            tags,
            title,
            tile_id,
            id,
            feature_id,
            feature_slot,
            part,
        }
    }

    /// Returns the geometry of the object.
    pub fn geometry(&self) -> &Geometry {
        &self.geometry
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
            + self.geometry.point_count() * std::mem::size_of::<parry2d::math::Vec2>()
    }
}
