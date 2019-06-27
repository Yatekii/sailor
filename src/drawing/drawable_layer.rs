use crate::css::{
    RulesCache,
    Selector,
    CSSValue,
    Color,
    Number,
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
    border_color: DrawableColor,
    border_width: f32,
    _padding: [u32; 3],
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
    pub fn from_layer(start_vertex: u32, end_vertex: u32, layer: &Layer, zoom: f32, css_cache: &mut RulesCache) -> Self {
        let mut drawable_layer = Self {
            start_vertex,
            end_vertex,
            layer_data: LayerData {
                background_color: Color::WHITE.into(),
                border_color: Color::WHITE.into(),
                border_width: 0.0,
                _padding: [0; 3],
            },
            layer_info: LayerInfo {
                name: layer.name.clone(),
            },
        };
        drawable_layer.load_style(zoom, css_cache);
        drawable_layer
    }

    pub fn load_style(&mut self, zoom: f32, css_cache: &mut RulesCache) {
        let rules = css_cache.get_matching_rules(
            &Selector::new()
                .with_type("layer".to_string())
                .with_any("name".to_string(), self.layer_info.name.clone())
                .with_any("zoom".to_string(), (zoom.floor() as u32).to_string())
        );
        let background_color = rules
            .iter()
            .filter_map(|r| r.kvs.get("background-color"))
            .last();

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
                        color => {
                            log::info!("The color '{}' is currently not supported.", color)
                        },
                    }
                },
                value => {
                    log::info!("The value '{:?}' is currently not supported for 'background-color'.", value);
                },
            }
        }

        let border_color = rules
            .iter()
            .filter_map(|r| r.kvs.get("border-color"))
            .last();

        if let Some(color) = border_color {
            match color {
                CSSValue::Color(bg) => {
                    self.layer_data.border_color = bg.clone().into();
                    self.layer_data.border_color;
                },
                CSSValue::String(string) => {
                    match &string[..] {
                        "red" => self.layer_data.border_color = Color::RED.into(),
                        "blue" => self.layer_data.border_color = Color::GREEN.into(),
                        "green" => self.layer_data.border_color = Color::BLUE.into(),
                        // Other CSS colors to come later.
                        color => {
                            log::info!("The color '{}' is currently not supported.", color)
                        },
                    }
                },
                value => {
                    log::info!("The value '{:?}' is currently not supported for 'border-color'.", value);
                },
            }
        }

        let border_width = rules
            .iter()
            .filter_map(|r| r.kvs.get("border-width"))
            .last();

        if let Some(width) = border_width {
            match width {
                CSSValue::Number(number) => match number {
                    Number::Px(px) => self.layer_data.border_width = px.clone().into()
                },
                value => {
                    log::info!("The value '{:?}' is currently not supported for 'border-width'.", value);
                }
            }
        }
    }
}