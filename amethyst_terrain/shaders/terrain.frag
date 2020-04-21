#version 450

layout(location = 0) in vec2 vert_out_tex_coord;

layout(location = 0) out vec4 out_color;

void main() {
    out_color = vec4(vert_out_tex_coord, 1.0, 1.0);
}
