//! Minimal winit -> egui input translation for the web.
//!
//! `egui-winit` is native-only, so on the web we accumulate egui events from the
//! winit window events ourselves. This covers pointer, scroll and modifier input,
//! which is enough to drive the debug overlay.

use egui::{Event, Modifiers, PointerButton, Pos2, RawInput, Rect, ViewportId, vec2};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::window::Window;

pub struct EventResponse {
    pub consumed: bool,
}

pub struct EguiState {
    ctx: egui::Context,
    events: Vec<Event>,
    pointer_pos: Pos2,
    modifiers: Modifiers,
}

impl EguiState {
    pub fn new(
        ctx: egui::Context,
        _viewport_id: ViewportId,
        _display_target: &Window,
        _native_pixels_per_point: Option<f32>,
        _theme: Option<winit::window::Theme>,
        _max_texture_side: Option<usize>,
    ) -> Self {
        Self {
            ctx,
            events: Vec::new(),
            pointer_pos: Pos2::ZERO,
            modifiers: Modifiers::default(),
        }
    }

    pub fn take_egui_input(&mut self, window: &Window) -> RawInput {
        let pixels_per_point = window.scale_factor() as f32;
        let size = window.inner_size();
        let screen_rect = Rect::from_min_size(
            Pos2::ZERO,
            vec2(
                size.width as f32 / pixels_per_point,
                size.height as f32 / pixels_per_point,
            ),
        );

        let mut raw_input = RawInput {
            screen_rect: Some(screen_rect),
            events: std::mem::take(&mut self.events),
            ..Default::default()
        };
        raw_input
            .viewports
            .entry(ViewportId::ROOT)
            .or_default()
            .native_pixels_per_point = Some(pixels_per_point);
        raw_input
    }

    pub fn handle_platform_output(&mut self, _window: &Window, _output: egui::PlatformOutput) {}

    pub fn on_window_event(&mut self, window: &Window, event: &WindowEvent) -> EventResponse {
        let pixels_per_point = window.scale_factor() as f32;
        match event {
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_pos = Pos2::new(
                    position.x as f32 / pixels_per_point,
                    position.y as f32 / pixels_per_point,
                );
                self.events.push(Event::PointerMoved(self.pointer_pos));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if let Some(button) = translate_button(*button) {
                    self.events.push(Event::PointerButton {
                        pos: self.pointer_pos,
                        button,
                        pressed: *state == ElementState::Pressed,
                        modifiers: self.modifiers,
                    });
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta = match delta {
                    MouseScrollDelta::LineDelta(x, y) => vec2(*x, *y) * 50.0,
                    MouseScrollDelta::PixelDelta(p) => {
                        vec2(p.x as f32, p.y as f32) / pixels_per_point
                    }
                };
                self.events.push(Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    delta,
                    phase: egui::TouchPhase::Move,
                    modifiers: self.modifiers,
                });
            }
            WindowEvent::CursorLeft { .. } => self.events.push(Event::PointerGone),
            WindowEvent::ModifiersChanged(new) => {
                let state = new.state();
                self.modifiers = Modifiers {
                    alt: state.alt_key(),
                    ctrl: state.control_key(),
                    shift: state.shift_key(),
                    mac_cmd: false,
                    command: state.control_key(),
                };
            }
            _ => {}
        }
        EventResponse {
            consumed: self.ctx.is_pointer_over_egui(),
        }
    }
}

fn translate_button(button: MouseButton) -> Option<PointerButton> {
    match button {
        MouseButton::Left => Some(PointerButton::Primary),
        MouseButton::Right => Some(PointerButton::Secondary),
        MouseButton::Middle => Some(PointerButton::Middle),
        _ => None,
    }
}
