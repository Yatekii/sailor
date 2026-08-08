use egui::{ComboBox, ScrollArea, Slider, Ui};
use osm::wind::WindModel;

use crate::app_state::AppState;

pub fn view_layer_toggle(ui: &mut Ui, app_state: &mut AppState) {
    ui.heading("Wind");
    {
        let wind = &mut app_state.ui.wind;
        ui.checkbox(&mut wind.visible, "Show wind");
        ComboBox::from_label("Model")
            .selected_text(wind.model.label())
            .show_ui(ui, |ui| {
                for model in WindModel::ALL {
                    ui.selectable_value(&mut wind.model, model, model.label());
                }
            });
        ui.add(Slider::new(&mut wind.density, 4.0..=30.0).text("arrows"));
    }

    ui.separator();
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
