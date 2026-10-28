@group(0) @binding(0) var base: texture_2d<f32>;
@group(0) @binding(1) var normals: texture_2d<f32>;
@group(0) @binding(2) var emission: texture_2d<f32>;
@group(0) @binding(3) var reflected: texture_2d<f32>;
@group(0) @binding(4) var auxiliary: texture_2d<f32>;
struct DebugConfig { view: u32, pad0: u32, pad1: u32, pad2: u32 };
@group(0) @binding(5) var<uniform> debug: DebugConfig;
struct VOut { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) i: u32) -> VOut {
 let p=array<vec2<f32>,3>(vec2<f32>(-1.0,-1.0),vec2<f32>(3.0,-1.0),vec2<f32>(-1.0,3.0)); var o: VOut; o.position=vec4<f32>(p[i],0.0,1.0); o.uv=vec2<f32>((p[i].x+1.0)*0.5,(1.0-p[i].y)*0.5); return o;
}
@fragment fn fs(i: VOut) -> @location(0) vec4<f32> {
 let output=vec2<f32>(textureDimensions(base));
 let p=vec2<i32>(min(vec2<u32>(i.uv*output),vec2<u32>(output)-vec2<u32>(1u)));
 let b=textureLoad(base,p,0); let n=textureLoad(normals,p,0); let e=textureLoad(emission,p,0).rgb;
 let rd=textureDimensions(reflected); let rp=vec2<i32>(vec2<u32>(p)*rd/vec2<u32>(output));
 let r=textureLoad(reflected,rp,0).rgb; let ao=textureLoad(auxiliary,rp,0).r;
 if (debug.view==1u) { return vec4<f32>(b.rgb,1.0); }
 if (debug.view==2u) { return vec4<f32>(n.xyz*0.5+vec3<f32>(0.5),1.0); }
 if (debug.view==3u) { return vec4<f32>(vec3<f32>(n.w),1.0); }
 if (debug.view==4u) { return vec4<f32>(r/(vec3<f32>(1.0)+r),1.0); }
 if (debug.view==5u) { return vec4<f32>(vec3<f32>(ao),1.0); }
 let light=0.08+0.3*max(dot(n.xyz,normalize(vec3<f32>(0.3,0.8,0.2))),0.0);
 let c=b.rgb*light*ao+e+r*(1.0-n.w)*0.6;
 return vec4<f32>(c/(vec3<f32>(1.0)+c),1.0);
}
