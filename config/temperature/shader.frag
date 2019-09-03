#version 450


layout(location = 0) in vec4 vertexPos;
layout(location = 0) out vec4 outColor;

layout(set = 0, binding = 0) uniform texture2D t_Color;
layout(set = 0, binding = 1) uniform sampler s_Color;

vec3 hsv2rgb(vec3 c) {
    vec4 K = vec4(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    vec3 p = abs(fract(c.xxx + K.xyz) * 6.0 - K.www);
    return c.z * mix(K.xxx, clamp(p - K.xxx, 0.0, 1.0), c.y);
}

void main() {
    float v = (texture(sampler2D(t_Color, s_Color), vertexPos.xy / 2 + 0.5).x + 1) / 2;
    outColor = vec4(hsv2rgb(vec3(mix(0.15, 0, v), 1, 1)), 0.3);
}