use crate::drawing::drawable_layer::DrawableLayer;
use wgpu::{
    RenderPass,
    Buffer,
    BindGroup,
};

pub struct DrawableTile {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
    pub bind_group: BindGroup,
    pub layers: Vec<DrawableLayer>
}

impl DrawableTile {
    pub fn paint(&mut self, render_pass: &mut RenderPass) {
        render_pass.set_index_buffer(&self.index_buffer, 0);
        render_pass.set_vertex_buffers(&[(&self.vertex_buffer, 0)]);
        render_pass.set_bind_group(0, &self.bind_group, &[]);
        render_pass.draw_indexed(0 .. self.index_count, 0, 0 .. 1);
    }
}