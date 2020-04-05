#version 450

layout(set = 1, binding = 0) uniform sampler2D tex;

layout(location = 0) in vec2 vert_out_tex_coords;
layout(location = 1) in vec4 vert_out_color;
layout(location = 2) flat in uint vert_out_tex_type;

layout(location = 0) out vec4 out_color;

const uint TEX_TYPE_GENERAL   = 0x00;
const uint TEX_TYPE_GLYPH     = 0x01;

void main() {
    vec4 color;

    if (vert_out_tex_type == TEX_TYPE_GLYPH)
        color = vec4(1.0, 1.0, 1.0, texture(tex, vert_out_tex_coords).r) * vert_out_color;
    else
        color = texture(tex, vert_out_tex_coords) * vert_out_color;

    if (color.a == 0.0) {
        discard;
    }

    out_color = color;
}
