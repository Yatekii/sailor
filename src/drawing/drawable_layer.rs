use crate::render::css::{
    RulesCache,
    Selector,
    CSSValue,
    Color,
};
use crate::vector_tile::transform::Layer;

#[derive(Debug, Clone)]
pub struct DrawableLayer {
    pub start_vertex: u32,
    pub end_vertex: u32,
    pub layer_data: LayerData,
    pub layer_info: LayerInfo,
}

#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub name: String,
}

#[derive(Debug, Copy, Clone)]
pub struct LayerData {
    background_color: DrawableColor,
}

#[derive(Debug, Copy, Clone)]
pub struct DrawableColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl From<Color> for DrawableColor {
    fn from(value: Color) -> Self {
        Self {
            r: value.r as f32 / 255.0, g: value.g as f32 / 255.0, b: value.b as f32 / 255.0, a: 1.0,
        }
    }
}

impl DrawableLayer {
    pub fn from_layer(start_vertex: u32, end_vertex: u32, layer: &Layer, cache: &RulesCache) -> Self {
        let mut drawable_layer = Self {
            start_vertex,
            end_vertex,
            layer_data: LayerData {
                background_color: Color::WHITE.into()
            },
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
                .with_any("name".to_string(), dbg!(self.layer_info.name.clone()))
        );
        let background_color = rules
            .iter()
            .filter_map(|r| r.kvs.get("background-color"))
            .last();

        dbg!(&background_color);

        if let Some(color) = background_color {
            match color {
                CSSValue::Color(bg) => {
                    self.layer_data.background_color = bg.clone().into();
                },
                CSSValue::String(string) => {
                    match &string[..] {
                        "red" => self.layer_data.background_color = Color::RED.into(),
                        "blue" => self.layer_data.background_color = Color::GREEN.into(),
                        "green" => self.layer_data.background_color = Color::BLUE.into(),
                        // Other CSS colors to come later.
                        _ => {},
                    }
                },
                _ => {},
            }
        }
    }
}