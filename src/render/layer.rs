use lyon::tessellation::geometry_builder::{
    VertexBuffers,
};

use crate::render::Vertex;
use lyon::math::Point;
use crate::vector_tile::transform::Layer;

#[derive(Debug)]
pub struct RenderLayer {
    pub layer: Layer,
    gpu_data: (glium::VertexBuffer<Vertex>, glium::IndexBuffer<u16>),
}

impl RenderLayer {
    pub fn new(layer: Layer, display: &glium::Display) -> Self {
        let gpu_data = Self::load(&layer, display);
        Self {
            layer,
            gpu_data,
        }
    }

    fn load(layer: &Layer, display: &glium::Display) -> (glium::VertexBuffer<Vertex>, glium::IndexBuffer<u16>) {
        println!("Loading layer {}.", layer.name);
        let vertex_buffer = glium::VertexBuffer::new(display, &layer.mesh.vertices).unwrap();
        let indices = glium::IndexBuffer::new(
            display,
            glium::index::PrimitiveType::TrianglesList,
            &layer.mesh.indices,
        ).unwrap();
        (vertex_buffer, indices)
    }

    pub fn draw(&self, target: &mut impl glium::Surface, program: &glium::Program, pan: Point) {
        target.draw(
            &self.gpu_data.0,
            &self.gpu_data.1,
            &program,
            &uniform! {
                layer_color: self.layer.color.clone(),
                pan: (pan.x, pan.y),
            },
            &Default::default(),
        ).unwrap();
    }
}