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
        use crate::drawing::layer::RenderMode;
        ui.horizontal(|ui| {
            ui.label("Render");
            ui.selectable_value(&mut wind.mode, RenderMode::Arrows, "arrows");
            let grib = wind.model.uses_grib();
            ui.add_enabled_ui(grib, |ui| {
                ui.selectable_value(&mut wind.mode, RenderMode::Particles, "particles");
            });
            // fall back to arrows if the model can't do particles
            if !grib {
                wind.mode = RenderMode::Arrows;
            }
        });
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
