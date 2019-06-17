#version 150
uniform vec3 layer_color;
out vec4 color;
void main() {
    color = vec4(layer_color, 1.0);
}