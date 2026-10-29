// Linear HDR throughout. Encode once at output: sRGB attachment or finish, never both.
// Debug values are also linear display values and use the same output encoding.
struct Frame {
 inv_vp: mat4x4<f32>, vp: mat4x4<f32>, camera_time: vec4<f32>,
 counts: vec4<u32>,
 effects: vec4<u32>, // bloom, atmosphere, shader sRGB encode (non-sRGB surface), material count
};
struct Light { position: vec3<f32>, range: f32, direction: vec3<f32>, cos_outer: f32, color: vec3<f32>, intensity: f32, kind: u32, source_id: u32, cos_inner_bits: u32, flags: u32 };
// Exact 64-byte MaterialGpu contract layout (including unused fields).
struct Material {
 base_color: vec3<f32>, metallic: f32, emission: vec3<f32>, roughness: f32,
 ior: f32, opacity: f32, normal_scale: f32, wetness: f32,
 kind: u32, atlas_region: u32, bloom_class: u32, flags: u32
};
struct Particle { position: vec3<f32>, age: f32, velocity: vec3<f32>, lifetime: f32, color: vec3<f32>, opacity: f32, size: f32, rotation: f32, kind_bits: u32, seed_bits: u32 };
@group(0) @binding(0) var base: texture_2d<f32>;
@group(0) @binding(1) var normals: texture_2d<f32>;
@group(0) @binding(2) var emission: texture_2d<f32>;
@group(0) @binding(3) var motion: texture_2d<f32>;
@group(0) @binding(4) var depth: texture_depth_2d;
@group(0) @binding(5) var reflected: texture_2d<f32>;
@group(0) @binding(6) var auxiliary: texture_2d<f32>;
@group(0) @binding(8) var<uniform> frame: Frame;
@group(0) @binding(9) var<storage,read> lights: array<Light>;
@group(0) @binding(10) var linear_sampler: sampler;
@group(0) @binding(11) var source: texture_2d<f32>;
@group(0) @binding(12) var hdr: texture_2d<f32>;
@group(0) @binding(13) var bloom0: texture_2d<f32>;
@group(0) @binding(14) var bloom1: texture_2d<f32>;
@group(0) @binding(15) var bloom2: texture_2d<f32>;
@group(0) @binding(17) var<storage,read> particles: array<Particle>;
@group(0) @binding(18) var<storage,read> materials: array<Material>;
struct VOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i:u32)->VOut {
 let p=array<vec2<f32>,3>(vec2<f32>(-1,-1),vec2<f32>(3,-1),vec2<f32>(-1,3)); var o:VOut;
 o.position=vec4<f32>(p[i],0,1); o.uv=vec2<f32>(p[i].x*.5+.5,.5-p[i].y*.5); return o;
}
fn coord(uv:vec2<f32>, size:vec2<u32>)->vec2<i32> { return vec2<i32>(clamp(uv*vec2<f32>(size),vec2<f32>(0),vec2<f32>(size)-1)); }
fn world(uv:vec2<f32>, z:f32)->vec3<f32> { let p=frame.inv_vp*vec4<f32>(uv.x*2-1,1-uv.y*2,z,1); return p.xyz/p.w; }
struct Lit { @location(0) color:vec4<f32>, @location(1) seed:vec4<f32> };
// Avoid undefined normalize(0), including coincident lights and opposite half vectors.
fn safe_normalize(v:vec3<f32>)->vec3<f32> { return v*inverseSqrt(max(dot(v,v),1e-12)); }
fn debug_color(uv:vec2<f32>)->vec3<f32> {
 var c=vec3<f32>(1,0,1); // Unsupported history/BVH views remain explicitly unavailable.
 switch frame.counts.y {
  case 1u:{c=textureLoad(base,coord(uv,textureDimensions(base)),0).rgb;}
  case 2u:{c=safe_normalize(textureLoad(normals,coord(uv,textureDimensions(normals)),0).xyz)*.5+.5;}
  case 3u:{c=vec3<f32>(clamp(textureLoad(normals,coord(uv,textureDimensions(normals)),0).w,.04,1.));}
  case 4u:{
   let z=textureLoad(depth,coord(uv,textureDimensions(depth)),0);
   c=vec3<f32>(1); if(z<.99999){c=vec3<f32>(length(world(uv,z)-frame.camera_time.xyz)/120.);}
  }
  case 5u:{c=vec3<f32>(textureLoad(motion,coord(uv,textureDimensions(motion)),0).xy*8.+.5,0);}
  case 6u:{c=textureLoad(reflected,coord(uv,textureDimensions(reflected)),0).rgb;}
  case 7u:{c=vec3<f32>(textureLoad(auxiliary,coord(uv,textureDimensions(auxiliary)),0).r);}
  case 8u:{c=vec3<f32>(1);} // Shadow tracing not implemented: neutral fully lit.
  default:{}
 }
 return max(c,vec3<f32>(0));
}
// Procedural shallow shop-interior approximation: world-anchored color, occluding
// frames/shelves and bounded micro-parallax only. Not traced transmission, true
// refraction, or multiple refraction; the reflection texture remains BVH traced.
fn shop_interior(w:vec3<f32>, n:vec3<f32>, v:vec3<f32>)->vec3<f32> {
 var tangent=vec3<f32>(1,0,0);
 if(abs(n.y)<.95){tangent=safe_normalize(cross(vec3<f32>(0,1,0),n));}
 let up=safe_normalize(cross(n,tangent));
 let parallax=vec2<f32>(dot(v,tangent),dot(v,up))*.12/max(abs(dot(n,v)),.3);
 let facade=vec2<f32>(dot(w,tangent),dot(w,up));
 let cell=floor(facade/vec2<f32>(2.4,2.8));
 let uv=fract((facade-parallax)/vec2<f32>(2.4,2.8));
 let edge=min(uv,1-uv);
 let opening=smoothstep(.025,.065,min(edge.x,edge.y));
 let shelf=1.-.65*(1.-smoothstep(.015,.035,abs(uv.y-.36)));
 let cabinet=1.-.4*(1.-smoothstep(.02,.045,abs(uv.x-.68)));
 let variation=.5+.5*sin(dot(cell,vec2<f32>(1.7,3.1)));
 let room=mix(vec3<f32>(.28,.15,.065),vec3<f32>(.075,.19,.26),variation);
 let ceiling=.65+.35*smoothstep(.2,.85,uv.y);
 return mix(vec3<f32>(.012,.016,.02),room*shelf*cabinet*ceiling,opening);
}
// Analytic night sky, also used for BVH ray misses (not a reflection substitute).
fn night_sky(direction:vec3<f32>)->vec3<f32> {
 let horizon=pow(1.-clamp(direction.y,0.,1.),4.);
 return mix(vec3<f32>(.012,.022,.055),vec3<f32>(.075,.035,.105),horizon);
}
@fragment fn lighting(i:VOut)->Lit {
 // Debug reads only its source; no direct lighting, fog, shafts or bloom seed work.
 if(frame.counts.y!=0u){return Lit(vec4<f32>(debug_color(i.uv),1),vec4<f32>(0));}
 let p=coord(i.uv,textureDimensions(base)); let z=textureLoad(depth,coord(i.uv,textureDimensions(depth)),0);
 let b=textureLoad(base,p,0); let nr=textureLoad(normals,p,0); let e=textureLoad(emission,p,0);
 let r=textureLoad(reflected,coord(i.uv,textureDimensions(reflected)),0).rgb;
 // Temporal aux = (traced AO, 1, history count, material ID).
 // AO is really traced in this version; shadow tracing is not implemented.
 // The remaining aux channels are NOT shadow visibility, validity bits or light IDs.
 let a=textureLoad(auxiliary,coord(i.uv,textureDimensions(auxiliary)),0);
 var o:Lit; o.seed=vec4<f32>(0); var c=night_sky(safe_normalize(world(i.uv,.9999)-frame.camera_time.xyz));
 if(z<.99999) {
  let w=world(i.uv,z); let n=safe_normalize(nr.xyz); let v=safe_normalize(frame.camera_time.xyz-w);
  let rough=clamp(nr.w,.04,1.); var f0=mix(vec3<f32>(.04),b.rgb,b.a);
  var glazing=false; var opacity=1.;
  // Read the full-resolution G3.w ID, not temporally filtered auxiliary metadata.
  let material_id=textureLoad(motion,coord(i.uv,textureDimensions(motion)),0).w;
  if(material_id>=0. && material_id<f32(min(frame.effects.w,arrayLength(&materials)))) {
   let material=materials[u32(material_id)];
   if(material.kind==4u) {
    glazing=true; opacity=clamp(material.opacity,0.,1.);
    let ior=max(material.ior,1.); let dielectric=(ior-1.)/(ior+1.);
    f0=vec3<f32>(dielectric*dielectric);
   }
  }
  let fresnel=f0+(1-f0)*pow(1.-max(dot(n,v),0.),5.);
  var surface_scale=vec3<f32>(1);
  if(glazing){surface_scale=opacity*(1-fresnel);}
  // Broad cool skylight reveals unlit architecture without lifting emissive signs.
  let sky_fill=vec3<f32>(.22,.28,.40)*(.65+.35*max(n.y,0.));
  c=b.rgb*sky_fill*clamp(a.r,0.,1.)*surface_scale;
  for(var j=0u;j<min(min(frame.counts.x,64u),arrayLength(&lights));j++) {
   let light=lights[j]; let delta=light.position-w; let dist2=dot(delta,delta);
   // Reject before sqrt, normalization, attenuation powers or BRDF work.
   if(light.range<=0. || dist2>light.range*light.range){continue;}
   let d2=max(dist2,.04); let l=delta*inverseSqrt(d2);
   var cone=1.;
   if(light.kind==1u) {
    let angle=dot(-l,safe_normalize(light.direction));
    if(angle<=light.cos_outer){continue;}
    cone=smoothstep(light.cos_outer,max(bitcast<f32>(light.cos_inner_bits),light.cos_outer+.001),angle);
   }
   let ndotl=max(dot(n,l),0.); if(ndotl<=0.){continue;}
   // Same quartic falloff, using squared distance instead of sqrt + pow.
   let range=max(light.range,.01); let ratio=d2/(range*range);
   let falloff=clamp(1.-ratio*ratio,0.,1.); let cutoff=falloff*falloff;
   let radiance=light.color*light.intensity*cutoff*cone/(1+d2);
   let h=safe_normalize(l+v); let exponent=mix(180.,3.,rough*rough);
   let spec=fresnel*pow(max(dot(n,h),0.),exponent)*(exponent+2)/6.283;
   c+=(b.rgb*(1-b.a)/3.14159*surface_scale+spec)*radiance*ndotl;
  }
  let reflected_energy=r*fresnel*(1-.65*rough);
  var surface_emission=e.rgb;
  if(glazing) {
   // Glass is not a neon emitter. Opacity covers the surface; the remaining
   // non-Fresnel energy shows a dim tinted approximate interior, never white fill.
   surface_emission=vec3<f32>(0);
   let tint=mix(vec3<f32>(1),clamp(b.rgb,vec3<f32>(0),vec3<f32>(1)),.25);
   c+=(1-opacity)*(1-fresnel)*shop_interior(w,n,v)*tint;
  }
  c+=surface_emission+reflected_energy;
  // Height-integrated low fog, capped so nearby facade detail remains visible.
  let distance=length(w-frame.camera_time.xyz); let low=exp(-max(w.y,0.)*1.6);
  if(frame.effects.y!=0u) {
  let fog=min(.38,1-exp(-distance*(.0015+.012*low)));
  c=mix(c,vec3<f32>(.045,.055,.10),fog);
  // Two finite distant volumetric cones; integrate only as far as opaque depth.
  let ray=safe_normalize(w-frame.camera_time.xyz); var shafts=vec3<f32>(0);
  for(var s=0u;s<12u;s++) {
   let t=(f32(s)+.5)*min(distance,110.)/12.; let q=frame.camera_time.xyz+ray*t;
   for(var k=0u;k<2u;k++) {
    let origin=vec3<f32>(select(-8.,8.,k==1u),1.,-48.+f32(k)*12.);
    let local=q-origin; let radius=.7+max(local.y,0.)*.12;
    let density=exp(-dot(local.xz,local.xz)/(radius*radius))*smoothstep(0.,2.,local.y)*(1-smoothstep(12.,22.,local.y));
    shafts+=select(vec3<f32>(.22,.015,.15),vec3<f32>(.015,.13,.2),k==1u)*density*.012*min(distance,110.)/12.;
   }
  }
  c+=shafts;
  }
  // Independent source-class thresholds/energies, not a global brightness lift.
  let cls=u32(round(e.a)); var threshold=1.4; var gain=.18;
  if(cls==1u){threshold=.7;gain=.42;} if(cls==2u){threshold=2.;gain=.14;}
  if(cls==3u){threshold=.9;gain=.26;} if(cls==4u){threshold=1.3;gain=.19;}
  let energy=surface_emission+reflected_energy;
  if(frame.effects.x!=0u) { o.seed=vec4<f32>(max(energy-vec3<f32>(threshold),vec3<f32>(0))*gain,1); }
 }
 o.color=vec4<f32>(max(c,vec3<f32>(0)),1); return o;
}
@fragment fn downsample(i:VOut)->@location(0) vec4<f32> {
 let t=1./vec2<f32>(textureDimensions(source)); var c=vec4<f32>(0);
 for(var y=-1;y<=1;y+=2){for(var x=-1;x<=1;x+=2){c+=textureSampleLevel(source,linear_sampler,i.uv+vec2<f32>(f32(x),f32(y))*t*.5,0.0);}} return c*.25;
}
fn blur(uv:vec2<f32>, axis:vec2<f32>)->vec4<f32> {
 let t=axis/vec2<f32>(textureDimensions(source));
 var c=textureSampleLevel(source,linear_sampler,uv,0.0)*.227027;
 c+=(textureSampleLevel(source,linear_sampler,uv+t*1.384615,0.0)+textureSampleLevel(source,linear_sampler,uv-t*1.384615,0.0))*.316216;
 c+=(textureSampleLevel(source,linear_sampler,uv+t*3.230769,0.0)+textureSampleLevel(source,linear_sampler,uv-t*3.230769,0.0))*.070270; return c;
}
@fragment fn blur_h(i:VOut)->@location(0) vec4<f32>{return blur(i.uv,vec2<f32>(1,0));}
@fragment fn blur_v(i:VOut)->@location(0) vec4<f32>{return blur(i.uv,vec2<f32>(0,1));}
fn linear_to_srgb(linear:vec3<f32>)->vec3<f32> {
 let c=max(linear,vec3<f32>(0));
 return select(1.055*pow(c,vec3<f32>(1./2.4))-.055,12.92*c,c<=vec3<f32>(.0031308));
}
@fragment fn finish(i:VOut)->@location(0) vec4<f32> {
 var c=textureSampleLevel(hdr,linear_sampler,i.uv,0.0).rgb;
 if(frame.counts.y==0u && frame.effects.x!=0u){ c+=textureSampleLevel(bloom0,linear_sampler,i.uv,0.0).rgb*.6+textureSampleLevel(bloom1,linear_sampler,i.uv,0.0).rgb*.28+textureSampleLevel(bloom2,linear_sampler,i.uv,0.0).rgb*.12; }
 if(frame.counts.y==0u || frame.counts.y==6u){c=clamp((c*(2.51*c+.03))/(c*(2.43*c+.59)+.14),vec3<f32>(0),vec3<f32>(1));}
 // Shared by final and every debug mode; an sRGB attachment encodes instead.
 if(frame.effects.z!=0u){c=linear_to_srgb(c);}
 return vec4<f32>(c,1);
}
struct POut { @builtin(position) position:vec4<f32>, @location(0) uv:vec2<f32>, @location(1) color:vec4<f32>, @location(2) @interpolate(flat) kind:u32 };
@vertex fn particle_vs(@builtin(vertex_index) vertex:u32,@builtin(instance_index) index:u32)->POut {
 let p=particles[index]; let corners=array<vec2<f32>,6>(vec2<f32>(-1,-1),vec2<f32>(1,-1),vec2<f32>(-1,1),vec2<f32>(-1,1),vec2<f32>(1,-1),vec2<f32>(1,1));
 let corner=corners[vertex]; let centre=frame.vp*vec4<f32>(p.position,1); let extent=max(p.size,.005);
 let right=normalize(vec3<f32>(frame.vp[0][0],frame.vp[1][0],frame.vp[2][0]));
 let up=normalize(vec3<f32>(frame.vp[0][1],frame.vp[1][1],frame.vp[2][1]));
 // ParticleGpu kind_bits: 0 = velocity-aligned rain, 1 = rotating steam billboard.
 // Project rain onto the camera plane so width stays perpendicular to its streak.
 let projected=vec2<f32>(dot(p.velocity,right),dot(p.velocity,up));
 var axis=vec2<f32>(0,1);
 if(dot(projected,projected)>.000001) { axis=normalize(projected); }
 var horizontal=(right*axis.y-up*axis.x)*extent;
 var vertical=(right*axis.x+up*axis.y)*.20;
 if(p.kind_bits==1u) {
  horizontal=(right*cos(p.rotation)+up*sin(p.rotation))*extent;
  vertical=(-right*sin(p.rotation)+up*cos(p.rotation))*extent;
 }
 var o:POut; o.position=frame.vp*vec4<f32>(p.position+horizontal*corner.x+vertical*corner.y,1);
 if(centre.w<=0. || frame.effects.y==0u){o.position=vec4<f32>(2,2,2,1);}
 o.uv=corner; o.kind=p.kind_bits; o.color=vec4<f32>(p.color,p.opacity); return o;
}
@fragment fn particle_fs(i:POut)->Lit {
 if(frame.effects.y==0u) { discard; }
 let z=textureLoad(depth,coord(i.position.xy/vec2<f32>(frame.counts.zw),textureDimensions(depth)),0);
 // Fixed 1280x720 HDR fragment coordinates map to independently clamped depth texels.
 // Hard depth rejection; no depth-linear soft-intersection fade is applied.
 if(i.position.z>z){discard;}
 let shape=exp(-dot(i.uv,i.uv)*3.)*(1-smoothstep(.7,1.,length(i.uv)));
 let alpha=i.color.a*shape*select(.65,.45,i.kind==1u);
 var o:Lit; o.color=vec4<f32>(i.color.rgb,alpha); o.seed=vec4<f32>(0); return o;
}
