use std::borrow::Cow;

use wgpu::naga::ShaderStage;

pub fn load_glsl(code: &str, stage: ShaderStage) -> wgpu::ShaderSource<'_> {
    wgpu::ShaderSource::Glsl {
        shader: Cow::Borrowed(code),
        stage,
        defines: &[],
    }
}
