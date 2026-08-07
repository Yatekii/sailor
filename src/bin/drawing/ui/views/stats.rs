use egui::{Frame, Ui};
use human_bytes::human_bytes;
use lyon::geom::point;
use osm::math::{Coord, Pixel, num2deg, world_to_tile_space};

use crate::{app_state::AppState, drawing::ui::widgets::key_value_table::widget_key_value_table};

pub fn view_stats(ui: &mut Ui, app_state: &mut AppState) {
    Frame::default().outer_margin(5.0).show(ui, |ui| {
        let tile_stats = app_state.tile_cache.get_stats(&app_state.visible_tiles);
        let z = app_state.zoom;
        let p2w = app_state.screen.pixel_to_world(z);
        let p = p2w.apply(Coord::<Pixel>::new(
            app_state.cursor().x * 2.0,
            app_state.cursor().y * 2.0,
        ));
        let latlon = num2deg(world_to_tile_space(&point(p.x(), p.y()), z.floor() as u32));
        let data = [
            ("mouse x", app_state.cursor().x.to_string()),
            ("mouse y", app_state.cursor().y.to_string()),
            ("mouse lat", latlon.y.to_string()),
            ("mouse lon", latlon.x.to_string()),
            ("cached tiles", tile_stats.cached_tiles.to_string()),
            ("visible tiles", tile_stats.visible_tiles.to_string()),
            ("loading tiles", tile_stats.loading_tiles.to_string()),
            ("objects", tile_stats.tile_stats.objects.to_string()),
            ("features", tile_stats.tile_stats.features.to_string()),
            ("vertices", tile_stats.tile_stats.vertices.to_string()),
            ("indices", tile_stats.tile_stats.indices.to_string()),
            ("size", human_bytes(tile_stats.tile_stats.size as f64)),
        ];
        widget_key_value_table(ui, &data);
    });
}
