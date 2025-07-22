use egui::{RichText, Ui};

use crate::{
    app_state::{AppState, EditableObject},
    drawing::ui::widgets::{
        color_picker::add_color_picker, display_toggle::add_display_none,
        float_slider::add_slider_float, key_value_table::widget_key_value_table,
    },
};

pub fn view_inspector(ui: &mut Ui, app_state: &mut AppState) {
    if !app_state.selected_objects().is_empty() {
        selected_objects(ui, app_state);
        ui.separator();
        selected_object(ui, app_state);
        ui.separator();
    }
    hovered_objects(ui, app_state);
    ui.separator();
}

fn selected_object(ui: &mut Ui, app_state: &mut AppState) {
    ui.horizontal(|ui| {
        ui.heading("Selected Object");
        ui.label(RichText::from("press <tab> to cycle").small());
    });

    if let Some(EditableObject { object, .. }) = app_state.selected_object() {
        let selector = object.selector().clone();
        let tags = object.tags();

        ui.label(RichText::from("Tags").strong());
        ui.separator();

        let data = tags
            .iter()
            .map(|(k, v)| (k.as_str(), v.clone()))
            .collect::<Vec<_>>();
        widget_key_value_table(ui, &data);
        ui.separator();

        let mut rules = app_state.css_cache.get_matching_rules_mut(&selector);
        for rule in rules.iter_mut() {
            ui.label(format!("{}", rule.selector));
            add_color_picker(ui, rule, "background-color");
            add_color_picker(ui, rule, "border-color");
            add_slider_float(ui, rule, "border-width");
            add_slider_float(ui, rule, "line-width");
            add_display_none(ui, rule, "display");
        }
    } else {
        ui.label("No Object selected");
    }
}

fn selected_objects(ui: &mut Ui, app_state: &mut AppState) {
    ui.heading("Selected Objects");
    ui.vertical(|ui| {
        let mut advance = None;
        for (i, item) in app_state.selected_object_labels().iter().enumerate() {
            if ui
                .label(item)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .clicked()
            {
                advance = Some(i);
            };
        }
        if let Some(i) = advance {
            app_state.select_object(i);
        }
    });
}

fn hovered_objects(ui: &mut Ui, app_state: &mut AppState) {
    ui.heading("Hovered Objects");
    ui.vertical(|ui| {
        let hovered_objects = app_state.hovered_objects.lock().unwrap();
        for object in hovered_objects.iter() {
            ui.label(object.selector().to_string());
        }
    });
}
