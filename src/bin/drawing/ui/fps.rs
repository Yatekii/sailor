use egui::{
    containers::Frame,
    emath,
    epaint::{self, PathStroke},
    pos2, vec2, Color32, Pos2, Rect,
};

use crate::{app_state::AppState, stats::FPS_SAMPLES};

const WIDTH: f32 = 1.0;

#[derive(Default)]
pub struct FpsGraph {
    pub open: bool,
}

impl FpsGraph {
    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        egui::Window::new("FPS")
            .open(&mut self.open)
            .default_size(vec2(500.0, 20.0))
            .vscroll(false)
            .show(ctx, |ui| {
                Frame::canvas(ui.style()).show(ui, |ui| {
                    ui.ctx().request_repaint();

                    let desired_size = vec2(ui.available_width(), ui.available_height());
                    let (_id, rect) = ui.allocate_space(desired_size);

                    let to_screen = emath::RectTransform::from_to(
                        Rect::from_x_y_ranges(0.0..=1.0, -1.0..=1.0),
                        rect,
                    );

                    let mut shapes = vec![];

                    let points: Vec<Pos2> = app_state
                        .stats
                        .get_times()
                        .skip(FPS_SAMPLES - rect.width() as usize)
                        .enumerate()
                        .map(|(x, y)| {
                            to_screen
                                * pos2(
                                    // Draw all the {ui.available_width()} last frame timings.
                                    x as f32 / rect.width(),
                                    // Draw y in relation to a maximum of 20 ms.
                                    -y.as_secs_f32() * 1000.0 / 20.0,
                                )
                                + vec2(0.0, rect.height() / 2.0)
                        })
                        .collect();

                    shapes.push(epaint::Shape::line(
                        points,
                        PathStroke::new(WIDTH, Color32::WHITE),
                    ));

                    ui.painter().extend(shapes);
                });
            });
    }
}
