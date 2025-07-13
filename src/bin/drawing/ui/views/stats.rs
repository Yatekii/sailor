use egui::{Frame, Ui};

use crate::{app_state::AppState, drawing::ui::widgets::key_value_table::widget_key_value_table};

pub fn view_stats(ui: &mut Ui, app_state: &mut AppState) {
    Frame::default().outer_margin(5.0).show(ui, |ui| {
        let tile_stats = app_state.tile_cache.get_stats();
        let data = [
            ("mouse x", app_state.cursor().x.to_string()),
            ("mouse y", app_state.cursor().x.to_string()),
            ("cached tiles", format!("{}", tile_stats.cached_tiles)),
            ("loading tiles", format!("{}", tile_stats.loading_tiles)),
            ("objects", format!("{}", tile_stats.tile_stats.objects)),
            ("features", format!("{}", tile_stats.tile_stats.features)),
            ("vertices", format!("{}", tile_stats.tile_stats.vertices)),
            ("indices", format!("{}", tile_stats.tile_stats.indices)),
            (
                "size",
                human_bytes::human_bytes(tile_stats.tile_stats.size as f64),
            ),
        ];
        widget_key_value_table(ui, &data);
    });
}
