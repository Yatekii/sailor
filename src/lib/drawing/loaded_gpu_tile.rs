use wgpu::{util::DeviceExt, Buffer, Device};

use crate::vector_tile::tile::Tile;

use super::as_byte_slice;

pub struct LoadedGPUTile {
    pub vertex_buffer: Buffer,
    pub index_buffer: Buffer,
}

impl LoadedGPUTile {
    pub fn load(device: &Device, tile: &Tile) -> Self {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("tile {} vertex buffer", tile.tile_id())),
            // size: tile.mesh().vertices.len() as u64 * 12,
            contents: as_byte_slice(&tile.mesh().vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(&format!("tile {} index buffer", tile.tile_id())),
            // size: tile.mesh().indices.len() as u64 * 4,
            contents: as_byte_slice(&tile.mesh().indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            vertex_buffer,
            index_buffer,
        }
    }
}
