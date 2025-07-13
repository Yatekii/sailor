use crate::{
    app_state::AppState,
    drawing::ui::widgets::tabs::{Pane, TabsBehavior},
};

use super::{inspector::view_inspector, layer_toggle::view_layer_toggle};

pub struct PanelLeft {
    _open: bool,
    tree: egui_tiles::Tree<Pane>,
}

impl PanelLeft {
    pub fn new() -> Self {
        let mut tiles = egui_tiles::Tiles::default();

        let tabs = vec![
            tiles.insert_pane(Pane {
                name: "Inspector",
                show: Box::new(|ui, app_state| {
                    view_inspector(ui, app_state);
                }),
            }),
            tiles.insert_pane(Pane {
                name: "Layers",
                show: Box::new(|ui, app_state| {
                    view_layer_toggle(ui, app_state);
                }),
            }),
        ];

        let root = tiles.insert_tab_tile(tabs);

        let tree = egui_tiles::Tree::new("my_tree", root, tiles);

        Self { _open: true, tree }
    }

    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        egui::SidePanel::left("sidepanel")
            .default_width(300.0)
            .min_width(300.0)
            .show_separator_line(false)
            .exact_width(300.0)
            .max_width(300.0)
            .show(ctx, |ui| {
                let mut behavior = TabsBehavior { app_state };
                self.tree.ui(&mut behavior, ui);
            });
    }
}

impl Default for PanelLeft {
    fn default() -> Self {
        Self::new()
    }
}
