use super::{FramePass, Layer, LayerCtx};

/// Wind overlay. Inert stub: renders nothing until the raw-data path (GFS 10m
/// U/V, particle flow) lands.
#[derive(Default)]
pub struct WindLayer {
    visible: bool,
}

impl Layer for WindLayer {
    fn name(&self) -> &str {
        "wind"
    }

    fn visible(&self) -> bool {
        self.visible
    }

    fn update(&mut self, _ctx: &mut LayerCtx) {}

    fn paint(&self, _frame: &mut FramePass) {}
}
