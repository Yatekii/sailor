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
}

impl AppState {
    pub fn new(
        style: impl Into<String>,
        center: Point,
        width: u32,
        height: u32,
        zoom: f32,
    ) -> Self {
        Self {
            tile_cache: TileCache::new(),
            css_cache: RulesCache::try_load_from_file(style).expect("Unable to load the style file. Please consult the log."),
            screen: Screen::new(center, width, height),
            tile_field: TileField::new(TileId::new(8, 0, 0), TileId::new(8, 0, 0)),
            zoom,
        }
    }
}