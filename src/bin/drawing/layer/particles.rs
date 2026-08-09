use osm::drawing::as_byte_slice;
use osm::wind::grid::WindGrid;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

/// Mercator wind texture resolution (u/v per texel).
const TEX_W: u32 = 1024;
const TEX_H: u32 = 1024;

/// Number of advected particles.
const PARTICLES: u32 = 6000;

/// One particle: current + previous world position, age, and a per-particle seed.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
struct Particle {
    pos: [f32; 2],
    prev: [f32; 2],
    age: f32,
    seed: f32,
    _pad: [f32; 2],
}

/// GPU resources for the particle flow. Passes are added in later tasks.
#[allow(dead_code)]
pub struct ParticleSystem {
    wind_tex: Texture,
    wind_view: TextureView,
    sampler: Sampler,
    particles: Buffer,
    pub count: u32,
}

impl ParticleSystem {
    pub fn new(device: &Device) -> Self {
        let wind_tex = device.create_texture(&TextureDescriptor {
            label: Some("wind uv texture"),
            size: Extent3d { width: TEX_W, height: TEX_H, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rg32Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let wind_view = wind_tex.create_view(&TextureViewDescriptor::default());
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("wind sampler"),
            address_mode_u: AddressMode::Repeat,
            address_mode_v: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // seed particles at hashed-random world positions so they start spread out.
        let mut data = Vec::with_capacity(PARTICLES as usize);
        for i in 0..PARTICLES {
            let x = hash01(i * 2 + 1);
            let y = hash01(i * 2 + 2);
            data.push(Particle {
                pos: [x, y],
                prev: [x, y],
                age: hash01(i * 2 + 3) * 100.0,
                seed: i as f32,
                _pad: [0.0, 0.0],
            });
        }
        let particles = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("particles"),
            contents: as_byte_slice(&data),
            usage: BufferUsages::STORAGE | BufferUsages::VERTEX | BufferUsages::COPY_DST,
        });

        Self { wind_tex, wind_view, sampler, particles, count: PARTICLES }
    }

    /// Resample the grid into the mercator wind texture.
    #[allow(dead_code)]
    pub fn upload_wind(&self, queue: &Queue, grid: &WindGrid) {
        let field = grid.resample_mercator(TEX_W as usize, TEX_H as usize);
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.wind_tex,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            as_byte_slice(&field),
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(TEX_W * 8), // rg32float = 8 bytes/texel
                rows_per_image: Some(TEX_H),
            },
            Extent3d { width: TEX_W, height: TEX_H, depth_or_array_layers: 1 },
        );
    }
}

/// Deterministic 0..1 hash of an integer (for seeding — no rng dependency).
fn hash01(n: u32) -> f32 {
    let mut x = n.wrapping_mul(747796405).wrapping_add(2891336453);
    x = (x >> ((x >> 28).wrapping_add(4))) ^ x;
    x = x.wrapping_mul(277803737);
    x = (x >> 22) ^ x;
    (x & 0xffffff) as f32 / 0xffffff as f32
}
