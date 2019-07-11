#version 450

layout(set = 0, binding = 2) uniform texture2D t_Color;
layout(set = 0, binding = 3) uniform sampler s_Color;

layout(location = 0) out vec4 outColor;

void main() {
    // vec4 tex = textureMultisample(t_Color, ivec2(gl_FragCoord.xy));
    vec2 factors = vec2(textureSize(sampler2D(t_Color, s_Color), 0));
    vec4 tex = texture(sampler2D(t_Color, s_Color), gl_FragCoord.xy / factors);
    outColor = tex;
}