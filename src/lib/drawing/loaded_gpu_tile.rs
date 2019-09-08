use wgpu::*;
use crate::*;

pub struct LoadedGPUTile {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
}

impl LoadedGPUTile {
    pub fn load(
        device: &Device,
        tile: &Tile,
    ) -> Self {

        Self {
            vertex_buffer: device
                .create_buffer_mapped(tile.mesh().vertices.len(), wgpu::BufferUsage::VERTEX)
                .fill_from_slice(&tile.mesh().vertices),
            index_buffer: device
                .create_buffer_mapped(tile.mesh().indices.len(), wgpu::BufferUsage::INDEX)
                .fill_from_slice(&tile.mesh().indices),
        }
    }
}