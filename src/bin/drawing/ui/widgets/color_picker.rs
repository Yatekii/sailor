use egui::{color_picker::Alpha, Rgba, Ui};
use osm::css::{CSSValue, Color, Rule};

pub fn add_color_picker(ui: &mut Ui, rule: &mut Rule, attribute: &str) {
    let default_color = CSSValue::Color(Color::TRANSPARENT);
    let color = if let Some(color) = rule.kvs.get(attribute) {
        color
    } else {
        &default_color
    };
    let color = match color {
        CSSValue::String(string) => match &string[..] {
            "red" => Color::RED,
            "green" => Color::GREEN,
            "blue" => Color::BLUE,
            "black" => Color::BLACK,
            "white" => Color::WHITE,
            _ => Color::TRANSPARENT,
        },
        CSSValue::Color(color) => color.clone(),
        _ => Color::TRANSPARENT, // This should never happen, but transparent should be a decent fallback
    };
    let mut color = Rgba::from_rgba_premultiplied(color.r, color.g, color.b, color.a);
    egui::widgets::color_picker::color_edit_button_rgba(ui, &mut color, Alpha::OnlyBlend);

    rule.kvs.insert(
        attribute.to_string(),
        CSSValue::Color(Color {
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        }),
    );
}
