use egui::{Frame, Ui};

use crate::app_state::AppState;

pub type ShowFn = Box<dyn Fn(&mut Ui, &mut AppState)>;

pub struct Pane {
    pub name: &'static str,
    pub show: ShowFn,
}

pub struct TabsBehavior<'a> {
    pub(crate) app_state: &'a mut AppState,
}

impl<'a> egui_tiles::Behavior<Pane> for TabsBehavior<'a> {
    fn tab_title_for_pane(&mut self, pane: &Pane) -> egui::WidgetText {
        pane.name.into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        pane: &mut Pane,
    ) -> egui_tiles::UiResponse {
        Frame::new()
            .outer_margin(5.0)
            .show(ui, |ui| (pane.show)(ui, self.app_state));
        egui_tiles::UiResponse::None
    }
}
