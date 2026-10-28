@group(0) @binding(0) var accumulated_ref: texture_2d<f32>;
@group(0) @binding(1) var accumulated_aux: texture_2d<f32>;
@group(0) @binding(2) var metadata: texture_2d<f32>;
@group(0) @binding(3) var normal: texture_2d<f32>;
@group(0) @binding(4) var filtered_ref: texture_storage_2d<rgba16float, write>;
@group(0) @binding(5) var filtered_aux: texture_storage_2d<rgba16float, write>;

@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims = textureDimensions(filtered_ref);
    if (any(id.xy >= dims)) { return; }
    let p = vec2<i32>(id.xy);
    let center_ref = textureLoad(accumulated_ref, p, 0);
    let center_aux = textureLoad(accumulated_aux, p, 0);
    let center_meta = textureLoad(metadata, p, 0);
    // Invalid/background surfaces do not participate in the filter.
    if (center_aux.z < 1.0 || dot(center_meta.xyz, center_meta.xyz) < 0.5) {
        textureStore(filtered_ref, p, center_ref);
        textureStore(filtered_aux, p, center_aux);
        return;
    }
    let full = textureDimensions(normal);
    let fp = vec2<i32>(min(id.xy * full / dims, full - vec2<u32>(1u)));
    let roughness = textureLoad(normal, fp, 0).a;
    // Keep smooth puddles sharp; damp surfaces get a modestly wider footprint.
    let neighbor_strength = mix(0.08, 0.35, smoothstep(0.08, 0.32, roughness));
    var ref_sum = center_ref.rgb;
    var aux_sum = center_aux.xy;
    var ref_weight = 1.0;
    var aux_weight = 1.0;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            if (x == 0 && y == 0) { continue; }
            let q = p + vec2<i32>(x, y);
            if (any(q < vec2<i32>(0)) || any(q >= vec2<i32>(dims))) { continue; }
            let a = textureLoad(accumulated_aux, q, 0);
            let m = textureLoad(metadata, q, 0);
            if (a.z < 1.0 || abs(a.w - center_aux.w) >= 0.5 || dot(m.xyz, center_meta.xyz) < 0.9) { continue; }
            let depth_delta = abs(m.w - center_meta.w);
            let depth_limit = max(0.02 * abs(center_meta.w), 0.01);
            if (depth_delta > depth_limit) { continue; }
            let spatial = select(1.0, 0.5, x != 0 && y != 0);
            let edge = spatial * pow(max(dot(m.xyz, center_meta.xyz), 0.0), 16.0) * (1.0 - depth_delta / depth_limit);
            let r = textureLoad(accumulated_ref, q, 0);
            if (abs(r.a - center_ref.a) < 0.5) {
                let w = edge * neighbor_strength;
                ref_sum += r.rgb * w;
                ref_weight += w;
            }
            let aw = edge * 0.25;
            aux_sum += a.xy * aw;
            aux_weight += aw;
        }
    }
    textureStore(filtered_ref, p, vec4<f32>(ref_sum / ref_weight, center_ref.a));
    textureStore(filtered_aux, p, vec4<f32>(aux_sum / aux_weight, center_aux.zw));
}
