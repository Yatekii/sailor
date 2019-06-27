#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 normal;
layout(location = 2) in uint layer_id;

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
    bool is_outline = (layer_id & 1) == 1;
    gl_Position = vec4((position - pan) * zoom, 0.0, 1.0);
    if(is_outline){
        gl_Position.xy += normal / 300.0;
    }

    outColor = layer_data[layer_id >> 1].background_color;
    // outColor = vec4(normal, 0.0, 1.0);
}