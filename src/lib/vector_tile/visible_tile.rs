use std::sync::{
    Arc,
    RwLock,
};

use wgpu::*;
use wgpu_glyph::{
    Section,
    GlyphBrush,
};

use crate::*;

pub struct VisibleTile {
    tile: Arc<RwLock<Tile>>,
    gpu_tile: Arc<RwLock<Option<LoadedGPUTile>>>,
    tile_collider: Arc<RwLock<TileCollider>>,
}

impl VisibleTile {
    pub fn new(tile: Arc<RwLock<Tile>>) -> Self {
        Self {
            tile: tile,
            gpu_tile: Arc::new(RwLock::new(None)),
            tile_collider: Arc::new(RwLock::new(TileCollider::new())),
        }
    }

    pub fn tile_id(&self) -> TileId {
        self.tile.read().unwrap().tile_id()
    }

    pub fn extent(&self) -> u16 {
        self.tile.read().unwrap().extent()
    }

    pub fn objects(&self) -> Arc<RwLock<Vec<Object>>> {
        self.tile.read().unwrap().objects()
    }
    
    pub fn load_to_gpu(&self, device: &Device) {
        let read_tile = self.tile.read().unwrap();
        let mut write_gpu_tile = self.gpu_tile.write().unwrap();
        *write_gpu_tile = Some(LoadedGPUTile::load(device, &read_tile));
    }

    pub fn unload_from_gpu(&self) {
        let mut write_gpu_tile = self.gpu_tile.write().unwrap();
        *write_gpu_tile = None;
    }

    pub fn is_loaded_to_gpu(&self) -> bool {
        self.gpu_tile.read().unwrap().is_some()
    }

    pub fn load_collider(&mut self) {
        self.tile_collider.load(self.tile.clone());
    }

    pub fn collider(&self) -> Arc<RwLock<TileCollider>> {
        self.tile_collider.clone()
    }

    pub fn paint(
        &self,
        render_pass: &mut RenderPass,
        blend_pipeline: &RenderPipeline,
        feature_collection: &FeatureCollection,
        tile_id: u32
    ) {
        if let Some(data) = self.gpu_tile.try_read().unwrap().as_ref() {
            render_pass.set_index_buffer(&data.index_buffer, 0);
            render_pass.set_vertex_buffers(0, &[(&data.vertex_buffer, 0)]);

            let features = {
                let read_tile = self.tile.read().unwrap();
                let mut features = read_tile.features().clone();
                features.sort_by(|a, b| {
                    feature_collection
                        .get_zindex(a.0)
                        .partial_cmp(&feature_collection.get_zindex(b.0)).unwrap()
                });
                features
            };

            let mut i = 0;
            render_pass.set_pipeline(blend_pipeline);
            for (id, range) in &features {
                if range.len() > 0 && feature_collection.is_visible(*id) {
                    render_pass.set_stencil_reference(i as u32);
                    i += 1;

                    let range_start = (tile_id << 1) | 1;
                    render_pass.draw_indexed(range.clone(), 0, 0 + range_start .. 1 + range_start);

                    if feature_collection.has_outline(*id) {
                        let range_start = tile_id << 1;
                        render_pass.draw_indexed(range.clone(), 0, 0 + range_start .. 1 + range_start);
                    }
                }
            }
        }
    }

    pub fn queue_text(
        &self,
        glyph_brush: &mut GlyphBrush<'static, ()>,
        screen: &Screen,
        z: f32
    ) {
        let read_tile = self.tile.read().unwrap();
        let matrix = screen.tile_to_global_space(z, &read_tile.tile_id());
        for text in read_tile.text() {
            let position = matrix * glm::vec4((text.0).0, (text.0).1, 0.0, 1.0);
            // dbg!(&position);
            let section = Section {
                text: &text.1,
                screen_position: (
                    (position.x + 1.0) * screen.width as f32 / 2.0,
                    (position.y + 1.0) * screen.height as f32 / 2.0
                ),
                ..Section::default() // color, position, etc
            };

            glyph_brush.queue(section);
        }
    }
}