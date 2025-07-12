use egui::{Frame, Ui};
use egui_extras::{Column, TableBuilder};

use crate::app_state::AppState;

pub fn view_stats(ui: &mut Ui, app_state: &mut AppState) {
    Frame::default().outer_margin(5.0).show(ui, |ui| {
        TableBuilder::new(ui)
            .column(Column::auto().resizable(true))
            .column(Column::remainder())
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.label("key");
                });
                header.col(|ui| {
                    ui.label("value");
                });
            })
            .body(|body| {
                let tile_stats = app_state.tile_cache.get_stats();
                let data = [
                    ("cached tiles", format!("{}", tile_stats.cached_tiles)),
                    ("loading tiles", format!("{}", tile_stats.loading_tiles)),
                    ("cached objects", format!("{}", tile_stats.cached_objects)),
                    ("cached features", format!("{}", tile_stats.cached_features)),
                    ("cached vertices", format!("{}", tile_stats.cached_vertices)),
                    ("objects", format!("{}", tile_stats.total_stats.objects)),
                    ("features", format!("{}", tile_stats.total_stats.features)),
                    ("vertices", format!("{}", tile_stats.total_stats.vertices)),
                    ("indices", format!("{}", tile_stats.total_stats.indices)),
                    (
                        "size",
                        human_bytes::human_bytes(tile_stats.total_stats.size as f64),
                    ),
                ];
                body.rows(20.0, data.len(), |mut row| {
                    let (key, value) = &data[row.index()];
                    row.col(|ui| {
                        ui.label(*key);
                    });
                    row.col(|ui| {
                        ui.label(value);
                    });
                });
            });
    });
}
