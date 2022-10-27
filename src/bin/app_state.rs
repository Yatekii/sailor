use crate::drawing::ui::*;
use crate::*;
use lyon::math::Point;
use stats::Stats;
use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

pub struct AppState {
    pub tile_cache: TileCache,
    pub css_cache: RulesCache,
    pub screen: Screen,
    pub tile_field: TileField,
    pub zoom: f32,
    pub hovered_objects: Vec<Object>,
    pub selected_objects: Vec<EditableObject>,
    pub stats: Stats,
    pub ui: UIState,
    visible_tiles: BTreeMap<TileId, VisibleTile>,
    feature_collection: Arc<RwLock<FeatureCollection>>,
}

impl AppState {
    pub fn new(
        style: impl Into<String>,
        center: Point,
        width: u32,
        height: u32,
        zoom: f32,
        hidpi_factor: f64,
    ) -> Self {
        Self {
            tile_cache: TileCache::new(CONFIG.general.data_root.clone()),
            css_cache: RulesCache::try_load_from_file(style)
                .expect("Unable to load the style file. Please consult the log."),
            screen: Screen::new(
                center,
                width,
                height,
                CONFIG.renderer.tile_size,
                hidpi_factor,
            ),
            tile_field: TileField::new(TileId::new(8, 0, 0), TileId::new(8, 0, 0)),
            zoom,
            hovered_objects: vec![],
            selected_objects: vec![],
            stats: Stats::new(),
            ui: UIState::new(),
            visible_tiles: BTreeMap::new(),
            feature_collection: Arc::new(RwLock::new(FeatureCollection::new(
                CONFIG.renderer.max_features as u32,
            ))),
        }
    }

    pub fn visible_tiles(&self) -> &BTreeMap<TileId, VisibleTile> {
        &self.visible_tiles
    }

    pub fn feature_collection(&self) -> Arc<RwLock<FeatureCollection>> {
        self.feature_collection.clone()
    }

    pub fn load_tiles(&mut self) {
        let tile_field = self.screen.get_tile_boundaries_for_zoom_level(self.zoom, 1);

        // Remove old bigger tiles which are not in the FOV anymore.
        let old_tile_field = self
            .screen
            .get_tile_boundaries_for_zoom_level(self.zoom - 1.0, 2);
        let key_iter: Vec<_> = self.visible_tiles.keys().copied().collect();
        for key in key_iter {
            if key.z == (self.zoom - 1.0) as u32 {
                if !old_tile_field.contains(&key) {
                    self.visible_tiles.remove(&key);
                }
            } else if !tile_field.contains(&key) {
                self.visible_tiles.remove(&key);
            }
        }

        self.tile_cache.finalize_loaded_tiles();
        for tile_id in tile_field.iter() {
            if let Entry::Vacant(entry) = self.visible_tiles.entry(tile_id) {
                self.tile_cache.request_tile(
                    &tile_id,
                    self.feature_collection.clone(),
                    &CONFIG.renderer.selection_tags.clone(),
                );

                let tile_cache = &mut self.tile_cache;
                if let Some(tile) = tile_cache.try_get_tile(&tile_id) {
                    let mut visible_tile = VisibleTile::new(tile);

                    // visible_tile.load_collider();

                    entry.insert(visible_tile);

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
                        if self.visible_tiles.contains_key(tile_id) {
                            count += 1;
                        }
                    }
                    if count == 4 {
                        self.visible_tiles.remove(&TileId::new(
                            tile_id.z - 1,
                            num_x / 2,
                            num_y / 2,
                        ));
                    }

                    // Remove old smaller tiles when all 4 smaller tiles are loaded.
                    for tile_id in &[
                        TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2),
                        TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2),
                        TileId::new(tile_id.z + 1, tile_id.x * 2 + 1, tile_id.y * 2 + 1),
                        TileId::new(tile_id.z + 1, tile_id.x * 2, tile_id.y * 2 + 1),
                    ] {
                        self.visible_tiles.remove(tile_id);
                    }
                } else {
                    log::trace!("Could not read tile {} from cache.", tile_id);
                }
            }
        }

        if let Ok(mut feature_collection) = self.feature_collection.try_write() {
            feature_collection.load_styles(self.zoom, &mut self.css_cache);
        }
    }

    pub fn update_hovered_objects(&mut self, point: (f32, f32)) {
        self.hovered_objects =
            Collider::get_hovered_objects(&self.visible_tiles, &self.screen, self.zoom, point);
    }

    pub fn update_selected_hover_objects(&mut self) {
        self.selected_objects = self
            .hovered_objects
            .iter()
            .map(|o| EditableObject::new(o.clone()))
            .collect();
    }

    pub fn advance_selected_object(&mut self) {
        let len = self.selected_objects.len();
        for i in 0..len {
            if self.selected_objects[i].selected {
                self.selected_objects[(i + 1) % len].selected = true;
                self.selected_objects[i].selected = true;
            }
        }
    }

    pub fn set_center(&mut self, center: (f32, f32)) {
        let tile_coordinate = deg2num(center.0, center.1, self.zoom as u32);
        self.screen.center = num_to_global_space(&tile_coordinate);
    }
}

pub struct EditableObject {
    pub object: Object,
    pub selected: bool,
}

impl EditableObject {
    pub fn new(object: Object) -> Self {
        Self {
            object,
            selected: false,
        }
    }
}
