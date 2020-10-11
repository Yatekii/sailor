use std::io::Cursor;

#[allow(dead_code)]
pub enum ShaderStage {
    Vertex,
    Fragment,
    Compute,
}

pub fn load_glsl(code: &str, stage: ShaderStage) -> Vec<u32> {
    let ty = match stage {
        ShaderStage::Vertex => shaderc::ShaderKind::Vertex,
        ShaderStage::Fragment => shaderc::ShaderKind::Fragment,
        ShaderStage::Compute => shaderc::ShaderKind::Compute,
    };

    let mut compiler = shaderc::Compiler::new().unwrap();
    let binary_result = compiler
        .compile_into_spirv(code, ty, "shader.glsl", "main", None)
        .unwrap();

    let reader = Cursor::new(binary_result.as_binary_u8());
    wgpu::read_spirv(reader).unwrap()
}
