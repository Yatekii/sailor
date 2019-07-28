use core::ops::Range;
use crate::drawing::layer_collection::LayerCollection;
use crate::vector_tile::math::TileId;
use crate::drawing::{
    drawable_layer::DrawableLayer,
};
use wgpu::{
    RenderPass,
    Buffer,
    Device,
};

use crate::vector_tile::tile::Tile;

pub struct DrawableTile {
    pub tile_id: TileId,
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
    pub features: Vec<(u32, Range<u32>)>,
    pub extent: u16,
}

impl DrawableTile {
    pub fn load_from_tile_id(
        device: &Device,
        tile_id: TileId,
        tile: &Tile,
    ) -> DrawableTile {
        let mut features = Vec::new();
        for l in tile.layers.clone() {
            features.extend(l.features);
        }

        DrawableTile {
            vertex_buffer: device
                .create_buffer_mapped(tile.mesh.vertices.len(), wgpu::BufferUsage::VERTEX)
                .fill_from_slice(&tile.mesh.vertices),
            index_buffer: device
                .create_buffer_mapped(tile.mesh.indices.len(), wgpu::BufferUsage::INDEX)
                .fill_from_slice(&tile.mesh.indices),
            index_count: tile.mesh.indices.len() as u32,
            features,
            tile_id,
            extent: tile.extent,
        }
    }

    pub fn paint(
        &mut self,
        render_pass: &mut RenderPass,
        layer_collection: &LayerCollection,
        tile_id: u32,
        feature_id: u32,
        outline: bool
    ) {
        render_pass.set_index_buffer(&self.index_buffer, 0);
        render_pass.set_vertex_buffers(&[(&self.vertex_buffer, 0)]);
        for (id, range) in &self.features {
            if feature_id == *id && layer_collection.is_visible(*id) {
                if outline {
                    if layer_collection.has_outline(*id) {
                        let range_start = tile_id << 1;
                        render_pass.draw_indexed(range.clone(), 0, 0 + range_start .. 1 + range_start);
                    }
                } else {
                    let range_start = (tile_id << 1) | 1;
                    render_pass.draw_indexed(range.clone(), 0, 0 + range_start .. 1 + range_start);
                }
            }
        }
    }
}