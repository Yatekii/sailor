use egui::{Frame, Key, KeyboardShortcut, Margin, Modifiers};
use egui_extras::syntax_highlighting::{highlight, CodeTheme};

use crate::app_state::AppState;

pub fn panel_bottom(ctx: &egui::Context, app_state: &mut AppState) {
    egui::TopBottomPanel::bottom("panel-bottom")
        .resizable(true)
        .default_height(300.0)
        .show(ctx, |ui| {
            ui.input_mut(|input| {
                if input.consume_shortcut(&KeyboardShortcut::new(Modifiers::COMMAND, Key::S)) {
                    app_state.css_cache.try_save_to_file().unwrap();
                }
            });

            let theme = CodeTheme::from_memory(ui.ctx(), ui.style());
            let mut layouter = |ui: &egui::Ui, buf: &str, wrap_width: f32| {
                let mut layout_job = highlight(ui.ctx(), ui.style(), &theme, buf, "css");
                layout_job.wrap.max_width = wrap_width;
                ui.fonts(|f| f.layout_job(layout_job))
            };

            let buffer = app_state.css_cache.buffer_mut();

            Frame::default()
                .inner_margin(Margin::symmetric(0, 10))
                .show(ui, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.add(
                            egui::TextEdit::multiline(buffer)
                                .font(egui::TextStyle::Monospace) // for cursor height
                                .code_editor()
                                .desired_rows(10)
                                .desired_width(f32::INFINITY)
                                .frame(false)
                                .layouter(&mut layouter),
                        );
                    });
                });
        });
}
