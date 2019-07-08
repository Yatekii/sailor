use crate::drawing::vertex::{
    Vertex,
    LayerVertexCtor
};
use lyon::{
    path::{
        Index,
        VertexId,
    },
    tessellation::{
        VertexBuffers,
        GeometryBuilder,
        FillVertex,
        StrokeVertex,
        VertexConstructor,
        geometry_builder::{
            GeometryBuilderError,
            MaxIndex,
            Count,
        },
    },
};

pub struct MeshBuilder<'l> {
    pub buffers: &'l mut VertexBuffers<Vertex, u32>,
    vertex_offset: Index,
    index_offset: Index,
    vertex_constructor: LayerVertexCtor,
}

impl<'l> MeshBuilder<'l> {
    pub fn new(buffers: &'l mut VertexBuffers<Vertex, u32>, vertex_constructor: LayerVertexCtor) -> Self {
        let vertex_offset = buffers.vertices.len() as Index;
        let index_offset = buffers.indices.len() as Index;
        Self {
            buffers,
            vertex_offset,
            index_offset,
            vertex_constructor,
        }
    }

    pub fn set_current_feature_id(&mut self, layer_id: u32) {
        // dbg!(&layer_id);
        self.vertex_constructor.layer_id = layer_id;
    }

    pub fn set_current_vertex_type(&mut self, stroke: bool) {
        self.vertex_constructor.stroke = if stroke { 1 } else { 0 };
    }

    pub fn get_current_index(&mut self) -> u32 {
        self.buffers.indices.len() as u32
    }
}

impl<'l> GeometryBuilder<FillVertex>
    for MeshBuilder<'l>
{
    fn begin_geometry(&mut self) {
        self.vertex_offset = self.buffers.vertices.len() as Index;
        self.index_offset = self.buffers.indices.len() as Index;
    }

    fn end_geometry(&mut self) -> Count {
        Count {
            vertices: self.buffers.vertices.len() as u32 - self.vertex_offset,
            indices: self.buffers.indices.len() as u32 - self.index_offset,
        }
    }

    fn add_vertex(&mut self, v: FillVertex) -> Result<VertexId, GeometryBuilderError> {
        self.buffers.vertices.push(self.vertex_constructor.new_vertex(v));
        let len = self.buffers.vertices.len();
        if len > u32::max_index() {
            return Err(GeometryBuilderError::TooManyVertices);
        }
        Ok(VertexId((len - 1) as Index - self.vertex_offset))
    }

    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        self.buffers.indices.push((a + self.vertex_offset).into());
        self.buffers.indices.push((b + self.vertex_offset).into());
        self.buffers.indices.push((c + self.vertex_offset).into());
    }

    fn abort_geometry(&mut self) {
        self.buffers.vertices.truncate(self.vertex_offset as usize);
        self.buffers.indices.truncate(self.index_offset as usize);
    }
}

impl<'l> GeometryBuilder<StrokeVertex>
    for MeshBuilder<'l>
{
    fn begin_geometry(&mut self) {
        self.vertex_offset = self.buffers.vertices.len() as Index;
        self.index_offset = self.buffers.indices.len() as Index;
    }

    fn end_geometry(&mut self) -> Count {
        Count {
            vertices: self.buffers.vertices.len() as u32 - self.vertex_offset,
            indices: self.buffers.indices.len() as u32 - self.index_offset,
        }
    }

    fn add_vertex(&mut self, v: StrokeVertex) -> Result<VertexId, GeometryBuilderError> {
        self.buffers.vertices.push(self.vertex_constructor.new_vertex(v));
        let len = self.buffers.vertices.len();
        if len > u32::max_index() {
            return Err(GeometryBuilderError::TooManyVertices);
        }
        Ok(VertexId((len - 1) as Index - self.vertex_offset))
    }

    fn add_triangle(&mut self, a: VertexId, b: VertexId, c: VertexId) {
        self.buffers.indices.push((a + self.vertex_offset).into());
        self.buffers.indices.push((b + self.vertex_offset).into());
        self.buffers.indices.push((c + self.vertex_offset).into());
    }

    fn abort_geometry(&mut self) {
        self.buffers.vertices.truncate(self.vertex_offset as usize);
        self.buffers.indices.truncate(self.index_offset as usize);
    }
}