use crate::app_state::EditableObject;
use crate::app_state::AppState;
use imgui::*;
use crate::*;

use crate::config::CONFIG;

pub struct HUD {
    platform: imgui_winit_support::WinitPlatform,
    imgui: imgui::Context,
    renderer: imgui_wgpu::Renderer,
    ruda: FontId,
}

impl HUD {
    pub fn new(
        window: &winit::window::Window,
        device: &mut wgpu::Device,
    ) -> Self {
        let hidpi_factor = window.hidpi_factor();
        let mut imgui = imgui::Context::create();
        let mut platform = imgui_winit_support::WinitPlatform::init(&mut imgui);
        platform.attach_window(imgui.io_mut(), &window, imgui_winit_support::HiDpiMode::Default);
        imgui.set_ini_filename(None);

        let font_size = (13.0 * hidpi_factor) as f32;
        imgui.io_mut().font_global_scale = (1.0 / hidpi_factor) as f32;

        imgui.fonts().add_font(&[
            imgui::FontSource::DefaultFontData {
                config: Some(imgui::FontConfig {
                    oversample_h: 1,
                    pixel_snap_h: true,
                    size_pixels: font_size,
                    ..Default::default()
                })
            }
        ]);

        use std::io::Read;
        let mut f = std::fs::File::open(&CONFIG.renderer.ui_font).expect("Could not open the UI font.");
        let mut data = vec![];
        f.read_to_end(&mut data).expect("Could not read the UI font.");

        let ruda = imgui.fonts().add_font(&[FontSource::TtfData {
            data: data.as_slice(),
            size_pixels: font_size,
            config: None,
        }]);

        let _style = imgui.style_mut();

        let renderer = imgui_wgpu::Renderer::new(
            &mut imgui,
            device,
            wgpu::TextureFormat::Bgra8Unorm,
            None,
        );

        Self {
            platform,
            imgui,
            renderer,
            ruda,
        }
    }

    pub fn paint(
        &mut self,
        app_state: &mut AppState,
        window: &winit::window::Window,
        width: f64,
        height: f64,
        hidpi_factor: f64,
        device: &mut wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
    ) {
        self.platform.prepare_frame(self.imgui.io_mut(), &window) // step 4
            .expect("Failed to prepare frame");
        let ui = self.imgui.frame();
        let ruda = ui.push_font(self.ruda);
        {
            let mouse_pos = ui.io().mouse_pos;

            // Draw menubar.
            ui.main_menu_bar(|| {
                ui.menu(im_str!("File"), true, || {
                    imgui::MenuItem::new(im_str!("Quit"))
                        .shortcut(im_str!("Ctrl + Q"))
                        .build(&ui);
                });

                ui.text(&im_str!(
                    "Mouse Position: ({:.1},{:.1})",
                    mouse_pos[0],
                    mouse_pos[1]
                ));

                ui.text(&im_str!(
                    "Frametime {:.2} at zoom {:.2}",
                    app_state.stats.get_average(),
                    app_state.zoom
                ));
            });

            let window = imgui::Window::new(im_str!("Main"));
            window
                .size([400.0, 800.0], imgui::Condition::FirstUseEver)
                .build(&ui, || {
                    let mut size = ui.window_size();
                    size[1] = 100.0;
                    let window = imgui::ChildWindow::new("Hovered objects")
                        .size(size);
                    window.build(&ui, || {
                        let objects = app_state.hovered_objects
                            .iter()
                            .map(|o| o.selector.to_string())
                            .collect::<Vec<_>>()
                            .join("\n");
                        ui.text(im_str!("{}", objects));
                    });

                    add_header_separator(&ui, im_str!("Selected objects"));

                    let mut item: i32 = 0;
                    for i in 0..app_state.selected_objects.len() {
                        if app_state.selected_objects[i].selected {
                            app_state.selected_objects[i].selected = false;
                            item = i as i32;
                        }
                    }
                    let items = app_state.selected_objects
                        .iter()
                        .map(|o| im_str!("{}", o.object.selector))
                        .collect::<Vec<_>>();
                    let mut item_refs = vec![];
                    for item in &items {
                        item_refs.push(item);
                    }
                    
                    ui.list_box(
                        im_str!("Selected objects"),
                        &mut item,
                        &item_refs,
                        5
                    );

                    add_header_separator(&ui, im_str!("Selected Object"));

                    if item >= 0 && items.len() > 0 {
                        app_state.selected_objects[item as usize].selected = true;
                    }

                    let objects = &mut app_state.selected_objects;

                    if let Some(EditableObject {
                        object,
                        ..
                    }) = objects.iter_mut().find(|object| object.selected) {
                        ui.separator();
                        ui.text(im_str!("Tags"));
                        ui.separator();

                        ui.text(im_str!("{:#?}", object.tags));

                        ui.separator();
                        ui.text(im_str!("Applying rules"));
                        ui.separator();

                        let mut rules = app_state.css_cache.get_matching_rules_mut(&object.selector);
                        for rule in rules.iter_mut() {
                            let show_block = add_header_separator(&ui, im_str!("{}", rule.selector));
                            if show_block {
                                add_color_picker(&ui, rule, "background-color");
                                add_color_picker(&ui, rule, "border-color");
                                add_slider_float(&ui, rule, "border-width");
                                add_slider_float(&ui, rule, "line-width");
                                add_display_none(&ui, rule);
                            }
                        }
                    } else {
                        ui.separator();
                        ui.text(im_str!("No Object selected"));
                        ui.separator();
                    }
                });
            ruda.pop(&ui);
            // ui.show_demo_window(&mut false);
        }

        self.platform.prepare_render(&ui, &window);
        self.renderer
            .render(ui, width, height, hidpi_factor, device, encoder, &view)
            .expect("Rendering failed");
    }

