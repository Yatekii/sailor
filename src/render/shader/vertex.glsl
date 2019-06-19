#version 140
in vec2 position;
uniform vec2 pan;
void main() {
    gl_Position = vec4(position + pan, 0.0, 1.0);
    gl_Position.y *= -1.0;
}