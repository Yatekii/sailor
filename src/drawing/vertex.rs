use lyon::tessellation;
use lyon::tessellation::geometry_builder::VertexConstructor;
use crate::vector_tile::math;

#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: [f32; 2],
}

pub fn vertex(x: f32, y: f32) -> Vertex {
    Vertex { position: [x, y] }
}

implement_vertex!(Vertex, position);

// A very simple vertex constructor that only outputs the vertex position
pub struct LayerVertexCtor {
    pub tile_id: math::TileId,
}

impl VertexConstructor<tessellation::FillVertex, Vertex> for LayerVertexCtor {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex) -> Vertex {
        assert!(!vertex.position.x.is_nan());
        assert!(!vertex.position.y.is_nan());
        // println!("{:?}", vertex.position);
        Vertex {
            position: math::tile_to_global_space(&self.tile_id, vertex.position).to_array(),
        }
    }
}