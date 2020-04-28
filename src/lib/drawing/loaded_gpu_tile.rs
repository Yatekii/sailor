use crate::*;
use wgpu::*;

pub struct LoadedGPUTile {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
}

impl LoadedGPUTile {
    pub fn load(device: &Device, tile: &Tile) -> Self {
        let vertex_buffer = device.create_buffer_mapped(&wgpu::BufferDescriptor {
            label: None,
            size: tile.mesh().vertices.len() as u64 * 12,
            usage: wgpu::BufferUsage::VERTEX,
        });

        vertex_buffer
            .data
            .copy_from_slice(as_byte_slice(&tile.mesh().vertices));

        let index_buffer = device.create_buffer_mapped(&wgpu::BufferDescriptor {
            label: None,
            size: tile.mesh().indices.len() as u64 * 4,
            usage: wgpu::BufferUsage::INDEX,
        });

        index_buffer
            .data
            .copy_from_slice(as_byte_slice(&tile.mesh().indices));

        Self {
            vertex_buffer: vertex_buffer.finish(),
            index_buffer: index_buffer.finish(),
        }
    }
}
