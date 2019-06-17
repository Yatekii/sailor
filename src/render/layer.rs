use lyon::tessellation::geometry_builder::{
    VertexBuffers,
};

use crate::render::Vertex;

pub struct Layer {
    pub name: String,
    pub id: u32,
    pub mesh: VertexBuffers<Vertex, u16>,
    pub color: [f32; 3],
}