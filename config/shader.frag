#version 450

layout(location = 0) in vec4 inColor;

layout(set = 0, binding = 2) uniform texture2D t_Color;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = vec4(1.0, 0.0, 0.0, 1.0);
    outColor = inColor;
}