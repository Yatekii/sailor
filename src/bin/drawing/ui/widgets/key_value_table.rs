use egui::{Frame, Ui};
use egui_extras::{Column, TableBuilder};

pub fn widget_key_value_table(ui: &mut Ui, data: &[(&str, String)]) {
    Frame::default().outer_margin(5.0).show(ui, |ui| {
        let width = ui.available_width() / 2.0;
        TableBuilder::new(ui)
            .column(Column::exact(width).resizable(true))
            .column(Column::remainder())
            .striped(true)
            .header(20.0, |mut header| {
                header.col(|ui| {
                    ui.label("key");
                });
                header.col(|ui| {
                    ui.label("value");
                });
            })
            .body(|body| {
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
