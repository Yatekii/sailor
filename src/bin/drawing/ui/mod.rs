pub mod state;
pub mod views;
pub mod widgets;

use std::sync::Arc;

use egui::color_picker::Alpha;
use egui::vec2;
use egui::Color32;
use egui::FontDefinitions;
use egui::Layout;
use egui::Rgba;
use egui::ScrollArea;
use egui::Style;
use egui::Ui;
use egui::Visuals;
use egui::WidgetText;
use egui_wgpu_backend::RenderPass;
use egui_wgpu_backend::ScreenDescriptor;
use egui_winit_platform::PlatformDescriptor;
use osm::css::CSSValue;
use osm::css::Color;
use osm::css::Number;
use osm::css::Rule;
use views::fps::FpsGraph;
use views::location_finder::LocationFinderWindow;
use views::stats::StatsWindow;
use wgpu::SurfaceConfiguration;
use widgets::tabs::Pane;
use widgets::tabs::TabsBehavior;

use crate::app_state::AppState;
use crate::app_state::EditableObject;

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
            physical_width: size.width,
            physical_height: size.height,
            scale_factor: window.scale_factor(),
            font_definitions: FontDefinitions::default(),
            style: Default::default(),
        });

        let style = Style {
            visuals: Visuals {
                override_text_color: Some(Color32::WHITE),
                ..Visuals::dark()
            },
            ..Style::default()
        };
        platform.context().set_style(style);

        // We use the egui_wgpu_backend crate as the render backend.
        let rpass = RenderPass::new(device, surface_config.format, 1);

        let ui = HudUi {
            stats_window: StatsWindow::new(true),
            location_finder_window: LocationFinderWindow::new(true),
            fps_graph: FpsGraph { open: true },
            side_panel: SidePanel::new(),
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

        self.platform.context().set_pixels_per_point(4.0);
        // Begin to draw the UI frame.
        self.platform.begin_pass();

        // Draw the demo application.
        self.ui.ui(&self.platform.context(), app_state);

        // End the UI frame. We could now handle the output and draw the UI with the backend.
        let full_output = self.platform.end_pass(Some(window));
        let paint_jobs = self.platform.context().tessellate(
            full_output.shapes,
            self.platform.context().pixels_per_point(),
        );

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

    pub fn interact(&mut self, event: &winit::event::WindowEvent) -> bool {
        self.platform.handle_event(event);
        self.platform.captures_event(event) || self.platform.context().is_pointer_over_area()
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

fn add_display_none(ui: &mut Ui, rule: &mut Rule, label: &str) {
    let attribute = "display";
    let mut value = if let Some(CSSValue::String(value)) = rule.kvs.get(attribute) {
        !matches!(&value[..], "none")
    } else {
        true
    };

    ui.checkbox(&mut value, WidgetText::from(label));

    if !value {
        rule.kvs
            .insert(attribute.to_string(), CSSValue::String("none".to_string()));
    } else {
        rule.kvs.remove(attribute);
    }
}

struct HudUi {
    stats_window: StatsWindow,
    location_finder_window: LocationFinderWindow,
    fps_graph: FpsGraph,
    side_panel: SidePanel,
}

impl HudUi {
    pub fn ui(&mut self, ctx: &egui::Context, app_state: &mut AppState) {
        {
            let pointer_position = ctx.input(|i| i.pointer.hover_pos()).unwrap_or_default();

            // Draw menubar.
            egui::TopBottomPanel::top("Main Menu Bar").show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // Take full width and fixed height:
                    let height = ui.spacing().interact_size.y;
                    ui.style_mut().spacing.button_padding = vec2(2.0, 0.0);
                    ui.set_min_size(vec2(ui.available_width(), height));
                    ui.menu_button("File", |ui| {
                        if ui.button("Quit").clicked() {
                            ui.close_menu();
                        }
                    });
                    ui.with_layout(
                        Layout::from_main_dir_and_cross_align(
                            egui::Direction::LeftToRight,
                            egui::Align::Center,
                        )
                        .with_cross_justify(false)
                        .with_main_justify(false)
                        .with_main_align(egui::Align::Center),
                        |ui| {},
                    );
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        self.location_finder_window.ui(ui, app_state);

                        ui.label(format!(
                            "Frametime {:.2?} at zoom {:.2}",
                            app_state.stats.get_average(),
                            app_state.zoom
                        ));

                        ui.label(format!(
                            "Mouse Position: ({:.1},{:.1})",
                            pointer_position[0], pointer_position[1]
                        ));
                    })
                })
            });

            self.stats_window.ui(ctx, app_state);

            self.fps_graph.ui(ctx, app_state);

            self.side_panel.ui(ctx, app_state);
        }
    }
}

struct SidePanel {
    open: bool,
    tree: egui_tiles::Tree<Pane>,
}

impl SidePanel {
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

        Self { open: true, tree }
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
                view_layer_toggle(ui, app_state);
            });
    }
}

impl Default for SidePanel {
    fn default() -> Self {
        Self::new()
    }
}

fn view_layer_toggle(ui: &mut Ui, app_state: &mut AppState) {
    ui.heading("Layers");
    ScrollArea::vertical().max_height(800.0).show(ui, |ui| {
        let feature_collection = app_state.feature_collection();
        let mut features = feature_collection.write().unwrap();
        let layers = features.layers_mut();
        for layer in layers {
            ui.checkbox(&mut layer.display, &layer.name);
        }
    });
}

fn view_inspector(ui: &mut Ui, app_state: &mut AppState) {
    ui.heading("Inspector");
    ScrollArea::vertical().max_height(800.0).show(ui, |ui| {
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
                        add_display_none(ui, rule, "display");
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
