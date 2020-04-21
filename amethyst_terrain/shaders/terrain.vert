#version 450

layout(std140, set = 0, binding = 0) uniform CustomUniformArgs {
    uniform float scale;
};

layout(location = 0) in vec3 position;
layout(location = 1) in vec2 tex_coord;

layout(location = 0) out vec2 vert_out_tex_coord;


void main() {
    vert_out_tex_coord = tex_coord;
    gl_Position = vec4(position, 1.0);
}
