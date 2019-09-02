#version 450

layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 1) uniform texture2D t_Color;
layout(set = 0, binding = 2) uniform sampler s_Color;

void main() {
    outColor = vec4(1.0);//texture(sampler2D(t_Color, s_Color), outColor.xy / 2 + 0.5);
}