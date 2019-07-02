#version 450

layout(set = 0, binding = 1) uniform texture2D t_Color;
layout(set = 0, binding = 2) uniform sampler s_Color;

layout(location = 0) out vec4 outColor;

vec4 textureMultisample(sampler2DMS s, ivec2 coord)
{
    vec4 color = vec4(0.0);
    int texSamples = 8;

    float totalWeight = 0.0;
    for (int i = 0; i < texSamples; i++) {
        float weight = smoothstep(0.3, 0.7, 1.0 / abs(float(i - texSamples) / 2.0));
        color += weight *texelFetch(s, coord, i);
        totalWeight += weight;
    }
    
    return color / totalWeight;
}

void main() {
    // vec4 tex = textureMultisample(t_Color, ivec2(gl_FragCoord.xy));
    vec2 factors = vec2(textureSize(sampler2D(t_Color, s_Color), 0));
    vec4 tex = texture(sampler2D(t_Color, s_Color), gl_FragCoord.xy / factors);
    outColor = tex;
}