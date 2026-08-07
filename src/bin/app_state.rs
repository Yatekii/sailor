use lyon::math::Point;
use nalgebra_glm::{Vec2, vec2};
use osm::cache::TileCache;
use osm::config::MAX_TILES;
use osm::css::RulesCache;
use osm::feature::collection::FeatureCollection;
use osm::interaction::collider::{Collider, VisibleTile};
use osm::math::{Screen, TileId, deg2num, tile_to_world_space};
use osm::object::Object;
use std::sync::{Arc, Mutex, RwLock};
use winit::dpi::PhysicalSize;

use crate::config::CONFIG;
use crate::drawing::ui::state::UIState;
use crate::stats::Stats;

pub struct AppState {
    pub tile_cache: TileCache,
    pub css_cache: RulesCache,
    pub screen: Screen,
    pub zoom: f32,
    pub hovered_objects: Arc<Mutex<Vec<Object>>>,
    selected_objects: Vec<EditableObject>,
    selected_object: usize,
    selected_object_labels: Vec<String>,
    pub stats: Stats,
    pub ui: UIState,
    pub visible_tiles: Vec<TileId>,
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
    ) -> Self {
        Self {
            tile_cache: TileCache::new(CONFIG.general.data_root.clone()),
            css_cache: RulesCache::try_load_from_file(style)
                .expect("Unable to load the style file. Please consult the log."),
            screen: Screen::new(
                center,
                size.width as f32,
                size.height as f32,
                CONFIG.renderer.tile_size as f32,
                hidpi_factor as f32,
            ),
            zoom,
            hovered_objects: Arc::new(Mutex::new(Vec::new())),
            selected_objects: Vec::with_capacity(64),
            selected_object: 0,
            selected_object_labels: Vec::with_capacity(64),
            stats: Stats::new(),
            ui: UIState::new(),
            visible_tiles: Vec::new(),
            feature_collection: Arc::new(RwLock::new(FeatureCollection::new())),
            cursor: vec2(0.0, 0.0),
        }
    }

    pub fn visible_tiles(&self) -> &[TileId] {
        &self.visible_tiles
    }

    pub fn feature_collection(&self) -> Arc<RwLock<FeatureCollection>> {
        self.feature_collection.clone()
    }

    pub fn load_tile(&mut self, tile_id: TileId) {
        self.tile_cache.finalize_loaded_tiles();
        if !self.visible_tiles.contains(&tile_id) {
            self.tile_cache.load_tile(
                &tile_id,
                self.feature_collection.clone(),
                &CONFIG.renderer.selection_tags.clone(),
            );

            let tile_cache = &mut self.tile_cache;
            if let Some(tile) = tile_cache.try_get_tile_mut(&tile_id) {
                tile.load_collider();

                self.visible_tiles.push(tile_id);
            }
        }

        if let Ok(mut feature_collection) = self.feature_collection.try_write() {
            feature_collection.load_styles(self.zoom, &mut self.css_cache);
        }
    }

    pub fn load_tiles(&mut self) {
        let tile_field = self.screen.get_tile_boundaries_for_zoom_level(self.zoom, 1);

        // Remove old bigger tiles which are not in the FOV anymore.
        let old_tile_field = self
            .screen
            .get_tile_boundaries_for_zoom_level(self.zoom - 1.0, 2);
        for tile_id in &self.visible_tiles.clone() {
            if tile_id.z == (self.zoom - 1.0) as u32 {
                if !old_tile_field.contains(tile_id) {
                    self.remove_visible_tile(tile_id);
                }
            } else if !tile_field.contains(tile_id) {
                self.remove_visible_tile(tile_id);
            }
        }

        self.tile_cache.finalize_loaded_tiles();
        for tile_id in tile_field.iter() {
            if !self.visible_tiles.contains(&tile_id) {
                self.tile_cache.load_tile(
                    &tile_id,
                    self.feature_collection.clone(),
                    &CONFIG.renderer.selection_tags.clone(),
                );

                let tile_cache = &mut self.tile_cache;
                if let Some(tile) = tile_cache.try_get_tile_mut(&tile_id) {
                    tile.load_collider();

                    self.visible_tiles.push(tile_id);

                    // Remove old bigger tile when all 4 smaller tiles are loaded.
                    let mut count = 0;
                    let num_x = (tile_id.x / 2) * 2;
                    let num_y = (tile_id.y / 2) * 2;
                    for tile_id in &[
                        TileId::new(tile_id.z, num_x, num_y),
                        TileId::new(tile_id.z, num_x + 1, num_y),
                        TileId::new(tile_id.z, num_x + 1, num_y + 1),
                        TileId::new(tile_id.z, num_x, num_y + 1),
                    ] {
                        if !tile_field.contains(tile_id) {
                            count += 1;
                            continue;
                        }
                        if self.visible_tiles.contains(tile_id) {
                            count += 1;
                        }
                    }
                    if count == 4 {
                        let tile_id = TileId::new(tile_id.z - 1, num_x / 2, num_y / 2);
                        self.remove_visible_tile(&tile_id);
                    }

                    // Remove old smaller tiles when all 4 smaller tiles are loaded.
                    for tile_id in &[
                        TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2),
                        TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2),
                        TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2 + 1),
                        TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2 + 1),
                    ] {
                        self.remove_visible_tile(tile_id);
                    }
                } else {
                    log::trace!("Could not read tile {tile_id} from cache.");
                }
            }
        }

        if let Ok(mut feature_collection) = self.feature_collection.try_write() {
            feature_collection.load_styles(self.zoom, &mut self.css_cache);
        }
    }

    #[track_caller]
    fn remove_visible_tile(&mut self, tile_id: &TileId) {
        if let Some(index) = self.visible_tiles.iter().position(|x| x == tile_id) {
            self.visible_tiles.swap_remove(index);
        }
    }

    pub fn update_hovered_objects(&mut self, point: (f32, f32)) {
        let hovered_objects = self.hovered_objects.clone();
        let screen = self.screen.clone();
        let zoom = self.zoom;
        let mut visible_tiles = Vec::with_capacity(MAX_TILES);
        for tile_id in self.visible_tiles.iter() {
            let tile = self.tile_cache.try_get_tile(tile_id).unwrap();
            visible_tiles.push(VisibleTile {
                tile_id: *tile_id,
                extent: tile.extent() as f32,
                collider: tile.collider(),
                objects: tile.objects(),
            });
        }
        osm::platform::spawn(async move {
            let objects = Collider::get_hovered_objects(&visible_tiles, &screen, zoom, point);
            let mut hovered_objects = hovered_objects.lock().unwrap();
            *hovered_objects = objects;
        });
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
        let tile_coordinate = deg2num(center.0, center.1, self.zoom as u32);
        self.screen.center = tile_to_world_space(&tile_coordinate);
    }

    pub(crate) fn scale_factor_updated(&mut self, scale_factor: f32) {
        self.screen = Screen::new(
            self.screen.center,
            self.screen.width,
            self.screen.height,
            self.screen.tile_size(),
            scale_factor,
        )
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
