use wgpu::RenderPipeline;
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
        blend_pipeline: &RenderPipeline,
        noblend_pipeline: &RenderPipeline,
        layer_collection: &LayerCollection,
        tile_id: u32,
        outline: bool
    ) {
        render_pass.set_index_buffer(&self.index_buffer, 0);
        render_pass.set_vertex_buffers(&[(&self.vertex_buffer, 0)]);

        let mut alpha_set = vec![];
        let mut opaque_set = vec![];

        self.features.sort_by(|a, b| {
            layer_collection
                .get_zindex(a.0)
                .partial_cmp(&layer_collection.get_zindex(b.0)).unwrap()
        });

        for (id, range) in &self.features {
            if layer_collection.has_alpha(*id) {
                alpha_set.push((id, range));
            } else {
                opaque_set.push((id, range));
            }
        }

        let mut i = 0;
        render_pass.set_pipeline(noblend_pipeline);
        for (id, range) in opaque_set {
            if range.len() > 0 && layer_collection.is_visible(*id) {
                render_pass.set_stencil_reference(i as u32);
                i += 1;

                let range_start = (tile_id << 1) | 1;
                render_pass.draw_indexed(range.clone(), 0, 0 + range_start .. 1 + range_start);

                if layer_collection.has_outline(*id) {
                    let range_start = tile_id << 1;
                    render_pass.draw_indexed(range.clone(), 0, 0 + range_start .. 1 + range_start);
                }
            }
        }

        let mut i = 0;
        render_pass.set_pipeline(blend_pipeline);
        for (id, range) in alpha_set {
            if range.len() > 0 && layer_collection.is_visible(*id) {
                render_pass.set_stencil_reference(i as u32);
                i += 1;

                let range_start = (tile_id << 1) | 1;
                render_pass.draw_indexed(range.clone(), 0, 0 + range_start .. 1 + range_start);

                if layer_collection.has_outline(*id) {
                    let range_start = tile_id << 1;
                    render_pass.draw_indexed(range.clone(), 0, 0 + range_start .. 1 + range_start);
                }
            }
        }
    }
}