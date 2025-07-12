use crate::app_state::AppState;

pub fn panel_left(ctx: &egui::Context, app_state: &mut AppState) {
    egui::SidePanel::left("panel-left")
        .exact_width(300.0)
        .show(ctx, |ui| {});
}
