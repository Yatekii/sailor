#version 450

layout(location = 0) in vec2 position;
layout(location = 0) out vec4 outColor;

// out gl_PerVertex {
//     vec4 gl_Position;
// };

const vec2 positions[3] = vec2[3](
    vec2(0.0, -0.5),
    vec2(0.5, 0.5),
    vec2(-0.5, 0.5)
);

void main() {
    // gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    // outColor = gl_Position;
    // gl_Position = vec4(position * 256, 0.0, 1.0);
    gl_Position = vec4((position - vec2(0.52508926, 0.3486519)) * 256, 0.0, 1.0);
    gl_Position.xy -= vec2(1.0);
    gl_Position.y *= -1.0;
}