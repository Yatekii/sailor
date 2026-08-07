use osm::math::Screen;

/// Forecast time selector for time-varying layers. Empty until weather layers land;
/// present now so the Layer contract doesn't churn later.
#[derive(Clone, Copy, Default)]
pub struct ForecastTime;

/// Shared, per-frame inputs a layer needs to prepare its GPU resources.
pub struct LayerCtx<'a> {
    pub device: &'a wgpu::Device,
    pub queue: &'a wgpu::Queue,
    pub screen: &'a Screen,
    pub zoom: f32,
    pub time: ForecastTime,
}

/// The in-flight frame a layer records draw commands into.
pub struct FramePass<'a> {
    pub encoder: &'a mut wgpu::CommandEncoder,
    pub target: &'a wgpu::TextureView,
    pub depth_stencil: &'a wgpu::TextureView,
}

/// One composable map layer. The basemap is just one of these.
pub trait Layer {
    fn name(&self) -> &str;
    fn visible(&self) -> bool;
    fn update(&mut self, ctx: &mut LayerCtx);
    fn paint(&self, frame: &mut FramePass);
}

/// Ordered stack of layers. Painted bottom-to-top in push order.
#[derive(Default)]
pub struct LayerStack {
    layers: Vec<Box<dyn Layer>>,
}

impl LayerStack {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, layer: Box<dyn Layer>) {
        self.layers.push(layer);
    }

    pub fn update_all(&mut self, ctx: &mut LayerCtx) {
        for l in &mut self.layers {
            l.update(ctx);
        }
    }

    pub fn paint_all(&self, frame: &mut FramePass) {
        for l in &self.layers {
            if l.visible() {
                l.paint(frame);
            }
        }
    }

    pub fn names_in_paint_order(&self) -> Vec<&str> {
        self.layers
            .iter()
            .filter(|l| l.visible())
            .map(|l| l.name())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Dummy {
        name: &'static str,
        visible: bool,
    }
    impl Layer for Dummy {
        fn name(&self) -> &str {
            self.name
        }
        fn visible(&self) -> bool {
            self.visible
        }
        fn update(&mut self, _ctx: &mut LayerCtx) {}
        fn paint(&self, _frame: &mut FramePass) {}
    }

    #[test]
    fn paints_visible_layers_in_push_order() {
        let mut stack = LayerStack::new();
        stack.push(Box::new(Dummy {
            name: "map",
            visible: true,
        }));
        stack.push(Box::new(Dummy {
            name: "wind",
            visible: false,
        }));
        stack.push(Box::new(Dummy {
            name: "temperature",
            visible: true,
        }));
        assert_eq!(stack.names_in_paint_order(), vec!["map", "temperature"]);
    }
}
