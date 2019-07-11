use crate::vector_tile::math::TileId;
use crate::drawing::{
    drawable_layer::DrawableLayer,
};
use wgpu::{
    RenderPass,
    Buffer,
    Device,
    BindGroup,
};

use crate::vector_tile::tile::Tile;

pub struct DrawableTile {
    pub tile_id: TileId,
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
    pub layers: Vec<DrawableLayer>,
    pub bind_group: BindGroup,
}

impl DrawableTile {
    pub fn load_from_tile_id(
        device: &Device,
        tile_id: TileId,
        tile: &Tile,
        bind_group: BindGroup,
    ) -> DrawableTile {
        let mut layers = Vec::with_capacity(tile.layers.len());
        for l in &tile.layers {
            layers.push(DrawableLayer::from_layer(&l))
        }

        DrawableTile {
            vertex_buffer: device
                .create_buffer_mapped(tile.mesh.vertices.len(), wgpu::BufferUsage::VERTEX)
                .fill_from_slice(&tile.mesh.vertices),
            index_buffer: device
                .create_buffer_mapped(tile.mesh.indices.len(), wgpu::BufferUsage::INDEX)
                .fill_from_slice(&tile.mesh.indices),
            index_count: tile.mesh.indices.len() as u32,
            layers: layers,
            tile_id,
            bind_group,
        }
    }

    pub fn layer_has_data(&self, layer_id: u32) -> bool {
        self.layers
            .iter()
            .find(|dl| dl.id == layer_id)
            .map(|dl| dl.indices_range.end - dl.indices_range.start > 1)
            .unwrap_or(false)
    }

    pub fn update_bind_group(&mut self, bind_group: BindGroup) {
        self.bind_group = bind_group;
    }

    pub fn paint(
        &mut self,
        render_pass: &mut RenderPass,
        layer_id: u32,
        outline: bool
    ) {
        if let Some(layer) = self.layers.iter().find(|l| l.id == layer_id) {
            render_pass.set_index_buffer(&self.index_buffer, 0);
            render_pass.set_vertex_buffers(&[(&self.vertex_buffer, 0)]);
            if outline {
                // render_pass.draw_indexed(layer.indices_range.clone(), 0, 0 .. 1);
            } else {
                render_pass.draw_indexed(layer.indices_range.clone(), 0, 1 .. 2);
            }
        }
    }
}