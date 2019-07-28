use crate::vector_tile::transform::Layer;
use core::ops::Range;

#[derive(Debug, Clone)]
pub struct DrawableLayer {
    pub id: u32,
    pub indices_range: Range<u32>,
    pub features: Vec<(u32, Range<u32>)>
}

impl DrawableLayer {
    pub fn from_layer(layer: &Layer) -> Self {
        Self {
            id: layer.id,
            indices_range: layer.indices_range.clone(),
            features: layer.features.clone(),
        }
    }
}