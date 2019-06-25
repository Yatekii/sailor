#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in uint layer_id;

layout(location = 0) out vec4 outColor;

struct LayerData {
    vec4 background_color;
};

layout(set = 0, binding = 0) uniform Locals {
    vec2 pan;
    LayerData layer_data[30];
};

void main() {
    // gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    // outColor = vec4(pan, 0.0, 1.0);
    // gl_Position = vec4(position * 256, 0.0, 1.0);
    gl_Position = vec4((position - pan) * 256, 0.0, 1.0);
    gl_Position.xy -= vec2(1.0);
}