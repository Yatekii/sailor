use egui::Ui;
use osm::css::{CSSValue, Number, Rule};

pub fn add_slider_float(ui: &mut Ui, rule: &mut Rule, attribute: &str) {
    let default_number = CSSValue::Number(Number::Px(0.0));
    let value = if let Some(value) = rule.kvs.get(attribute) {
        value
    } else {
        &default_number
    };
    let mut value = match value {
        CSSValue::Number(Number::Px(px)) => *px,
        _ => 0.0,
    };

    ui.add(egui::Slider::new(&mut value, 0.0..=10.0).text(attribute));

    rule.kvs
        .insert(attribute.to_string(), CSSValue::Number(Number::Px(value)));
}
