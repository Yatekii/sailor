use std::borrow::Cow;

use wgpu::naga::{FastHashMap, ShaderStage};

pub fn load_glsl(code: &str, stage: ShaderStage) -> wgpu::ShaderSource {
    wgpu::ShaderSource::Glsl {
        shader: Cow::Borrowed(code),
        stage,
        defines: FastHashMap::default(),
    }
}
