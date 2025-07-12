use crate::app_state::AppState;

use super::stats::view_stats;

pub fn panel_right(ctx: &egui::Context, app_state: &mut AppState) {
    egui::SidePanel::right("panel-right")
        .exact_width(300.0)
        .show(ctx, |ui| {
            view_stats(ui, app_state);
        });
}
