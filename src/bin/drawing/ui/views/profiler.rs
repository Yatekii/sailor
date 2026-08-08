use egui::{Align2, Color32, FontId, Rect, Sense, Ui, pos2, vec2};

use crate::app_state::AppState;

const BUCKETS: usize = 40;
const ROW_HEIGHT: f32 = 34.0;

/// Per-value timing histograms. One row per recorded series (cpu spans and gpu
/// passes), so a change can be attributed to the exact section it moved.
pub fn view_profiler(ui: &mut Ui, app_state: &mut AppState) {
    egui::ScrollArea::vertical().show(ui, |ui| {
        for series in app_state.stats.series() {
            let avg = series.average().as_secs_f32() * 1000.0;
            let max = series.max().as_secs_f32() * 1000.0;
            ui.label(format!("{}  avg {:.2}ms  max {:.2}ms", series.name, avg, max));

            // Bucket the window into [0, max] and draw a count histogram.
            let (rect, _) =
                ui.allocate_exact_size(vec2(ui.available_width(), ROW_HEIGHT), Sense::hover());
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 2.0, Color32::from_gray(24));

            if max <= 0.0 || series.samples.is_empty() {
                continue;
            }

            let mut counts = [0u32; BUCKETS];
            for s in &series.samples {
                let ms = s.as_secs_f32() * 1000.0;
                let b = ((ms / max) * BUCKETS as f32) as usize;
                counts[b.min(BUCKETS - 1)] += 1;
            }
            let peak = counts.iter().copied().max().unwrap_or(1).max(1) as f32;

            let bar_w = rect.width() / BUCKETS as f32;
            for (i, &c) in counts.iter().enumerate() {
                if c == 0 {
                    continue;
                }
                let h = (c as f32 / peak) * rect.height();
                let x = rect.left() + i as f32 * bar_w;
                painter.rect_filled(
                    Rect::from_min_max(
                        pos2(x, rect.bottom() - h),
                        pos2(x + bar_w - 1.0, rect.bottom()),
                    ),
                    0.0,
                    Color32::from_rgb(120, 180, 255),
                );
            }

            // Axis ticks: y is frame count (0..peak), x is duration (0..max ms).
            let font = FontId::proportional(9.0);
            let ink = Color32::from_gray(150);
            painter.text(
                rect.left_top() + vec2(2.0, 0.0),
                Align2::LEFT_TOP,
                format!("{peak:.0} frames"),
                font.clone(),
                ink,
            );
            painter.text(
                rect.left_bottom() + vec2(2.0, -1.0),
                Align2::LEFT_BOTTOM,
                "0",
                font.clone(),
                ink,
            );
            painter.text(
                rect.right_bottom() + vec2(-2.0, -1.0),
                Align2::RIGHT_BOTTOM,
                format!("{max:.2}ms"),
                font,
                ink,
            );
        }
    });
}
