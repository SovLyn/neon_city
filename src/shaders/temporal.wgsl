struct Params { inv: mat4x4<f32>, vp: mat4x4<f32>, valid: u32, _p0: u32, _p1: u32, _p2: u32 };
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var raw_ref: texture_2d<f32>;
@group(0) @binding(2) var raw_aux: texture_2d<f32>;
@group(0) @binding(3) var motion: texture_2d<f32>;
@group(0) @binding(4) var normal: texture_2d<f32>;
@group(0) @binding(5) var depth: texture_depth_2d;
@group(0) @binding(6) var old_ref: texture_2d<f32>;
@group(0) @binding(7) var old_aux: texture_2d<f32>;
@group(0) @binding(8) var old_meta: texture_2d<f32>;
@group(0) @binding(9) var out_ref: texture_storage_2d<rgba16float, write>;
@group(0) @binding(10) var out_aux: texture_storage_2d<rgba16float, write>;
@group(0) @binding(11) var out_meta: texture_storage_2d<rgba16float, write>;
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
 let dims = textureDimensions(out_ref);
 if (any(id.xy >= dims)) { return; }
 let p = vec2<i32>(id.xy); let full = textureDimensions(depth);
 // Match the exact surface sample selected by raytrace.wgsl.
 let fp = vec2<i32>(min(id.xy * full / dims, full - vec2<u32>(1u)));
 let z = textureLoad(depth, fp, 0); let n0 = textureLoad(normal, fp, 0).xyz;
 let r = textureLoad(raw_ref, p, 0); let a = textureLoad(raw_aux, p, 0);
 if (z >= 1.0 || dot(n0,n0) < 0.01) {
   textureStore(out_ref,p,r); textureStore(out_aux,p,vec4<f32>(a.xy,0.0,-1.0)); textureStore(out_meta,p,vec4<f32>(0.0)); return;
 }
 let n = normalize(n0); let m = textureLoad(motion,fp,0);
 let uv = (vec2<f32>(fp)+0.5)/vec2<f32>(full);
 let world_h = params.inv * vec4<f32>(uv.x*2.0-1.0,1.0-uv.y*2.0,z,1.0);
 let clip = params.vp * vec4<f32>(world_h.xyz/world_h.w,1.0);
 let prev_uv = uv + m.xy;
 var color = r.rgb; var aux = a.xy; var count = 1.0;
 if (params.valid != 0u && m.z > 0.0 && all(prev_uv >= vec2<f32>(0.0)) && all(prev_uv < vec2<f32>(1.0))) {
   let hp = vec2<i32>(min(vec2<u32>(prev_uv*vec2<f32>(dims)),dims-vec2<u32>(1u)));
   let history_meta = textureLoad(old_meta,hp,0); let ha = textureLoad(old_aux,hp,0); let hr = textureLoad(old_ref,hp,0);
   let compatible = ha.z >= 1.0 && abs(history_meta.w-m.z) <= max(0.02*abs(m.z),0.01) && dot(history_meta.xyz,n) >= 0.9 && abs(ha.w-m.w) < 0.5;
   if (compatible) {
     var lo = r.rgb; var hi = r.rgb;
     for (var y = -1; y <= 1; y++) { for (var x = -1; x <= 1; x++) {
       let q = clamp(p+vec2<i32>(x,y),vec2<i32>(0),vec2<i32>(dims)-vec2<i32>(1));
       let c = textureLoad(raw_ref,q,0).rgb; lo = min(lo,c); hi = max(hi,c);
     }}
     let limit = select(16.0,3.0,r.a > 1.5 || hr.a > 1.5);
     count = min(ha.z+1.0,limit);
     color = mix(clamp(hr.rgb,lo,hi),r.rgb,1.0/count);
     aux = mix(ha.xy,a.xy,1.0/count);
   }
 }
 textureStore(out_ref,p,vec4<f32>(color,r.a));
 textureStore(out_aux,p,vec4<f32>(aux,count,m.w));
 textureStore(out_meta,p,vec4<f32>(n,clip.w));
}
