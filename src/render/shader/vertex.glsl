#version 140
in vec2 position;
void main() {
    gl_Position = vec4(position, 0.0, 1.0);
    gl_Position.y *= -1.0;
}