use egui::Modifiers;

use crate::app_state::AppState;

pub struct LocationFinderWindow {
    open: bool,
}

impl LocationFinderWindow {
    pub fn new(open: bool) -> Self {
        Self { open }
    }

    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        let mut request_focus = false;
        if ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, egui::Key::F)) {
            self.open = true;
            request_focus = true;
        }

        if self.open {
            let mut close = false;
            let mut valid = false;
            let width = 600.0;
            egui::Window::new("location finder")
                .default_pos([(ctx.content_rect().width() - width) / 2.0, 40.0])
                .default_width(width)
                .default_height(100.0)
                .open(&mut self.open)
                .collapsible(false)
                .title_bar(false)
                .resizable(false)
                .show(ctx, |ui| {
                    ui.label("Center Coordinates");
                    let input = ui.text_edit_singleline(&mut app_state.ui.loaction_finder.input);
                    if request_focus {
                        input.request_focus();
                    }

                    let split: Result<Vec<f32>, _> = app_state
                        .ui
                        .loaction_finder
                        .input
                        .split(' ')
                        .map(|s| s.parse::<f32>())
                        .collect();
                    if let Ok(split) = split {
                        if split.len() == 2 {
                            valid = true;
                            app_state.set_center((split[0], split[1]));
                        } else {
                            valid = false
                        }
                    }

                    if valid && ui.input_mut(|i| i.consume_key(Modifiers::NONE, egui::Key::Enter)) {
                        close = true;
                    }
                });

            if close {
                self.open = false;
            }
        }
    }
}
