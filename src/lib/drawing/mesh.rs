use crate::drawing::vertex::VertexType;
use crate::drawing::vertex::{LayerVertexCtor, Vertex};
use lyon::lyon_tessellation::{
    FillGeometryBuilder, FillVertexConstructor, StrokeGeometryBuilder, StrokeVertexConstructor,
    VertexId,
};
use lyon::math::{Point, Vector};
use lyon::tessellation::{
    geometry_builder::GeometryBuilderError, FillVertex, GeometryBuilder, StrokeVertex,
    VertexBuffers,
};

pub struct MeshBuilder<'l> {
    pub buffers: &'l mut VertexBuffers<Vertex, u32>,
    vertex_offset: u32,
    index_offset: u32,
    vertex_constructor: LayerVertexCtor,
}

impl<'l> MeshBuilder<'l> {
    pub fn new(
        buffers: &'l mut VertexBuffers<Vertex, u32>,
        vertex_constructor: LayerVertexCtor,
    ) -> Self {
        let vertex_offset = buffers.vertices.len() as u32;
        let index_offset = buffers.indices.len() as u32;
        Self {
            buffers,
            vertex_offset,
            index_offset,
            vertex_constructor,
        }
    }

    pub fn set_current_feature_id(&mut self, feature_id: u32) {
        self.vertex_constructor.feature_id = feature_id;
    }

    pub fn set_current_extent(&mut self, extent: f32) {
        self.vertex_constructor.extent = extent;
    }

    pub fn get_current_index(&mut self) -> u32 {
        self.buffers.indices.len() as u32
    }

    pub fn set_current_vertex_type(&mut self, vertex_type: VertexType) {
        self.vertex_constructor.vertex_type = vertex_type;
    }

    pub fn add_vertex(
        &mut self,
        vertex: Point,
        normal: Vector,
    ) -> Result<VertexId, GeometryBuilderError> {
        self.buffers
            .vertices
            .push(self.vertex_constructor.new_osm_vertex(vertex, normal));
        let len = self.buffers.vertices.len();
        if len > u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }
        Ok(VertexId((len - 1) as u32 - self.vertex_offset))
    }
}

impl<'l> GeometryBuilder for MeshBuilder<'l> {
    fn begin_geometry(&mut self) {
        self.vertex_offset = self.buffers.vertices.len() as u32;
        self.index_offset = self.buffers.indices.len() as u32;
    }

    fn end_geometry(&mut self) {
        // Count {
        //     vertices: self.buffers.vertices.len() as u32 - self.vertex_offset,
        //     indices: self.buffers.indices.len() as u32 - self.index_offset,
        // }
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

impl<'l> FillGeometryBuilder for MeshBuilder<'l> {
    fn add_fill_vertex(
        &mut self,
        vertex: FillVertex,
    ) -> Result<lyon::lyon_tessellation::VertexId, GeometryBuilderError> {
        self.buffers
            .vertices
            .push(FillVertexConstructor::new_vertex(
                &mut self.vertex_constructor,
                vertex,
            ));
        let len = self.buffers.vertices.len();
        if len > u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }
        Ok(VertexId((len - 1) as u32 - self.vertex_offset))
    }
}

impl<'l> StrokeGeometryBuilder for MeshBuilder<'l> {
    fn add_stroke_vertex(
        &mut self,
        vertex: StrokeVertex,
    ) -> Result<lyon::lyon_tessellation::VertexId, GeometryBuilderError> {
        self.buffers
            .vertices
            .push(StrokeVertexConstructor::new_vertex(
                &mut self.vertex_constructor,
                vertex,
            ));
        let len = self.buffers.vertices.len();
        if len > u32::MAX as usize {
            return Err(GeometryBuilderError::TooManyVertices);
        }
        Ok(VertexId((len - 1) as u32 - self.vertex_offset))
    }
}
