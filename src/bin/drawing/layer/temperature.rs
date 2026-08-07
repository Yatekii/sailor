use super::{FramePass, Layer, LayerCtx};

/// Temperature overlay. Inert stub: renders nothing until the real data path
/// (gridded field + colormap) lands. The old synthetic-texture experiment lives
/// in git history if its texture/colormap pipeline is worth reviving.
#[derive(Default)]
pub struct TemperatureLayer {
    visible: bool,
}

impl Layer for TemperatureLayer {
    fn name(&self) -> &str {
        "temperature"
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn update(&mut self, _ctx: &mut LayerCtx) {}

    fn paint(&self, _frame: &mut FramePass) {}
}
