struct SurfaceMaterial { albedo: vec3<f32>, normal: vec3<f32>, roughness: f32 };

fn material_surface(
    world: vec3<f32>, geometric_normal: vec3<f32>, base: vec3<f32>, roughness: f32,
    kind: u32, flags: u32, footprint: f32,
    color_tex: texture_2d_array<f32>, detail_tex: texture_2d_array<f32>, material_sampler: sampler,
) -> SurfaceMaterial {
    var out: SurfaceMaterial;
    out.albedo = base;
    out.normal = normalize(geometric_normal);
    out.roughness = roughness;
    let material_class = (flags >> 8u) & 3u;
    if (material_class == 0u || kind == 1u || kind == 4u || kind == 5u) { return out; }

    var scale = 1.5;
    if (material_class == 2u) { scale = 2.0; }
    if (material_class == 3u) { scale = 1.25; }
    let n = normalize(geometric_normal);
    let a = abs(n);
    var t = vec3<f32>(1.0, 0.0, 0.0);
    var b = vec3<f32>(0.0, 1.0, 0.0);
    if (a.x >= a.y && a.x >= a.z) {
        t = vec3<f32>(0.0, 0.0, -sign(n.x)); b = vec3<f32>(0.0, 1.0, 0.0);
    } else if (a.y >= a.z) {
        t = vec3<f32>(1.0, 0.0, 0.0); b = vec3<f32>(0.0, 0.0, -sign(n.y));
    } else {
        t = vec3<f32>(sign(n.z), 0.0, 0.0); b = vec3<f32>(0.0, 1.0, 0.0);
    }
    let uv = vec2<f32>(dot(world, t), -dot(world, b)) / scale;
    let levels = f32(textureNumLevels(color_tex));
    let texel_size = f32(textureDimensions(color_tex, 0).x);
    let lod = clamp(log2(max(footprint / scale * texel_size, 1.0)), 0.0, max(levels - 1.0, 0.0));
    let layer = i32(material_class - 1u);
    let texel = textureSampleLevel(color_tex, material_sampler, uv, layer, lod).rgb;
    var mean = 0.27445437;
    if (material_class == 2u) { mean = 0.04534131; }
    if (material_class == 3u) { mean = 0.06869006; }
    out.albedo = base * clamp(texel / mean, vec3<f32>(0.3), vec3<f32>(2.5));

    let detail = textureSampleLevel(detail_tex, material_sampler, uv, layer, lod);
    var tn = normalize(detail.rgb * 2.0 - 1.0);
    let projected_t = normalize(t - n * dot(t, n));
    let projected_b = cross(n, projected_t);
    out.normal = normalize(n + projected_t * tn.x * 0.2 + projected_b * tn.y * 0.2);
    let map_roughness = detail.a;
    // Modulate the wet material rather than lerping toward dry source roughness:
    // otherwise most asphalt texels clamp to .45 and erase both variation and sheen.
    if (kind == 2u) { out.roughness = clamp(roughness * mix(0.65, 1.15, map_roughness), 0.2, 0.45); }
    else if (kind == 3u) { out.roughness = clamp(roughness * 0.65 + map_roughness * 0.35, 0.65, 0.9); }
    else if (material_class == 1u) { out.roughness = max(0.6, roughness * 0.7 + map_roughness * 0.3); }
    else { out.roughness = clamp(roughness * 0.7 + map_roughness * 0.3, 0.35, 0.85); }
    return out;
}
