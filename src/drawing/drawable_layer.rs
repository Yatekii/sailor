use crate::render::css::{
    RulesCache,
    Selector,
    CSSValue,
    Color,
};
use crate::vector_tile::transform::Layer;

pub struct DrawableLayer {
    pub start_vertex: u32,
    pub end_vertex: u32,
    pub background_color: Color,
    pub layer_info: LayerInfo,
}

pub struct LayerInfo {
    pub name: String,
}

impl DrawableLayer {
    pub fn from_layer(start_vertex: u32, end_vertex: u32, layer: &Layer, cache: &RulesCache) -> Self {
        let mut drawable_layer = Self {
            start_vertex,
            end_vertex,
            background_color: Color::WHITE,
            layer_info: LayerInfo {
                name: layer.name.clone(),
            },
        };
        drawable_layer.load_style(cache);
        drawable_layer
    }

    pub fn load_style(&mut self, cache: &RulesCache) {
        let rules = cache.get_matching_rules(
            &Selector::new()
                .with_type("layer".to_string())
                .with_any("name".to_string(), self.layer_info.name.clone())
        );
        let background_color = rules
            .iter()
            .filter_map(|r| r.kvs.get("background-color"))
            .last();

        if let Some(color) = background_color {
            match color {
                CSSValue::Color(bg) => {
                    self.background_color = bg.clone();
                },
                CSSValue::String(string) => {
                    match &string[..] {
                        "red" => self.background_color = Color::RED,
                        "blue" => self.background_color = Color::GREEN,
                        "green" => self.background_color = Color::BLUE,
                        // Other CSS colors to come later.
                        _ => {},
                    }
                },
                _ => {},
            }
        }
    }
}