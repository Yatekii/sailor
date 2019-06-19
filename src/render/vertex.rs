use lyon::tessellation;
use lyon::tessellation::geometry_builder::VertexConstructor;

#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    pub position: [f32; 2],
}

implement_vertex!(Vertex, position);

// A very simple vertex constructor that only outputs the vertex position
pub struct LayerVertexCtor;

impl VertexConstructor<tessellation::FillVertex, Vertex> for LayerVertexCtor {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex) -> Vertex {
        assert!(!vertex.position.x.is_nan());
        assert!(!vertex.position.y.is_nan());
        // println!("{:?}", vertex.position);
        Vertex {
            position: vertex.position.to_array(),
        }
    }
}