    pub fn interact(&mut self, window: &winit::window::Window, event: &winit::event::Event<()>) -> (bool, bool) {
        self.platform.handle_event(self.imgui.io_mut(), window, &event);
        let io = self.imgui.io();
        (!io.want_capture_mouse, !io.want_capture_keyboard)
    }
}

fn add_color_picker(ui: &Ui, rule: &mut Rule, attribute: &str) {
    let default_color = CSSValue::Color(Color::TRANSPARENT);
    let color = if let Some(color) = rule.kvs.get(attribute) {
        color
    } else {
        &default_color
    };
    let color = match color {
        CSSValue::String(string) => {
            match &string[..] {
                "red" => Color::RED,
                "green" => Color::GREEN,
                "blue" => Color::BLUE,
                "black" => Color::BLACK,
                "white" => Color::WHITE,
                _ => Color::TRANSPARENT,
            }
        },
        CSSValue::Color(color) => {
            color.clone()
        },
        _ => Color::TRANSPARENT, // This should never happen, but transparent should be a decent fallback
    };
    let mut color = [
        color.r,
        color.g, 
        color.b,
        color.a
    ];
    let label = im_str!("{}", attribute);
    let cp = ColorEdit::new(&label, EditableColor::Float4(&mut color));
    cp
        .build(&ui);

    rule.kvs.insert(attribute.to_string(), CSSValue::Color(Color {
        r: color[0],
        g: color[1],
        b: color[2],
        a: color[3],
    }));
}

fn add_slider_float(ui: &Ui, rule: &mut Rule, attribute: &str) {
    let default_number = CSSValue::Number(Number::Px(0.0));
    let value = if let Some(value) = rule.kvs.get(attribute) {
        value
    } else {
        &default_number
    };
    let mut value = match value {
        CSSValue::Number(number) => {
            match &number {
                Number::Px(px) => *px,
                _ => 0.0,
            }
        },
        _ => 0.0, // This should never happen, but transparent should be a decent fallback
    };

    let label = im_str!("{}", attribute);
    let cp = imgui::Slider::new(&label, 0.0..=10.0);
    cp
        .build(&ui, &mut value);

    rule.kvs.insert(attribute.to_string(), CSSValue::Number(Number::Px(value)));
}

fn add_display_none(ui: &Ui, rule: &mut Rule) {
    let attribute = "display";
    let mut value = if let Some(value) = rule.kvs.get(attribute) {
        match value {
            CSSValue::String(value) => {
                match &value[..] {
                    "none" => false,
                    _ => true,
                }
            },
            _ => true,
        }
    } else {
        true
    };

    let label = im_str!("{}", attribute);
    ui.checkbox(&label, &mut value);

    if !value {
        rule.kvs.insert(attribute.to_string(), CSSValue::String("none".to_string()));
    } else {
        rule.kvs.remove(attribute);
    }
}

fn add_header_separator(ui: &Ui, title: impl Into<ImString>) -> bool {
    CollapsingHeader::new(&ui, &title.into())
        .flags(
            ImGuiTreeNodeFlags::Bullet
            | ImGuiTreeNodeFlags::Leaf
            | ImGuiTreeNodeFlags::Framed
            | ImGuiTreeNodeFlags::Selected
            | ImGuiTreeNodeFlags::NoTreePushOnOpen
            | ImGuiTreeNodeFlags::DefaultOpen
        )
        .build()
}