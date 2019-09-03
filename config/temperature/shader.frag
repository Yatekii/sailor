#version 450


layout(location = 0) in vec4 vertexPos;
layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 0) uniform texture2D t_Color;
layout(set = 0, binding = 1) uniform sampler s_Color;

void main() {
    outColor = texture(sampler2D(t_Color, s_Color), vertexPos.xy / 2 + 0.5);
}