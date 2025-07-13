use egui::{ScrollArea, Ui};

use crate::app_state::AppState;

pub fn view_layer_toggle(ui: &mut Ui, app_state: &mut AppState) {
    ui.heading("Layers");
    ScrollArea::vertical().max_height(800.0).show(ui, |ui| {
        let feature_collection = app_state.feature_collection();
        let mut features = feature_collection.write().unwrap();
        let layers = features.layers_mut();
        for layer in layers {
            ui.checkbox(&mut layer.display, &layer.name);
        }
    });
}
