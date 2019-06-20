use lyon::tessellation::geometry_builder::{
    VertexBuffers,
};

use crate::render::Vertex;
use lyon::math::Point;

#[derive(Debug)]
pub struct Layer {
    pub name: String,
    pub mesh: VertexBuffers<Vertex, u16>,
    pub color: [f32; 3],
    gpu_data: Option<(glium::VertexBuffer<Vertex>, glium::IndexBuffer<u16>)>,
}

impl Clone for Layer {
    fn clone(&self) -> Layer {
        Layer {
            name: self.name.clone(),
            mesh: self.mesh.clone(),
            color: self.color,
            gpu_data: None,
        }
    }
}

impl Layer {
    pub fn new(name: String, mesh: VertexBuffers<Vertex, u16>, color: [f32; 3]) -> Self {
        Self {
            name,
            mesh,
            color,
            gpu_data: None,
        }
    }

    pub fn load(&mut self, display: &glium::Display) {
        println!("Loading layer {}.", self.name);
        let vertex_buffer = glium::VertexBuffer::new(display, &self.mesh.vertices).unwrap();
        let indices = glium::IndexBuffer::new(
            display,
            glium::index::PrimitiveType::TrianglesList,
            &self.mesh.indices,
        ).unwrap();
        self.gpu_data = Some((vertex_buffer, indices));
    }

    pub fn draw(&self, target: &mut impl glium::Surface, program: &glium::Program, pan: Point) {
        if let Some(gpu_data) = &self.gpu_data {
            target.draw(
                &gpu_data.0,
                &gpu_data.1,
                &program,
                &uniform! {
                    layer_color: self.color.clone(),
                    pan: (pan.x, pan.y),
                },
                &Default::default(),
            ).unwrap();
        }
    }
}