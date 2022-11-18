mod state;
use std::sync::Arc;

use egui::color_picker::Alpha;
use egui::FontDefinitions;
use egui::Rgba;
use egui::Ui;
use egui::WidgetText;
use egui_wgpu_backend::RenderPass;
use egui_wgpu_backend::ScreenDescriptor;
use egui_winit_platform::PlatformDescriptor;
pub use state::*;
use wgpu::SurfaceConfiguration;

use crate::app_state::AppState;
use crate::app_state::EditableObject;
use crate::*;

use crate::config::CONFIG;

pub struct Hud {
    platform: egui_winit_platform::Platform,
    rpass: RenderPass,
    ui: HudUi,
}

impl Hud {
    pub fn new(
        window: &winit::window::Window,
        device: &mut wgpu::Device,
        surface_config: &SurfaceConfiguration,
    ) -> Self {
        // We use the egui_winit_platform crate as the platform.
        let size = window.inner_size();
        let platform = egui_winit_platform::Platform::new(PlatformDescriptor {
            physical_width: size.width as u32,
            physical_height: size.height as u32,
            scale_factor: window.scale_factor(),
            font_definitions: FontDefinitions::default(),
            style: Default::default(),
        });
        // We use the egui_wgpu_backend crate as the render backend.
        let rpass = RenderPass::new(device, surface_config.format, 1);

        let ui = HudUi {
            main_window: MainWindow { open: true },
            stats_window: StatsWindow { open: true },
            location_finder_window: LocationFinderWindow { open: true },
        };

        Self {
            platform,
            rpass,
            ui,
        }
    }

    pub fn paint(
        &mut self,
        app_state: &mut AppState,
        window: &winit::window::Window,
        device: &mut wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        frame: &wgpu::SurfaceTexture,
    ) {
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Begin to draw the UI frame.
        self.platform.begin_frame();

        // Draw the demo application.
        self.ui.ui(&self.platform.context(), app_state);

        // End the UI frame. We could now handle the output and draw the UI with the backend.
        let full_output = self.platform.end_frame(Some(window));
        let paint_jobs = self.platform.context().tessellate(full_output.shapes);

        // Upload all resources for the GPU.
        let size = window.inner_size();
        let screen_descriptor = ScreenDescriptor {
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: window.scale_factor() as f32,
        };
        let tdelta: egui::TexturesDelta = full_output.textures_delta;
        self.rpass
            .add_textures(device, queue, &tdelta)
            .expect("add texture ok");
        self.rpass
            .update_buffers(device, queue, &paint_jobs, &screen_descriptor);

        // Record all render passes.
        self.rpass
            .execute(encoder, &view, &paint_jobs, &screen_descriptor, None)
            .unwrap();
        self.rpass
            .remove_textures(tdelta)
            .expect("remove texture ok");
    }

    pub fn interact(&mut self, event: &winit::event::Event<()>) -> bool {
        self.platform.handle_event(event);
        self.platform.captures_event(event)
    }
}

fn add_color_picker(ui: &mut Ui, rule: &mut Rule, attribute: &str) {
    let default_color = CSSValue::Color(Color::TRANSPARENT);
    let color = if let Some(color) = rule.kvs.get(attribute) {
        color
    } else {
        &default_color
    };
    let color = match color {
        CSSValue::String(string) => match &string[..] {
            "red" => Color::RED,
            "green" => Color::GREEN,
            "blue" => Color::BLUE,
            "black" => Color::BLACK,
            "white" => Color::WHITE,
            _ => Color::TRANSPARENT,
        },
        CSSValue::Color(color) => color.clone(),
        _ => Color::TRANSPARENT, // This should never happen, but transparent should be a decent fallback
    };
    let mut color = Rgba::from_rgba_premultiplied(color.r, color.g, color.b, color.a);
    egui::widgets::color_picker::color_edit_button_rgba(ui, &mut color, Alpha::Opaque);

    rule.kvs.insert(
        attribute.to_string(),
        CSSValue::Color(Color {
            r: color[0],
            g: color[1],
            b: color[2],
            a: color[3],
        }),
    );
}

fn add_slider_float(ui: &mut Ui, rule: &mut Rule, attribute: &str) {
    let default_number = CSSValue::Number(Number::Px(0.0));
    let value = if let Some(value) = rule.kvs.get(attribute) {
        value
    } else {
        &default_number
    };
    let mut value = match value {
        CSSValue::Number(Number::Px(px)) => *px,
        _ => 0.0,
    };

    ui.add(egui::Slider::new(&mut value, 0.0..=10.0).text(attribute));

    rule.kvs
        .insert(attribute.to_string(), CSSValue::Number(Number::Px(value)));
}

