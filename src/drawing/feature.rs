use crate::css::{
    RulesCache,
    Selector,
    CSSValue,
    Color,
    Number,
};

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
            r: value.r as f32 / 255.0, g: value.g as f32 / 255.0, b: value.b as f32 / 255.0, a: value.a,
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
    _padding: [u32; 1],
}

#[derive(Debug, Clone, Default)]
pub struct Feature {
    pub selector: Selector,
    pub style: FeatureStyle,
}

impl Feature {
    pub fn new(selector: Selector) -> Self {
        Self {
            selector,
            style: Default::default(),
        }
    }

    pub fn load_style(&mut self, zoom: f32, css_cache: &mut RulesCache) {
        let rules = css_cache.get_matching_rules(
            &self.selector.clone().with_any("zoom".to_string(), (zoom.floor() as u32).to_string())
        );

        let background_color = rules
            .iter()
            .filter_map(|r| r.kvs.get("background-color"))
            .last();

        if let Some(color) = background_color {
            match color {
                CSSValue::Color(bg) => {
                    self.style.background_color = bg.clone().into();
                },
                CSSValue::String(string) => {
                    match &string[..] {
                        "red" => self.style.background_color = Color::RED.into(),
                        "green" => self.style.background_color = Color::GREEN.into(),
                        "blue" => self.style.background_color = Color::BLUE.into(),
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
                    self.style.border_color = bg.clone().into();
                    self.style.border_color;
                },
                CSSValue::String(string) => {
                    match &string[..] {
                        "red" => self.style.border_color = Color::RED.into(),
                        "blue" => self.style.border_color = Color::GREEN.into(),
                        "green" => self.style.border_color = Color::BLUE.into(),
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

        if let Some(border_width) = border_width {
            match border_width {
                CSSValue::Number(number) => match number {
                    Number::Px(px) => self.style.border_width = px.clone().into(),
                    value => log::info!("The value '{:?}' is currently not supported for 'border-width'.", value)
                },
                value => log::info!("The value '{:?}' is currently not supported for 'border-width'.", value)
            }
        }

        let display = rules
            .iter()
            .filter_map(|r| r.kvs.get("display"))
            .last();

        if let Some(display) = display {
            match display {
                CSSValue::String(value) => match &value[..] {
                    "none" => self.style.display = false,
                    _ => self.style.display = true,
                },
                value => log::info!("The value '{:?}' is currently not supported for 'display'.", value)
            }
        } else {
            self.style.display = true;
        }

        let line_width = rules
            .iter()
            .filter_map(|r| r.kvs.get("line-width"))
            .last();

        if let Some(line_width) = line_width {
            match line_width {
                CSSValue::Number(number) => match number {
                    Number::Px(px) => self.style.line_width = (px.clone() as u32) << 1 | 0,
                    Number::World(world) => self.style.line_width = ((world.clone() as u32) << 1) | 1,
                    value => log::info!("The value '{:?}' is currently not supported for 'line-width'.", value)
                },
                value => log::info!("The value '{:?}' is currently not supported for 'line-width'.", value)
            }
        } else {
            self.style.line_width = 0;
        }
    }
}