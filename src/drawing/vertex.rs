use crate::vector_tile::math::TileId;
use lyon::tessellation;
use lyon::tessellation::geometry_builder::VertexConstructor;
use crate::vector_tile::math;

#[derive(Copy, Clone, Debug)]
#[repr(C,packed)]
pub struct Vertex<> {
    pub position: [i16; 2],
    pub normal: [f32; 2],
    pub layer_id: u32,
}

// A very simple vertex constructor that only outputs the vertex position
pub struct LayerVertexCtor {
    pub tile_id: math::TileId,
    pub layer_id: u32,
    pub stroke: u32,
}

impl LayerVertexCtor {
    pub fn new(tile_id: &TileId) -> Self {
        Self {
            tile_id: tile_id.clone(),
            layer_id: 0,
            stroke: 0,
        }
    }
}

impl VertexConstructor<tessellation::FillVertex, Vertex> for LayerVertexCtor {
    fn new_vertex(&mut self, vertex: tessellation::FillVertex) -> Vertex {
        assert!(!vertex.position.x.is_nan());
        assert!(!vertex.position.y.is_nan());
        // println!("{:?}", vertex.position);
        if vertex.normal.length() > 300.0 {
            println!("KEKEKKE");
        }
        Vertex {
            // position: math::tile_to_global_space(&self.tile_id, vertex.position).to_array(),
            position: [vertex.position.x as i16, vertex.position.y as i16],
            normal: vertex.normal.to_array(),
            layer_id: self.layer_id,
        }
    }
}

impl VertexConstructor<tessellation::StrokeVertex, Vertex> for LayerVertexCtor {
    fn new_vertex(&mut self, vertex: tessellation::StrokeVertex) -> Vertex {
        assert!(!vertex.position.x.is_nan());
        assert!(!vertex.position.y.is_nan());
        // println!("{:?}", vertex.position);
        let pos = math::tile_to_global_space(&self.tile_id, vertex.position);
        Vertex {
            position: [pos.x as i16, pos.y as i16],
            normal: vertex.normal.to_array(),
            layer_id: self.layer_id,
        }
    }
}