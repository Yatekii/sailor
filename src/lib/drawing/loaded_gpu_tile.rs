use crate::*;
use wgpu::util::DeviceExt;
use wgpu::*;

pub struct LoadedGPUTile {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
}

impl LoadedGPUTile {
    pub fn load(device: &Device, tile: &Tile) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            // size: tile.mesh().vertices.len() as u64 * 12,
            contents: as_byte_slice(&tile.mesh().vertices),
            usage: wgpu::BufferUsage::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            // size: tile.mesh().indices.len() as u64 * 4,
            contents: as_byte_slice(&tile.mesh().indices),
            usage: wgpu::BufferUsage::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
        }
    }
}
