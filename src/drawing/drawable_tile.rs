use wgpu::{
    BindGroup,
    RenderPass,
    Buffer,
};

pub struct DrawableTile {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
    pub index_count: u32,
    pub bind_group: BindGroup,
}

impl DrawableTile {
    pub fn paint(&mut self, redner_pass: &mut RenderPass) {
        redner_pass.set_bind_group(0, &self.bind_group, &[]);
        redner_pass.set_index_buffer(&self.index_buffer, 0);
        redner_pass.set_vertex_buffers(&[(&self.vertex_buffer, 0)]);
        redner_pass.draw_indexed(0 .. self.index_count, 0, 0 .. 1);
    }
}