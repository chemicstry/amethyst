#version 450

layout(std140, set = 0, binding = 0) uniform UiViewArgs {
    uniform vec2 inverse_window_size;
};

layout(location = 0) in vec2 coords;
layout(location = 1) in vec2 dimensions;
layout(location = 2) in vec4 tex_coord_bounds;
layout(location = 3) in vec4 color;
layout(location = 4) in uint tex_type;

layout(location = 0) out vec2 vert_out_tex_coords;
layout(location = 1) out vec4 vert_out_color;
layout(location = 2) flat out uint vert_out_tex_type;

const vec2 positions[4] = vec2[](
    vec2(0.5, -0.5), // Right bottom
    vec2(-0.5, -0.5), // Left bottom
    vec2(0.5, 0.5), // Right top
    vec2(-0.5, 0.5) // Left top
);

void main() {
    vec2 pos = positions[gl_VertexIndex];

    vec2 coords_base = pos + vec2(0.5);
    vert_out_tex_coords = mix(tex_coord_bounds.xy, tex_coord_bounds.zw, coords_base);
    vert_out_color = color;
    vert_out_tex_type = tex_type;

    vec2 center = coords * inverse_window_size;
    center.y = 1.0 - center.y;
    vec2 final_pos = (center + dimensions * inverse_window_size * pos) * 2.0 - vec2(1.0);

    gl_Position = vec4(final_pos, 0.0, 1.0);
}
