use crate::app_state::AppState;

pub struct StatsWindow {
    open: bool,
}

impl StatsWindow {
    pub fn new(open: bool) -> Self {
        Self { open }
    }

    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        egui::Window::new("Stats")
            .default_pos([320.0, 330.0])
            .default_width(400.0)
            .default_height(230.0)
            .open(&mut self.open)
            .show(ctx, |ui| {
                // Show cache stats
                egui::CollapsingHeader::new("Cache Stats")
                    .default_open(true)
                    .show(ui, |ui| {
                        ui.label(format!("{:#?}", app_state.tile_cache.get_stats()));
                    });
            });

        egui::SidePanel::right("hehe")
            .exact_width(300.0)
            .show(ctx, |ui| {});
    }
}
