use crate::app_state::AppState;

use super::stats::view_stats;

pub fn panel_right(ui: &mut egui::Ui, app_state: &mut AppState) {
    egui::Panel::right("panel-right")
        .exact_size(300.0)
        .show(ui, |ui| {
            view_stats(ui, app_state);
        });
}
