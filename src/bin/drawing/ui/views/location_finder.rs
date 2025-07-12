use egui::Ui;

use crate::app_state::AppState;

pub struct LocationFinderWindow {
    open: bool,
}

impl LocationFinderWindow {
    pub fn new(open: bool) -> Self {
        Self { open }
    }

    pub fn ui(&mut self, ui: &mut Ui, app_state: &mut AppState) {
        ui.label("Center Coordinates");
        ui.text_edit_singleline(&mut app_state.ui.loaction_finder.input);

        if ui.button("Find").clicked() {
            let split: Result<Vec<f32>, _> = app_state
                .ui
                .loaction_finder
                .input
                .split(' ')
                .map(|s| s.parse::<f32>())
                .collect();
            if let Ok(split) = split {
                if split.len() == 2 {
                    app_state.set_center((split[0], split[1]));
                }
            }
        }
    }
}
