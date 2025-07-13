use egui::Ui;
use osm::css::{CSSValue, Rule};

pub fn add_display_none(ui: &mut Ui, rule: &mut Rule, label: &str) {
    let attribute = "display";
    let mut value = if let Some(CSSValue::String(value)) = rule.kvs.get(attribute) {
        !matches!(&value[..], "none")
    } else {
        true
    };

    ui.checkbox(&mut value, label);

    if !value {
        rule.kvs
            .insert(attribute.to_string(), CSSValue::String("none".to_string()));
    } else {
        rule.kvs.remove(attribute);
    }
}
