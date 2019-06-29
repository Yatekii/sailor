use crate::css::RulesCache;
use crate::vector_tile::math::Screen;
use crate::vector_tile::math::TileId;
use crate::drawing::{
    drawable_layer::DrawableLayer,
    painter::Painter,
};
use wgpu::{
    RenderPass,
    Buffer,
    BindGroup,
    BindGroupLayout,
    Device,
    CommandEncoder,
};

use crate::vector_tile::tile::Tile;

pub struct DrawableTile {
    pub tile_id: TileId,
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
    pub bind_group: BindGroup,
    pub layers: Vec<DrawableLayer>
}

impl DrawableTile {
    pub fn load_from_tile_id(
        device: &Device,
        encoder: &mut CommandEncoder,
        bind_group_layout: &BindGroupLayout,
        tile_id: TileId,
        tile: &Tile,
        zoom: f32,
        screen: &Screen,
        css_cache: &mut RulesCache
    ) -> DrawableTile {
        let mut layers = vec![];
        for l in &tile.layers {
            layers.push(DrawableLayer::from_layer(l, zoom, css_cache))
        }

        let bind_group = Painter::create_bind_group(
            &device,
            encoder,
            bind_group_layout,
            &screen,
            zoom,
            &layers
        );

        DrawableTile {
            vertex_buffer: device
                .create_buffer_mapped(tile.mesh.vertices.len(), wgpu::BufferUsage::VERTEX)
                .fill_from_slice(&tile.mesh.vertices),
            index_buffer: device
                .create_buffer_mapped(tile.mesh.indices.len(), wgpu::BufferUsage::INDEX)
                .fill_from_slice(&tile.mesh.indices),
            index_count: tile.mesh.indices.len() as u32,
            bind_group: bind_group,
            layers: layers,
            tile_id,
        }
    }

    pub fn paint(&mut self, render_pass: &mut RenderPass) {
        render_pass.set_index_buffer(&self.index_buffer, 0);
        render_pass.set_vertex_buffers(&[(&self.vertex_buffer, 0)]);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw_indexed(0 .. self.index_count, 0, 0 .. 1);
    }
}