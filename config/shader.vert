#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in uint layer_id;

layout(location = 0) out vec4 outColor;

struct LayerData {
    vec4 background_color;
};

layout(set = 0, binding = 0) uniform Locals {
    vec2 pan;
    vec2 _unused;
    vec2 zoom;
    LayerData layer_data[30];
};

void main() {
    gl_Position = vec4((position - pan) * zoom, 0.0, 1.0);

    outColor = layer_data[layer_id].background_color;
}