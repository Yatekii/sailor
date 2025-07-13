use std::sync::Arc;

use egui::{ScrollArea, Ui};

use crate::{
    app_state::{AppState, EditableObject},
    drawing::ui::widgets::{
        color_picker::add_color_picker, display_toggle::add_display_none,
        float_slider::add_slider_float,
    },
};

pub fn view_inspector(ui: &mut Ui, app_state: &mut AppState) {
    ui.heading("Inspector");
    ScrollArea::vertical().max_height(800.0).show(ui, |ui| {
        let mut size = ui.min_size();
        size[1] = 100.0;
        ui.heading("Hovered Objects");
        ui.vertical(|ui| {
            let hovered_objects = app_state.hovered_objects.lock().unwrap();
            let objects = hovered_objects
                .iter()
                .map(|o| {
                    let pts = o.points().iter().fold(0.0, |a, b| a + b.x + b.y);
                    format!("{pts}: {}", o.selector())
                })
                .collect::<Vec<_>>()
                .join("\n");
            ui.label(objects);
        });

        let mut item: i32 = 0;
        for i in 0..app_state.selected_objects.len() {
            if app_state.selected_objects[i].selected {
                app_state.selected_objects[i].selected = false;
                item = i as i32;
            }
        }
        let items = Arc::new(
            app_state
                .selected_objects
                .iter()
                .map(|o| format!("{}", o.object.selector()))
                .collect::<Vec<_>>(),
        );
        let items_clone = items.clone();

        ui.heading("Selected Objects");
        ui.vertical(|ui| {
            for item in &*items_clone {
                ui.label(item);
            }
        });

        ui.heading("Selected Object");
        if item >= 0 && !items.is_empty() {
            app_state.selected_objects[item as usize].selected = true;
        }

        let objects = &mut app_state.selected_objects;

        if let Some(EditableObject { object, .. }) =
            objects.iter_mut().find(|object| object.selected)
        {
            ui.separator();
            ui.label("Tags");
            ui.separator();

            ui.label(format!("{:#?}", object.tags()));

            ui.separator();
            ui.label("Applying rules");
            ui.separator();

            let mut rules = app_state
                .css_cache
                .get_matching_rules_mut(object.selector());
            for rule in rules.iter_mut() {
                ui.collapsing(format!("{}", rule.selector), |ui| {
                    add_color_picker(ui, rule, "background-color");
                    add_color_picker(ui, rule, "border-color");
                    add_slider_float(ui, rule, "border-width");
                    add_slider_float(ui, rule, "line-width");
                    add_display_none(ui, rule, "display");
                });
            }
        } else {
            ui.separator();
            ui.label("No Object selected");
            ui.separator();
        }
    });
}
