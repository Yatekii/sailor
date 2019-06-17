#version 150
uniform vec3 layer_color;
out vec4 color;
void main() {
    color = vec4(0.0, 0.0, layer_color.z, 1.0);
}