#version 450

#include "header/math.frag"

layout(std140, set = 1, binding = 0) uniform Material {
    UvOffset uv_offset;
    float alpha_cutoff;
};

layout(set = 1, binding = 1) uniform sampler2DArray albedo;
layout(set = 1, binding = 2) uniform usampler2D splat;

layout(location = 0) in VertexData {
    vec3 position;
    vec2 tex_coord;
    vec4 color;
} vertex;

layout(location = 0) out vec4 out_color;

void main() {
    uvec4 splat_data = texture(splat, tex_coords(vertex.tex_coord, uv_offset));
    vec4 albedo1 = texture(albedo, vec3(tex_coords(vertex.tex_coord*100.0, uv_offset), splat_data.r));
    vec4 albedo2 = texture(albedo, vec3(tex_coords(vertex.tex_coord*100.0, uv_offset), splat_data.g));
    vec4 albedo = mix(albedo1, albedo2, splat_data.b/255.0);
    if(albedo.w < alpha_cutoff) discard;
    out_color = albedo * vertex.color;
}
