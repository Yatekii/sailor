use lyon::math::Point;
use nalgebra_glm::{Vec2, vec2};
use osm::css::RulesCache;
use osm::feature::collection::FeatureCollection;
use osm::math::{Camera, Coord, Geo, PointF64, TileId, deg2num, tile_to_world_space};
use osm::cache::CacheStats;
use osm::object::Object;
use std::sync::{Arc, Mutex, RwLock};
use winit::dpi::PhysicalSize;

use crate::config::CONFIG;
use crate::drawing::ui::state::UIState;
use crate::stats::Stats;

pub struct AppState {
    pub css_cache: RulesCache,
    pub screen: Camera,
    pub hovered_objects: Arc<Mutex<Vec<Object>>>,
    selected_objects: Vec<EditableObject>,
    selected_object: usize,
    selected_object_labels: Vec<String>,
    pub stats: Stats,
    pub ui: UIState,
    /// Tile-cache stats for the current frame, copied from the map for the debug view.
    pub tile_stats: CacheStats,
    feature_collection: Arc<RwLock<FeatureCollection>>,
    cursor: Vec2,
}

impl AppState {
    pub fn new(
        style: impl Into<String>,
        center: Point,
        size: PhysicalSize<u32>,
        zoom: f32,
        hidpi_factor: f64,
        feature_collection: Arc<RwLock<FeatureCollection>>,
    ) -> Self {
        Self {
            css_cache: RulesCache::try_load_from_file(style)
                .expect("Unable to load the style file. Please consult the log."),
            screen: Camera::new(
                center,
                size.width as f32,
                size.height as f32,
                CONFIG.renderer.tile_size as f32,
                hidpi_factor as f32,
                zoom,
            ),
            hovered_objects: Arc::new(Mutex::new(Vec::new())),
            selected_objects: Vec::with_capacity(64),
            selected_object: 0,
            selected_object_labels: Vec::with_capacity(64),
            stats: Stats::new(),
            ui: UIState::new(),
            tile_stats: CacheStats::default(),
            feature_collection,
            cursor: vec2(0.0, 0.0),
        }
    }

    pub fn feature_collection(&self) -> Arc<RwLock<FeatureCollection>> {
        self.feature_collection.clone()
    }

    pub fn update_selected_from_hover_objects(&mut self) {
        let hovered_objects = self.hovered_objects.lock().unwrap();
        self.selected_objects = hovered_objects
            .iter()
            .map(|o| EditableObject::new(o.tile_id, o.clone()))
            .collect();
        drop(hovered_objects);
        self.selected_object = 0;
        self.refresh_labels();
    }

    pub fn advance_selected_object(&mut self) {
        let len = self.selected_objects.len();
        self.selected_object = (self.selected_object + 1) % len;
        self.refresh_labels();
    }

    pub fn select_object(&mut self, index: usize) {
        self.selected_object = index;
        self.refresh_labels();
    }

    fn refresh_labels(&mut self) {
        self.selected_object_labels = self
            .selected_objects
            .iter()
            .enumerate()
            .map(|(i, o)| {
                let selector = o.object.selector();
                if i == self.selected_object {
                    format!("{selector} •")
                } else {
                    format!("{selector}")
                }
            })
            .collect();
    }

    pub(crate) fn selected_object(&self) -> Option<&EditableObject> {
        self.selected_objects.get(self.selected_object)
    }

    pub fn selected_objects(&mut self) -> &[EditableObject] {
        &self.selected_objects
    }

    pub fn selected_object_labels(&mut self) -> &[String] {
        &self.selected_object_labels
    }

    pub fn set_center(&mut self, center: (f32, f32)) {
        let tile_coordinate = deg2num(Coord::<Geo>::new(center.1, center.0), self.screen.zoom as u32);
        let p = tile_to_world_space(&tile_coordinate);
        self.screen.center = PointF64::new(p.x as f64, p.y as f64);
    }

    pub(crate) fn scale_factor_updated(&mut self, scale_factor: f32) {
        // Rebuild the camera for the new scale factor but keep the f64 center as
        // is — routing it back through the f32 `Camera::new` would drop precision.
        let center = self.screen.center;
        self.screen = Camera::new(
            Point::new(0.0, 0.0),
            self.screen.width,
            self.screen.height,
            self.screen.tile_size(),
            scale_factor,
            self.screen.zoom,
        );
        self.screen.center = center;
    }

    pub fn set_cursor(&mut self, cursor: Vec2) {
        self.cursor = cursor;
    }

    pub fn cursor(&self) -> Vec2 {
        self.cursor
    }
}

pub struct EditableObject {
    pub tile_id: TileId,
    pub object: Object,
}

impl EditableObject {
    pub fn new(tile_id: TileId, object: Object) -> Self {
        Self { tile_id, object }
    }
}
