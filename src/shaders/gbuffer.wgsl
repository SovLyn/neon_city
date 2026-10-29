struct Camera { view_proj: mat4x4<f32>, prev_view_proj: mat4x4<f32>, camera: vec4<f32> };
struct MaterialState { multiplier: vec4<f32>, params: vec4<f32> };
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<storage, read> instances: array<InstanceGpu>;
@group(0) @binding(2) var<storage, read> materials: array<MaterialGpu>;
@group(0) @binding(3) var<storage, read> states: array<MaterialState>;
@group(0) @binding(4) var atlas: texture_2d<f32>;
@group(0) @binding(5) var atlas_sampler: sampler;
@group(0) @binding(6) var<storage, read> rects: array<vec4<f32>>;
struct Ripples { count: u32, _p0: u32, _p1: u32, _p2: u32, items: array<RippleGpu,128> };
@group(0) @binding(7) var<uniform> ripples: Ripples;
@group(0) @binding(8) var material_color: texture_2d_array<f32>;
@group(0) @binding(9) var material_detail: texture_2d_array<f32>;
@group(0) @binding(10) var material_sampler: sampler;
struct VOut { @builtin(position) position: vec4<f32>, @location(0) normal: vec3<f32>, @location(1) uv: vec2<f32>, @location(2) prev: vec4<f32>, @location(3) current: vec4<f32>, @location(4) @interpolate(flat) material: u32, @location(5) world: vec3<f32> };
@vertex fn vs(@location(0) position: vec3<f32>, @location(1) normal: vec3<f32>, @location(2) uv: vec2<f32>, @builtin(instance_index) index: u32) -> VOut {
 let inst=instances[index]; let p=inst.world*vec4<f32>(position,1.0); var o: VOut;
 o.position=camera.view_proj*p; o.current=o.position; o.prev=camera.prev_view_proj*inst.prev_world*vec4<f32>(position,1.0);
 o.normal=normalize(mat3x3<f32>(inst.normal_0.xyz,inst.normal_1.xyz,inst.normal_2.xyz)*normal); o.uv=uv; o.material=inst.ids.y; o.world=p.xyz; return o;
}
struct GOut { @location(0) base: vec4<f32>, @location(1) normal: vec4<f32>, @location(2) emission: vec4<f32>, @location(3) motion: vec4<f32> };
@fragment fn fs(i: VOut) -> GOut {
 let world_footprint=max(length(dpdx(i.world)),length(dpdy(i.world)));
 let m=materials[i.material];
 let surface=material_surface(i.world,normalize(i.normal),m.base_metallic.rgb,m.emission_roughness.a,m.ids.x,m.ids.w,world_footprint,material_color,material_detail,material_sampler); var emission=m.emission_roughness.rgb;
 var multiplier=vec4<f32>(1.0,1.0,1.0,0.0); var params=vec4<f32>(0.0);
 if (i.material<arrayLength(&states)) { multiplier=states[i.material].multiplier; params=states[i.material].params; }
 if (m.ids.x==5u && m.ids.y<arrayLength(&rects)) { emission*=sign_emission(atlas,atlas_sampler,rects[m.ids.y],i.uv,m.ids.z,multiplier,params); }
 else { emission*=multiplier.rgb; }
 var n=surface.normal;
 if (m.ids.x==1u || m.ids.x==2u) {
   var slope=vec2<f32>(0.0);
   if (m.ids.x==1u) {
     slope=vec2<f32>(sin(i.world.x*6.0+camera.camera.w*1.2),cos(i.world.z*7.0-camera.camera.w))*0.008;
     for (var j=0u; j<min(ripples.count,128u); j++) {
       let r=ripples.items[j]; let delta=i.world.xz-r.center_age.xz; let dist=length(delta);
       let age=r.center_age.w; let life=max(r.params.x,0.001); let wavelength=max(r.params.z,0.03);
       let ring=dist-age*r.params.w; let envelope=exp(-ring*ring/(wavelength*wavelength))*clamp(1.0-age/life,0.0,1.0);
       slope+=delta/max(dist,0.001)*cos(ring*6.2831853/wavelength)*envelope*r.params.y*0.12;
     }
   } else if (((m.ids.w>>8u)&3u)==0u) { slope=vec2<f32>(sin(i.world.x*41.0+i.world.z*13.0),cos(i.world.z*47.0-i.world.x*17.0))*0.025; }
   n=normalize(n+vec3<f32>(slope.x,0.0,slope.y));
 }
 var o: GOut; o.base=vec4<f32>(surface.albedo,m.base_metallic.a); o.normal=vec4<f32>(n,surface.roughness); o.emission=vec4<f32>(emission,f32(m.ids.z));
 let current_uv=vec2<f32>(i.current.x/i.current.w*0.5+0.5, 0.5-i.current.y/i.current.w*0.5);
 let prev_uv=vec2<f32>(i.prev.x/i.prev.w*0.5+0.5, 0.5-i.prev.y/i.prev.w*0.5);
 o.motion=vec4<f32>(prev_uv-current_uv,i.prev.w,f32(i.material)); return o;
}
