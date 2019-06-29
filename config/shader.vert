#version 450

layout(location = 0) in vec2 position;
layout(location = 1) in vec2 normal;
layout(location = 2) in uint layer_id;

layout(location = 0) out vec4 outColor;

struct LayerData {
    vec4 background_color;
    vec4 outline_color;
    float border_width;
};

layout(set = 0, binding = 0) uniform Locals {
    vec2 pan;
    vec2 _unused;
    vec2 zoom;
    vec2 _unused2;
    vec2 canvas_size;
    vec2 _unused3;
    mat4 transform;
    LayerData layer_datas[30];
};

void main() {
    LayerData layer_data = layer_datas[layer_id >> 1];
    bool is_outline = (layer_id & 1) == 1;
    // gl_Position = vec4((position - pan) * zoom, 0.0, 1.0);
    gl_Position = transform * vec4(position, 0.0, 1.0);
    if(is_outline){
        gl_Position.xy += normal / canvas_size * layer_data.border_width / 2;
        outColor = layer_data.outline_color;
    } else {
        outColor = layer_data.background_color;
    }

    
    // outColor = vec4(normal, 0.0, 1.0);
}