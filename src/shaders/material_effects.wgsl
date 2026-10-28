// Shared sign atlas coverage and modulation for raster and traced hits.
fn sign_emission(atlas_tex: texture_2d<f32>, atlas_samp: sampler, rect: vec4<f32>, uv: vec2<f32>, sign_class: u32, multiplier: vec4<f32>, params: vec4<f32>) -> vec3<f32> {
 let safe_uv = clamp(uv, vec2<f32>(0.0), vec2<f32>(1.0));
 let coord = mix(rect.xy, rect.zw, safe_uv);
 let coverage = textureSampleLevel(atlas_tex, atlas_samp, coord, 0.0).a;
 var mask = vec3<f32>(coverage);
 let phase = multiplier.w;
 let fault = clamp(params.x, 0.0, 1.0);
 let scan = params.y;
 if (sign_class == 1u) {
   mask *= 0.9 + 0.1 * smoothstep(0.15, 0.8, coverage);
 } else if (sign_class == 2u) {
   let grid = fract(safe_uv * vec2<f32>(96.0, 24.0));
   let pixel = 0.78 + 0.22 * smoothstep(0.0, 0.22, min(min(grid.x,1.0-grid.x),min(grid.y,1.0-grid.y)));
   mask *= pixel * (0.94 + 0.06 * sin(phase * 3.0 + floor(safe_uv.y * 24.0)));
 } else if (sign_class == 3u) {
   mask *= 0.62 + 0.18 * sin(safe_uv.y * 180.0 - scan * 6.2831853) + 0.2 * pow(max(0.0, 1.0-abs(fract(safe_uv.y-scan)-0.5)*8.0), 2.0);
 } else if (sign_class == 4u) {
   let shift = (0.002 + 0.012*fault) * (rect.z-rect.x);
   let lo = rect.xy; let hi = rect.zw;
   let red = textureSampleLevel(atlas_tex, atlas_samp, clamp(coord+vec2<f32>(shift,0.0),lo,hi),0.0).a;
   let blue = textureSampleLevel(atlas_tex, atlas_samp, clamp(coord-vec2<f32>(shift,0.0),lo,hi),0.0).a;
   mask = mix(mask, vec3<f32>(red,coverage,blue), fault);
   mask *= 1.0 - clamp(max(fault,params.z*0.35),0.0,1.0) * (0.12 + 0.2 * (0.5 + 0.5*sin(safe_uv.y*250.0+scan*6.2831853)));
 }
 return max(mask,vec3<f32>(0.0)) * multiplier.rgb;
}
