use crate::vector_tile::object::Object;
use crate::app_state::EditableObject;
use crate::app_state::AppState;
use imgui::*;
use crate::css::{
    CSSValue,
    Color,
    Rule,
    Number,
};

pub struct HUD {
    platform: imgui_winit_support::WinitPlatform,
    imgui: imgui::Context,
    renderer: imgui_wgpu::Renderer,
}

impl HUD {
    pub fn new(
        window: &wgpu::winit::Window,
        device: &mut wgpu::Device,
    ) -> Self {
        let hidpi_factor = window.get_hidpi_factor();
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

        let renderer = imgui_wgpu::Renderer::new(
            &mut imgui,
            device,
            wgpu::TextureFormat::Bgra8Unorm,
            None,
        )
        .expect("Failed to initialize renderer");

        Self {
            platform,
            imgui,
            renderer,
        }
    }

    pub fn paint(
        &mut self,
        app_state: &mut AppState,
        window: &wgpu::winit::Window,
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

        {
            let window = imgui::Window::new(im_str!("Hello world"));
            window
                .size([400.0, 800.0], imgui::Condition::FirstUseEver)
                .build(&ui, || {
                    let objects = app_state.hovered_objects
                        .iter()
                        .map(|o| o.selector.to_string())
                        .collect::<Vec<_>>()
                        .join("\n");
                    ui.text(im_str!("{}", objects));
                    ui.separator();
                    let mouse_pos = ui.io().mouse_pos;
                    ui.text(im_str!(
                        "Mouse Position: ({:.1},{:.1})",
                        mouse_pos[0],
                        mouse_pos[1]
                    ));
                    ui.text(im_str!(
                        "Frametime {:.2} at zoom {:.2}",
                        app_state.stats.get_average(),
                        app_state.zoom
                    ));
                });

            let window = imgui::Window::new(im_str!("Hello too"));
            window
                .size([400.0, 800.0], Condition::FirstUseEver)
                .position([520.0, 60.0], Condition::FirstUseEver)
                .build(&ui, || {
                    let mut item: i32 = 0;
                    for i in 0..app_state.selected_objects.len() {
                        if app_state.selected_objects[i].selected {
                            app_state.selected_objects[i].selected = false;
                            item = i as i32;
                        }
                    }
                    let items = app_state.selected_objects.iter().map(|o| im_str!("{}", o.object.selector)).collect::<Vec<_>>();
                    let mut item_refs = vec![];
                    for item in &items {
                        item_refs.push(item);
                    }
                    ui.text(im_str!("Hello world!"));
                    ui.list_box(
                        im_str!("hello top"),
                        &mut item,
                        &item_refs,
                        5
                    );
                    if item >= 0 && items.len() > 0 {
                        app_state.selected_objects[item as usize].selected = true;
                    }
                });

            let window = imgui::Window::new(im_str!("Edit Feature"));
            window
                .size([400.0, 800.0], Condition::FirstUseEver)
                .position([980.0, 60.0], Condition::FirstUseEver)
                .build(&ui, || {
                    let objects = &mut app_state.selected_objects;

                    if let Some(EditableObject {
                        object: Object {
                            selector,
                            ..
                        },
                        ..
                    }) = objects.iter_mut().find(|object| object.selected) {
                        let mut rules = app_state.css_cache.get_matching_rules_mut(&selector);
                        for rule in rules.iter_mut() {
                            let show_block = CollapsingHeader::new(&ui, &im_str!("{}", rule.selector)).build();
                            if show_block {
                                add_color_picker(&ui, rule, "background-color");
                                add_color_picker(&ui, rule, "border-color");
                                add_slider_float(&ui, rule, "border-width");
                                add_slider_float(&ui, rule, "line-width");
                                add_display_none(&ui, rule);
                            }
                        }
                    }
                });

            ui.show_demo_window(&mut false);
        }

        self.platform.prepare_render(&ui, &window);
        self.renderer
            .render(ui, width, height, hidpi_factor, device, encoder, &view)
            .expect("Rendering failed");
    }

    pub fn interact(&mut self, window: &wgpu::winit::Window, event: &wgpu::winit::Event) -> bool {
        self.platform.handle_event(self.imgui.io_mut(), &window, &event);
        self.imgui.io().want_capture_mouse
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
    let default_number = CSSValue::Number(Number::Px(0.0));
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