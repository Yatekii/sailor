use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, CommandEncoder, Device, MapMode, PollType, QuerySet,
    QuerySetDescriptor, QueryType, Queue, RenderPassTimestampWrites, TextureView,
};

use crate::app_state::AppState;

pub mod map;

// Two timestamps (begin/end) for each instrumented pass: polygon, then text.
pub const GPU_TS_COUNT: u32 = 4;

/// Per-pass GPU timing via timestamp queries. Results are read back one frame
/// late without blocking, so the measured frametime stays honest. Only present
/// when the adapter supports `TIMESTAMP_QUERY`.
pub struct GpuTiming {
    query_set: QuerySet,
    resolve: Buffer,
    readback: Buffer,
    pub period: f32,
    ready: Arc<AtomicBool>,
    pub pending: bool,
}

impl GpuTiming {
    pub fn new(device: &Device, period: f32) -> Self {
        let query_set = device.create_query_set(&QuerySetDescriptor {
            label: Some("pass timestamps"),
            ty: QueryType::Timestamp,
            count: GPU_TS_COUNT,
        });
        let size = GPU_TS_COUNT as u64 * 8;
        let resolve = device.create_buffer(&BufferDescriptor {
            label: Some("ts resolve"),
            size,
            usage: BufferUsages::QUERY_RESOLVE | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&BufferDescriptor {
            label: Some("ts readback"),
            size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Self {
            query_set,
            resolve,
            readback,
            period,
            ready: Arc::new(AtomicBool::new(false)),
            pending: false,
        }
    }

    /// timestamp_writes for a pass, given its begin/end query indices.
    pub fn writes(&self, begin: u32, end: u32) -> RenderPassTimestampWrites<'_> {
        RenderPassTimestampWrites {
            query_set: &self.query_set,
            beginning_of_pass_write_index: Some(begin),
            end_of_pass_write_index: Some(end),
        }
    }

    pub fn resolve(&self, encoder: &mut CommandEncoder) {
        encoder.resolve_query_set(&self.query_set, 0..GPU_TS_COUNT, &self.resolve, 0);
        encoder.copy_buffer_to_buffer(&self.resolve, 0, &self.readback, 0, GPU_TS_COUNT as u64 * 8);
    }

    pub fn map(&mut self) {
        let ready = self.ready.clone();
        self.readback
            .slice(..)
            .map_async(MapMode::Read, move |_| {
                ready.store(true, Ordering::Release);
            });
        self.pending = true;
    }

    /// If the previous frame's readback has landed, return the raw ticks and
    /// free the buffer for reuse. Non-blocking.
    pub fn take(&mut self, device: &Device) -> Option<[u64; GPU_TS_COUNT as usize]> {
        if !self.pending {
            return None;
        }
        let _ = device.poll(PollType::Poll);
        if !self.ready.load(Ordering::Acquire) {
            return None;
        }
        let out = {
            let view = self.readback.slice(..).get_mapped_range().unwrap();
            let mut out = [0u64; GPU_TS_COUNT as usize];
            for (i, o) in out.iter_mut().enumerate() {
                let b: [u8; 8] = view[i * 8..i * 8 + 8].try_into().unwrap();
                *o = u64::from_le_bytes(b);
            }
            out
        };
        self.readback.unmap();
        self.ready.store(false, Ordering::Release);
        self.pending = false;
        Some(out)
    }
}

/// Named CPU-timing spans a layer records during a frame; drained into stats by
/// the painter (which holds `&mut AppState`).
pub type Spans = Vec<(&'static str, Duration)>;

/// Forecast time selector for time-varying layers. Empty until weather layers land;
/// present now so the Layer contract doesn't churn later.
#[derive(Clone, Copy, Default)]
pub struct ForecastTime;

/// Shared inputs a layer needs to prepare its GPU resources for the frame.
///
/// Carries `&mut AppState` and the frame encoder for now; both narrow at the
/// crate split (render must not depend on the bin's `AppState`).
pub struct LayerCtx<'a> {
    pub device: &'a Device,
    pub queue: &'a Queue,
    pub encoder: &'a mut CommandEncoder,
    pub app_state: &'a mut AppState,
    /// Physical render-target resolution (width, height).
    pub resolution: (u32, u32),
    pub spans: &'a mut Spans,
    pub time: ForecastTime,
}

/// The in-flight frame a layer records draw commands into.
pub struct FramePass<'a> {
    pub encoder: &'a mut CommandEncoder,
    /// Final surface view (resolve + text target).
    pub view: &'a TextureView,
    /// Multisample color target, when MSAA is on.
    pub msaa: Option<&'a TextureView>,
    pub depth_stencil: &'a TextureView,
    pub app_state: &'a AppState,
    pub gpu_timing: Option<&'a GpuTiming>,
    pub record_gpu: bool,
    pub spans: &'a mut Spans,
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
