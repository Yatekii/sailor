use std::sync::Arc;

use glyphon::{Cache, FontSystem, Resolution, SwashCache, TextAtlas, Viewport};
use wgpu::{Device, Queue, TextureFormat};

/// Shared glyphon text resources: one font load and one glyph atlas for the
/// whole frame. Layers keep their own cheap `TextRenderer` but prepare and
/// render against these, so the font and atlas aren't duplicated per layer.
pub struct TextStack {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
    pub atlas: TextAtlas,
    pub viewport: Viewport,
}

impl TextStack {
    pub fn new(device: &Device, queue: &Queue, format: TextureFormat) -> Self {
        // Bundle the Ruda font so text is deterministic and works on the web,
        // which has no system fonts.
        let mut font_system = FontSystem::new_with_fonts([
            glyphon::fontdb::Source::Binary(Arc::new(
                include_bytes!("../../../../config/Ruda-Regular.ttf").to_vec(),
            )),
            glyphon::fontdb::Source::Binary(Arc::new(
                include_bytes!("../../../../config/Ruda-Bold.ttf").to_vec(),
            )),
        ]);
        {
            let db = font_system.db_mut();
            db.set_sans_serif_family("Ruda");
            db.set_serif_family("Ruda");
            db.set_monospace_family("Ruda");
        }
        let swash_cache = SwashCache::new();
        let cache = Cache::new(device);
        let viewport = Viewport::new(device, &cache);
        let atlas = TextAtlas::new(device, queue, &cache, format);
        Self {
            font_system,
            swash_cache,
            atlas,
            viewport,
        }
    }

    /// Refresh the viewport to the current render resolution; call once per frame.
    pub fn update_viewport(&mut self, queue: &Queue, width: u32, height: u32) {
        self.viewport.update(queue, Resolution { width, height });
    }
}
