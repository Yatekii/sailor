use std::borrow::Cow;

#[allow(dead_code)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

pub fn load_glsl(code: &str, stage: ShaderStage) -> wgpu::ShaderModuleSource {
    let ty = match stage {
        ShaderStage::Vertex => shaderc::ShaderKind::Vertex,
        ShaderStage::Fragment => shaderc::ShaderKind::Fragment,
        ShaderStage::Compute => shaderc::ShaderKind::Compute,
    };

    let mut compiler = shaderc::Compiler::new().unwrap();
    let binary_result = compiler
        .compile_into_spirv(code, ty, "shader.glsl", "main", None)
        .unwrap();
    let binary_result = binary_result.as_binary();
    let binary_result = binary_result.to_vec();
    wgpu::ShaderModuleSource::SpirV(Cow::Owned(binary_result))
}
