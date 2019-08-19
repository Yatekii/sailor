use crate::stats::Stats;
use crate::vector_tile::object::Object;
use crate::interaction::collider::Collider;
use lyon::math::Point;
use crate::vector_tile::{
    math::{
        TileField,
        Screen,
        TileId,
    },
    cache::{
        TileCache,
    },
};
use crate::css::RulesCache;

pub struct AppState {
    pub tile_cache: TileCache,
    pub css_cache: RulesCache,
    pub screen: Screen,
    pub tile_field: TileField,
    pub zoom: f32,
    pub hovered_objects: Vec<Object>,
    pub selected_objects: Vec<EditableObject>,
    pub stats: Stats,
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
            tile_cache: TileCache::new(),
            css_cache: RulesCache::try_load_from_file(style).expect("Unable to load the style file. Please consult the log."),
            screen: Screen::new(center, width, height, hidpi_factor),
            tile_field: TileField::new(TileId::new(8, 0, 0), TileId::new(8, 0, 0)),
            zoom,
            hovered_objects: vec![],
            selected_objects: vec![],
            stats: Stats::new(),
        }
    }

    pub fn update_hovered_objects(&mut self, point: (f32, f32)) {
        self.hovered_objects = Collider::get_hovered_objects(self, point)
            .iter()
            .map(|o| (**o).clone())
            .collect();
    }

    pub fn update_selected_hover_objects(&mut self) {
        self.selected_objects =
            self.hovered_objects
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