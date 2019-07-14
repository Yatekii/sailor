#version 450
#extension GL_AMD_gpu_shader_int16 : enable

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
    vec2 canvas_size;
    vec2 _unused;
    LayerData layer_datas[1000];
};

// std140
struct TileData {
    mat4 transform;
    float extent;
    float _unused;
    float _unused2;
    float _unused3;
};

layout(set = 0, binding = 1) uniform Transform {
    TileData tile_datas[200];
};

void main() {
    bool is_outline = (gl_InstanceIndex & 0x01) == 0;
    uint tile_id = (gl_InstanceIndex >> 1);

    LayerData layer_data = layer_datas[layer_id];
    TileData tile_data = tile_datas[tile_id];
    gl_Position = tile_data.transform * vec4(position / tile_data.extent, 0.0, 1.0);
    if(is_outline){
        gl_Position.xy += normal / canvas_size * layer_data.border_width / 2;
        outColor = layer_data.outline_color;
    } else {
        outColor = layer_data.background_color;
    }
    // outColor = vec4(normal, 0.0, 1.0);
}