fn add_display_none(ui: &mut Ui, rule: &mut Rule) {
    let attribute = "display";
    let mut value = if let Some(CSSValue::String(value)) = rule.kvs.get(attribute) {
        !matches!(&value[..], "none")
    } else {
        true
    };

    ui.checkbox(&mut value, WidgetText::from(attribute));

    if !value {
        rule.kvs
            .insert(attribute.to_string(), CSSValue::String("none".to_string()));
    } else {
        rule.kvs.remove(attribute);
    }
}

struct HudUi {
    main_window: MainWindow,
    stats_window: StatsWindow,
    location_finder_window: LocationFinderWindow,
}

impl HudUi {
    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        {
            let pointer_position = ctx.input().pointer.hover_pos().unwrap_or_default();

            // Draw menubar.
            egui::TopBottomPanel::top("Main Menu Bar").show(ctx, |ui| {
                egui::menu::bar(ui, |ui| {
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.close_menu();
                        }
                    });

                    ui.label(&format!(
                        "Mouse Position: ({:.1},{:.1})",
                        pointer_position[0], pointer_position[1]
                    ));

                    ui.label(&format!(
                        "Frametime {:.2} at zoom {:.2}",
                        app_state.stats.get_average(),
                        app_state.zoom
                    ));
                });
            });

            self.main_window.ui(ctx, app_state);

            self.stats_window.ui(ctx, app_state);

            self.location_finder_window.ui(ctx, app_state);
        }
    }
}

struct MainWindow {
    open: bool,
}

impl MainWindow {
    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        // Draw main window.
        egui::Window::new("Main")
            .default_width(400.0)
            .default_height(600.0)
            .open(&mut self.open)
            .show(ctx, |ui| {
                let mut size = ui.min_size();
                size[1] = 100.0;
                ui.heading("Hovered Objects");
                ui.vertical(|ui| {
                    let hovered_objects = app_state.hovered_objects.lock().unwrap();
                    let objects = hovered_objects
                        .iter()
                        .map(|o| o.selector().to_string())
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.label(objects);
                });

                let mut item: i32 = 0;
                for i in 0..app_state.selected_objects.len() {
                    if app_state.selected_objects[i].selected {
                        app_state.selected_objects[i].selected = false;
                        item = i as i32;
                    }
                }
                let items = Arc::new(
                    app_state
                        .selected_objects
                        .iter()
                        .map(|o| format!("{}", o.object.selector()))
                        .collect::<Vec<_>>(),
                );
                let items_clone = items.clone();

                ui.collapsing("Selected objects", |ui| {
                    ui.vertical(|ui| {
                        for item in &*items_clone {
                            ui.label(item);
                        }
                    })
                });

                ui.collapsing("Selected object", |ui| {
                    if item >= 0 && !items.is_empty() {
                        app_state.selected_objects[item as usize].selected = true;
                    }

                    let objects = &mut app_state.selected_objects;

                    if let Some(EditableObject { object, .. }) =
                        objects.iter_mut().find(|object| object.selected)
                    {
                        ui.separator();
                        ui.label("Tags");
                        ui.separator();

                        ui.label(format!("{:#?}", object.tags()));

                        ui.separator();
                        ui.label("Applying rules");
                        ui.separator();

                        let mut rules = app_state
                            .css_cache
                            .get_matching_rules_mut(object.selector());
                        for rule in rules.iter_mut() {
                            ui.collapsing(format!("{}", rule.selector), |ui| {
                                add_color_picker(ui, rule, "background-color");
                                add_color_picker(ui, rule, "border-color");
                                add_slider_float(ui, rule, "border-width");
                                add_slider_float(ui, rule, "line-width");
                                add_display_none(ui, rule);
                            });
                        }
                    } else {
                        ui.separator();
                        ui.label("No Object selected");
                        ui.separator();
                    }
                });
            });
    }
}

struct StatsWindow {
    open: bool,
}

impl StatsWindow {
    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        egui::Window::new("Stats")
            .default_pos([60.0, 720.0])
            .default_width(400.0)
            .default_height(200.0)
            .open(&mut self.open)
            .show(ctx, |ui| {
                // Show cache stats
                ui.collapsing("Cache Stats", |ui| {
                    ui.label(format!("{:#?}", app_state.tile_cache.get_stats()));
                });
            });
    }
}

struct LocationFinderWindow {
    open: bool,
}

impl LocationFinderWindow {
    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        egui::Window::new("Location Finder")
            .default_pos([520.0, 60.0])
            .default_width(400.0)
            .default_height(100.0)
            .open(&mut self.open)
            .show(ctx, |ui| {
                ui.label("Center Coordinates");
                ui.text_edit_singleline(&mut app_state.ui.loaction_finder.input);

                if ui.button("Find").clicked() {
                    let split: Result<Vec<f32>, _> = app_state
                        .ui
                        .loaction_finder
                        .input
                        .split(' ')
                        .map(|s| s.parse::<f32>())
                        .collect();
                    if let Ok(split) = split {
                        if split.len() == 2 {
                            app_state.set_center((split[0], split[1]));
                        }
                    }
                }
            });
    }
}
