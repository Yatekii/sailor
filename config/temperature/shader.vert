#version 450

out gl_PerVertex {
    vec4 gl_Position;
};

layout(location = 0) out vec4 vertexPos;

const vec2 positions[6] = vec2[6](
    vec2(1, -1),
    vec2(1, 1),
    vec2(-1, 1),
    vec2(-1, -1),
    vec2(1, -1),
    vec2(-1, 1)
);

void main() {
    gl_Position = vec4(positions[gl_VertexIndex], 0.0, 1.0);
    vertexPos = gl_Position;
}