use lyon::tessellation;
use lyon::tessellation::geometry_builder::VertexConstructor;

#[derive(Copy, Clone, Debug)]
pub struct Vertex {
    position: [f32; 2],
}

implement_vertex!(Vertex, position);

// A very simple vertex constructor that only outputs the vertex position
pub struct VertexCtor;
impl VertexConstructor<tessellation::FillVertex, Vertex> for VertexCtor {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex) -> Vertex {
        assert!(!vertex.position.x.is_nan());
        assert!(!vertex.position.y.is_nan());
        // println!("{:?}", vertex.position);
        Vertex {
            // (ugly hack) tweak the vertext position so that the logo fits roughly
            // within the (-1.0, 1.0) range.
            position: vertex.position.to_array(),
        }
    }
}