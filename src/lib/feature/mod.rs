mod collection;

pub use collection::*;

use crate::*;

#[derive(Debug, Copy, Clone, Default)]
pub struct DrawableColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl From<Color> for DrawableColor {
    fn from(value: Color) -> Self {
        Self {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

#[derive(Debug, Copy, Clone, Default)]
pub struct FeatureStyle {
    pub background_color: DrawableColor,
    pub border_color: DrawableColor,
    pub border_width: f32,
    pub line_width: u32,
    pub display: bool,
    pub z_index: f32,
}

#[derive(Debug, Clone, Default)]
pub struct Feature {
    pub selector: Selector,
    pub layer_id: u32,
    pub id: u32,
    pub style: FeatureStyle,
}

impl Feature {
    pub fn new(selector: Selector, layer_id: u32) -> Self {
        Self {
            selector,
            layer_id,
            // The featureid is set later on.
            id: 0,
            style: Default::default(),
        }
    }

    pub fn load_style(&mut self, zoom: f32, css_cache: &mut RulesCache) {
        let rules = css_cache.get_matching_rules(
            &self
                .selector
                .clone()
                .with_any("zoom".to_string(), (zoom.floor() as u32).to_string()),
        );

        let background_color = rules
            .iter()
            .filter_map(|r| r.kvs.get("background-color"))
            .last();

        if let Some(color) = background_color {
            match color {
                CSSValue::Color(bg) => {
                    self.style.background_color = bg.clone().into();
                }
                CSSValue::String(string) => {
                    match &string[..] {
                        "red" => self.style.background_color = Color::RED.into(),
                        "green" => self.style.background_color = Color::GREEN.into(),
                        "blue" => self.style.background_color = Color::BLUE.into(),
                        "black" => self.style.background_color = Color::BLACK.into(),
                        "white" => self.style.background_color = Color::WHITE.into(),
                        // Other CSS colors to come later.
                        color => log::info!("The color '{}' is currently not supported.", color),
                    }
                }
                value => {
                    log::info!(
                        "The value '{:?}' is currently not supported for 'background-color'.",
                        value
                    );
                }
            }
        }

        let border_color = rules
            .iter()
            .filter_map(|r| r.kvs.get("border-color"))
            .last();

        if let Some(color) = border_color {
            match color {
                CSSValue::Color(bg) => {
                    self.style.border_color = bg.clone().into();
                    self.style.border_color;
                }
                CSSValue::String(string) => {
                    match &string[..] {
                        "red" => self.style.border_color = Color::RED.into(),
                        "blue" => self.style.border_color = Color::GREEN.into(),
                        "green" => self.style.border_color = Color::BLUE.into(),
                        "black" => self.style.background_color = Color::BLACK.into(),
                        "white" => self.style.background_color = Color::WHITE.into(),
                        // Other CSS colors to come later.
                        color => log::info!("The color '{}' is currently not supported.", color),
                    }
                }
                value => {
                    log::info!(
                        "The value '{:?}' is currently not supported for 'border-color'.",
                        value
                    );
                }
            }
        }

        let border_width = rules
            .iter()
            .filter_map(|r| r.kvs.get("border-width"))
            .last();

        if let Some(border_width) = border_width {
            match border_width {
                CSSValue::Number(number) => match number {
                    Number::Px(px) => self.style.border_width = (*px).into(),
                    value => log::info!(
                        "The value '{:?}' is currently not supported for 'border-width'.",
                        value
                    ),
                },
                value => log::info!(
                    "The value '{:?}' is currently not supported for 'border-width'.",
                    value
                ),
            }
        }

        let display = rules.iter().filter_map(|r| r.kvs.get("display")).last();

        if let Some(display) = display {
            match display {
                CSSValue::String(value) => match &value[..] {
                    "none" => self.style.display = false,
                    _ => self.style.display = true,
                },
                value => log::info!(
                    "The value '{:?}' is currently not supported for 'display'.",
                    value
                ),
            }
        } else {
            self.style.display = true;
        }

        let line_width = rules.iter().filter_map(|r| r.kvs.get("line-width")).last();

        if let Some(line_width) = line_width {
            match line_width {
                CSSValue::Number(number) => match number {
                    Number::Px(px) => self.style.line_width = (*px as u32) << 1 | 0b01,
                    Number::World(world) => self.style.line_width = ((*world as u32) << 1) | 0b00,
                    value => log::info!(
                        "The value '{:?}' is currently not supported for 'line-width'.",
                        value
                    ),
                },
                value => log::info!(
                    "The value '{:?}' is currently not supported for 'line-width'.",
                    value
                ),
            }
        } else {
            self.style.line_width = 0;
        }

        let z_index = rules.iter().filter_map(|r| r.kvs.get("z-index")).last();

        if let Some(z_index) = z_index {
            match z_index {
                CSSValue::Number(number) => match number {
                    Number::Unitless(unitless) => self.style.z_index = *unitless,
                    value => log::info!(
                        "The value '{:?}' is currently not supported for 'z-index'.",
                        value
                    ),
                },
                value => log::info!(
                    "The value '{:?}' is currently not supported for 'z-index'.",
                    value
                ),
            }
        } else {
            self.style.z_index = self.layer_id as f32;
        }
    }
}
