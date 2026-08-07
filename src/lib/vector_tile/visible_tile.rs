use std::sync::{Arc, RwLock};

use wgpu_glyph::{GlyphBrush, Section, Text};

use crate::vector_tile::tile::Tile;
use crate::*;

use self::drawing::loaded_gpu_tile::LoadedGPUTile;
use self::feature::collection::FeatureCollection;
use self::interaction::tile_collider::{TileCollider, TileColliderLoader};
use self::math::{Camera, TileId};
use self::object::Object;

pub struct VisibleTile<'t> {
    tile: &'t Tile,
    gpu_tile: Option<LoadedGPUTile>,
    tile_collider: Arc<RwLock<TileCollider>>,
}

impl<'t> VisibleTile<'t> {
    pub fn new(tile: &'t Tile) -> Self {
        Self {
            tile,
            gpu_tile: None,
            tile_collider: Arc::new(RwLock::new(TileCollider::new())),
        }
    }

    pub fn tile_id(&self) -> TileId {
        self.tile.tile_id()
    }

    pub fn extent(&self) -> u16 {
        self.tile.extent()
    }

    pub fn objects(&self) -> Arc<RwLock<Vec<Object>>> {
        self.tile.objects()
    }

    pub fn load_to_gpu(&mut self, device: &wgpu::Device) {
        self.gpu_tile = Some(LoadedGPUTile::load(device, self.tile));
    }

    pub fn unload_from_gpu(&mut self) {
        self.gpu_tile = None;
    }

    pub fn is_loaded_to_gpu(&self) -> bool {
        self.gpu_tile.is_some()
    }

    pub fn load_collider(&mut self) {
        self.tile_collider.load(self.objects());
    }

    pub fn collider(&self) -> Arc<RwLock<TileCollider>> {
        self.tile_collider.clone()
    }

    pub fn gpu_tile(&self) -> &Option<LoadedGPUTile> {
        &self.gpu_tile
    }

    pub fn paint<'a, 'b>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'b>,
        blend_pipeline: &'b wgpu::RenderPipeline,
        data: Option<&'b LoadedGPUTile>,
        feature_collection: &'a FeatureCollection,
        tile_id: u32,
    ) {
        if let Some(data) = data {
            render_pass.set_index_buffer(data.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            render_pass.set_vertex_buffer(0, data.vertex_buffer.slice(..));

            let features = {
                let mut features = self.tile.features().clone();
                features.sort_by(|a, b| {
                    feature_collection
                        .get_zindex(a.0)
                        .partial_cmp(&feature_collection.get_zindex(b.0))
                        .unwrap()
                });
                features
            };

            let mut i = 0;
            render_pass.set_pipeline(blend_pipeline);
            for (id, range) in &features {
                if !range.is_empty() && feature_collection.is_visible(*id) {
                    render_pass.set_stencil_reference(i as u32);
                    i += 1;

                    let range_start = (tile_id << 1) | 1;
                    render_pass.draw_indexed(range.clone(), 0, range_start..1 + range_start);

                    if feature_collection.has_outline(*id) {
                        let range_start = tile_id << 1;
                        render_pass.draw_indexed(range.clone(), 0, range_start..1 + range_start);
                    }
                }
            }
        }
    }

    pub fn queue_text(&self, glyph_brush: &mut GlyphBrush<()>, screen: &Camera, z: f32) {
        let matrix = screen.tile_to_global_space(z, &self.tile.tile_id());
        for text in self.tile.text() {
            let position = matrix * glm::vec4((text.0).0, (text.0).1, 0.0, 1.0);
            let section = Section::default()
                .add_text(Text::new(&text.1))
                .with_screen_position((
                    (position.x + 1.0) * screen.width as f32 / 2.0,
                    (position.y + 1.0) * screen.height as f32 / 2.0,
                ));

            glyph_brush.queue(section);
        }
    }
}
