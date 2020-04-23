#version 450

#include "header/math.frag"

#include "header/environment.frag"

layout(set = 1, binding = 0) uniform Material {
    UvOffset uv_offset;
    float alpha_cutoff;
};

layout(set = 1, binding = 1) uniform sampler2DArray albedo;
layout(set = 1, binding = 2) uniform sampler2D emission;
layout(set = 1, binding = 3) uniform usampler2D splat;

layout(location = 0) in VertexData {
    vec3 position;
    vec3 normal;
    vec2 tex_coord;
    vec4 color;
} vertex;

layout(location = 0) out vec4 out_color;


void main() {
    vec2 final_tex_coords   = tex_coords(vertex.tex_coord, uv_offset);
    uvec4 splat_data = texture(splat, tex_coords(vertex.tex_coord, uv_offset));
    vec4 albedo1 = texture(albedo, vec3(tex_coords(vertex.tex_coord*100.0, uv_offset), splat_data.r));
    vec4 albedo2 = texture(albedo, vec3(tex_coords(vertex.tex_coord*100.0, uv_offset), splat_data.g));
    vec4 albedo_alpha = mix(albedo1, albedo2, splat_data.b/255.0);
    float alpha             = albedo_alpha.a;
    if(alpha < alpha_cutoff) discard;

    vec3 albedo = albedo_alpha.rgb;
    vec3 emission = texture(emission, final_tex_coords).rgb;

    vec3 lighting = vec3(0.0);
    vec3 normal = normalize(vertex.normal);
    for (uint i = 0u; i < point_light_count; i++) {
        // Calculate diffuse light
        vec3 light_dir = normalize(plight[i].position - vertex.position);
        float diff = max(dot(light_dir, normal), 0.0);
        vec3 diffuse = diff * normalize(plight[i].color);
        // Calculate attenuation
        vec3 dist = plight[i].position - vertex.position;
        float dist2 = dot(dist, dist);
        float attenuation = (plight[i].intensity / dist2);
        lighting += diffuse * attenuation;
    }
    for (uint i = 0u; i < directional_light_count; i++) {
        vec3 dir = dlight[i].direction;
        float diff = max(dot(-dir, normal), 0.0);
        vec3 diffuse = diff * dlight[i].color;
        lighting += diffuse * dlight[i].intensity;
    }
    lighting += ambient_color;
    out_color = vec4(lighting * albedo + emission, alpha) * vertex.color;
}
