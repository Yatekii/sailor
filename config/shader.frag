#version 450

layout(location = 0) in vec4 inColor;
layout(location = 1) in float d;

layout(location = 0) out vec4 outColor;

void main() {
    outColor = inColor;
    outColor.a = outColor.a * 1-abs(d);
}