#version 450
#extension GL_AMD_gpu_shader_int16 : enable

layout(location = 0) in ivec2 position;
layout(location = 1) in vec2 normal;
layout(location = 2) in uint layer_id;

layout(location = 0) out vec4 outColor;

struct LayerData {
    vec4 background_color;
    vec4 outline_color;
    float border_width;
    uint line_width;
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
    // Are we handling an outline?
    bool is_outline = (gl_InstanceIndex & 0x01) == 0;
    // The current tile.
    uint tile_id = (gl_InstanceIndex >> 1);

    // Shortcut the array indexing.
    LayerData layer_data = layer_datas[layer_id];
    TileData tile_data = tile_datas[tile_id];

    // Is the line we are currently handling sized in world coordinates or pixels?
    bool is_world_scale_line = (layer_data.line_width & 0x01) == 1;

    // Calculate the tile normal normal in [0.0, 1.0] coordinates.
    vec2 local_normal = normal / tile_data.extent;

    vec4 tile_local_position = vec4(position / tile_data.extent, 0.0, 1.0);

    // If we have a world scale line, add the normal to the vertex before the world transform.
    // if(is_world_scale_line) {
    //     vec2 n = local_normal / tile_data.extent * (layer_data.line_width >> 1);
    //     tile_local_position.xy += n;
    // }

    // Transform the vertex.
    gl_Position = tile_data.transform * tile_local_position;

    // If we have a pixel scale line, add the normal to the vertex after the world transform.
    if(!is_world_scale_line) {
        gl_Position.xy += local_normal / canvas_size * (layer_data.line_width >> 1);
    }

    // If we handle an outline, add the normal to the vertex (always pixel space) and pick the appropriate color.
    if(is_outline){
        gl_Position.xy += local_normal / canvas_size * layer_data.border_width * 2;
        outColor = layer_data.outline_color;
    } else {
        outColor = layer_data.background_color;
    }
}