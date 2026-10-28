// Software ray tracing pass. Concatenate common.wgsl before this file.
// Packed offsets are measured in u32 words. The host must bind valid headers.
struct RayFrame {
    inv_view_proj: mat4x4<f32>,
    camera: vec4<f32>,
    frame_index: u32,
    _pad: vec3<u32>,
};
@group(0) @binding(0) var<uniform> frame: RayFrame;
@group(0) @binding(1) var<storage, read> scene: array<u32>;
@group(0) @binding(2) var<storage, read> dynamic: array<u32>;
@group(0) @binding(3) var<storage, read> sign_rects: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read> material_states: array<vec4<f32>>;
@group(1) @binding(0) var normals: texture_2d<f32>;
@group(1) @binding(1) var depths: texture_depth_2d;
@group(1) @binding(2) var sign_atlas: texture_2d<f32>;
@group(1) @binding(3) var sign_sampler: sampler;
@group(2) @binding(0) var reflection_out: texture_storage_2d<rgba16float, write>;
@group(2) @binding(1) var auxiliary_out: texture_storage_2d<rgba16float, write>;

struct Hit { t: f32, normal: vec3<f32>, material: u32, kind: u32, uv: vec2<f32> };
fn sf(i: u32) -> f32 { return bitcast<f32>(scene[i]); }
fn df(i: u32) -> f32 { return bitcast<f32>(dynamic[i]); }
fn sv3(i: u32) -> vec3<f32> { return vec3<f32>(sf(i), sf(i+1u), sf(i+2u)); }
fn dm(i: u32) -> mat4x4<f32> {
    return mat4x4<f32>(vec4<f32>(df(i),df(i+1u),df(i+2u),df(i+3u)),
        vec4<f32>(df(i+4u),df(i+5u),df(i+6u),df(i+7u)),
        vec4<f32>(df(i+8u),df(i+9u),df(i+10u),df(i+11u)),
        vec4<f32>(df(i+12u),df(i+13u),df(i+14u),df(i+15u)));
}
fn section_ok(offset: u32, count: u32, stride: u32, size: u32) -> bool {
    return offset <= size && count <= (size - offset) / stride;
}
fn box_hit(o: vec3<f32>, d: vec3<f32>, lo: vec3<f32>, hi: vec3<f32>, limit: f32) -> f32 {
    var near = 0.0;
    var far = limit;
    for (var axis = 0u; axis < 3u; axis++) {
        if (abs(d[axis]) < 1e-8) {
            if (o[axis] < lo[axis] || o[axis] > hi[axis]) { return -1.0; }
        } else {
            let a = (lo[axis] - o[axis]) / d[axis];
            let b = (hi[axis] - o[axis]) / d[axis];
            near = max(near, min(a, b));
            far = min(far, max(a, b));
            if (near > far) { return -1.0; }
        }
    }
    return near;
}
fn triangle_hit(o: vec3<f32>, d: vec3<f32>, i: u32, limit: f32, double_sided: bool) -> vec3<f32> {
    let p = sv3(i); let e1 = sv3(i+4u)-p; let e2 = sv3(i+8u)-p;
    let h = cross(d, e2); let det = dot(e1,h);
    if (abs(det) < 1e-8 || (!double_sided && det <= 0.0)) { return vec3<f32>(-1.0); }
    let inv = 1.0/det; let s = o-p; let u = dot(s,h)*inv;
    let q = cross(s,e1); let v = dot(d,q)*inv; let t = dot(e2,q)*inv;
    if (u < 0.0 || v < 0.0 || u+v > 1.0 || t <= 0.001 || t >= limit) { return vec3<f32>(-1.0); }
    return vec3<f32>(t,u,v);
}
fn trace(o: vec3<f32>, d: vec3<f32>, limit: f32, occlusion: bool) -> Hit {
    var result = Hit(limit, vec3<f32>(0.0,1.0,0.0), 0u, 0u, vec2<f32>(0.0));
    let size = arrayLength(&scene);
    if (size >= 16u) {
        let no = scene[4u]; let nc = scene[5u]; let to = scene[6u]; let tc = scene[7u];
        if (nc > 0u && section_ok(no,nc,8u,size) && section_ok(to,tc,32u,size)) {
            var stack: array<u32,64>; var distances: array<f32,64>; var sp = 1u; stack[0] = 0u;
            distances[0] = box_hit(o,d,sv3(no),sv3(no+4u),result.t);
            // A hard cap prevents malformed cyclic trees from hanging the GPU.
            var visits = 0u;
            loop {
                if (sp == 0u || visits >= min(nc * 2u, 200000u)) { break; }
                sp--; let node = stack[sp]; let entry = distances[sp]; visits++;
                if (node >= nc || entry < 0.0 || entry > result.t) { continue; }
                let n = no + node*8u;
                let right = scene[n+7u]; let first = scene[n+3u];
                if ((right & 0x80000000u) != 0u) {
                    let count = right & 0x7fffffffu;
                    if (first > tc || count > tc-first) { continue; }
                    for (var j=0u; j<count; j++) {
                        let tri = to+(first+j)*32u; let flags = scene[tri+31u];
                        if (occlusion && (flags & 4u) != 0u) { continue; }
                        let hit = triangle_hit(o,d,tri,result.t,(flags & 2u) != 0u);
                        if (hit.x > 0.0) {
                            let normal = sv3(tri+12u)*(1.0-hit.y-hit.z)+sv3(tri+16u)*hit.y+sv3(tri+20u)*hit.z;
                            let uv0=vec2<f32>(sf(tri+24u),sf(tri+25u)); let uv1=vec2<f32>(sf(tri+26u),sf(tri+27u)); let uv2=vec2<f32>(sf(tri+28u),sf(tri+29u));
                            result = Hit(hit.x, normalize(normal), scene[tri+30u], 1u, uv0*(1.0-hit.y-hit.z)+uv1*hit.y+uv2*hit.z);
                            if (occlusion) { return result; }
                        }
                    }
                } else if (sp <= 62u) {
                    var left_t = -1.0; var right_t = -1.0;
                    if (first < nc) { let child=no+first*8u; left_t=box_hit(o,d,sv3(child),sv3(child+4u),result.t); }
                    if (right < nc) { let child=no+right*8u; right_t=box_hit(o,d,sv3(child),sv3(child+4u),result.t); }
                    // LIFO: push the farther child first; cached entry distances are
                    // checked again against the nearest hit when popped.
                    if (left_t >= 0.0 && right_t >= 0.0) {
                        if (left_t <= right_t) {
                            stack[sp]=right; distances[sp]=right_t; sp++;
                            stack[sp]=first; distances[sp]=left_t; sp++;
                        } else {
                            stack[sp]=first; distances[sp]=left_t; sp++;
                            stack[sp]=right; distances[sp]=right_t; sp++;
                        }
                    } else if (left_t >= 0.0) { stack[sp]=first; distances[sp]=left_t; sp++; }
                    else if (right_t >= 0.0) { stack[sp]=right; distances[sp]=right_t; sp++; }
                }
            }
        }
    }
    let ds = arrayLength(&dynamic);
    if (ds >= 16u) {
        let off = dynamic[4u]; let count = dynamic[5u];
        if (section_ok(off,count,36u,ds)) {
            for (var j=0u; j<min(count,32u); j++) {
                let i=off+j*36u; let flags=dynamic[i+35u];
                if (occlusion && (flags & 4u) != 0u) { continue; }
                let inv=dm(i+16u); let ro=(inv*vec4<f32>(o,1.0)).xyz; let rd=(inv*vec4<f32>(d,0.0)).xyz;
                var t = -1.0; var n = vec3<f32>(0.0);
                if (dynamic[i+32u] == 0u) {
                    t=box_hit(ro,rd,vec3<f32>(-0.5),vec3<f32>(0.5),result.t);
                    if (t > 0.001) {
                        let p=ro+rd*t; let a=abs(p);
                        if (a.x>=a.y && a.x>=a.z) { n=vec3<f32>(sign(p.x),0.0,0.0); }
                        else if (a.y>=a.z) { n=vec3<f32>(0.0,sign(p.y),0.0); }
                        else { n=vec3<f32>(0.0,0.0,sign(p.z)); }
                    }
                } else if (dynamic[i+32u] == 1u) {
                    let a=dot(rd,rd); let b=dot(ro,rd); let c=dot(ro,ro)-0.25; let disc=b*b-a*c;
                    if (disc>=0.0 && a>1e-12) {
                        t=(-b-sqrt(disc))/a; if (t<=0.001) { t=(-b+sqrt(disc))/a; }
                        if (t>0.001) { n=normalize(ro+rd*t); }
                    }
                }
                if (t>0.001 && t<result.t) {
                    result=Hit(t,normalize(transpose(mat3x3<f32>(inv[0].xyz,inv[1].xyz,inv[2].xyz))*n),dynamic[i+33u],2u,vec2<f32>(0.0));
                    if (occlusion) { return result; }
                }
            }
        }
    }
    return result;
}
fn hash(x: u32) -> f32 {
    var v=x; v=(v^(v>>16u))*0x7feb352du; v=(v^(v>>15u))*0x846ca68bu; v=v^(v>>16u);
    return f32(v & 0x00ffffffu)/16777216.0;
}
@compute @workgroup_size(8,8)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    let dims=textureDimensions(reflection_out);
    if (id.x>=dims.x || id.y>=dims.y) { return; }
    let full=textureDimensions(depths); let px=vec2<i32>(min((id.xy*full)/dims,full-vec2<u32>(1u)));
    let depth=textureLoad(depths,px,0); let normal_rough=textureLoad(normals,px,0); let normal=normal_rough.xyz;
    if (depth>=1.0 || dot(normal,normal)<0.01) {
        textureStore(reflection_out,vec2<i32>(id.xy),vec4<f32>(0.0));
        textureStore(auxiliary_out,vec2<i32>(id.xy),vec4<f32>(1.0,1.0,0.0,0.0)); return;
    }
    let uv=(vec2<f32>(px)+0.5)/vec2<f32>(full);
    let clip=vec4<f32>(uv.x*2.0-1.0,1.0-uv.y*2.0,depth,1.0);
    let wp=frame.inv_view_proj*clip; let world=wp.xyz/wp.w; let n=normalize(normal);
    let seed=id.x+id.y*dims.x+frame.frame_index*747796405u;
    let origin=world+n*0.002;
    let rough=clamp(normal_rough.w,0.0,1.0);
    var dir=vec3<f32>(0.0);
    var hit=Hit(200.0,vec3<f32>(0.0,1.0,0.0),0u,0u,vec2<f32>(0.0));
    if (rough<=0.55) {
        let mirror=reflect(normalize(world-frame.camera.xyz),n);
        let tangent_r=normalize(cross(select(vec3<f32>(0.0,1.0,0.0),vec3<f32>(1.0,0.0,0.0),abs(mirror.y)>0.9),mirror));
        let bitangent_r=cross(mirror,tangent_r); let angle_r=6.2831853*hash(seed+104729u);
        let spread=rough*rough*sqrt(hash(seed+130363u));
        dir=normalize(mirror+(tangent_r*cos(angle_r)+bitangent_r*sin(angle_r))*spread);
        if (dot(dir,n)>0.0) { hit=trace(origin,dir,200.0,false); }
    }
    var color=vec3<f32>(0.005,0.007,0.02);
    let size=arrayLength(&scene);
    if (hit.kind!=0u && size>=16u) {
        let mo=scene[8u]; let mc=scene[9u];
        if (hit.material<mc && section_ok(mo,mc,16u,size)) {
            let m=mo+hit.material*16u;
            var em=sv3(m+4u); var multiplier=vec4<f32>(1.0,1.0,1.0,0.0); var params=vec4<f32>(0.0);
            if (hit.material*2u+1u<arrayLength(&material_states)) { multiplier=material_states[hit.material*2u]; params=material_states[hit.material*2u+1u]; }
            let region=scene[m+13u];
            if (scene[m+12u]==5u && region<arrayLength(&sign_rects)) { em*=sign_emission(sign_atlas,sign_sampler,sign_rects[region],hit.uv,scene[m+14u],multiplier,params); }
            else { em*=multiplier.rgb; }
            color=em+sv3(m)*0.08;
            let ds=arrayLength(&dynamic);
            if (ds>=16u && section_ok(dynamic[6u],dynamic[7u],16u,ds)) {
                let hitpos=origin+dir*hit.t;
                var scores=array<f32,4>(0.0,0.0,0.0,0.0); var chosen=array<u32,4>(0u,0u,0u,0u);
                for (var k=0u;k<min(dynamic[7u],64u);k++) {
                    let l=dynamic[6u]+k*16u; let delta=vec3<f32>(df(l),df(l+1u),df(l+2u))-hitpos; let dist2=dot(delta,delta); let range=df(l+3u);
                    if (range<=0.0 || dist2<=0.000001 || dist2>=range*range || dot(hit.normal,delta)<=0.0) { continue; }
                    let rgb=vec3<f32>(df(l+8u),df(l+9u),df(l+10u));
                    let peak=max(max(rgb.x,rgb.y),rgb.z); let intensity=df(l+11u);
                    // Attenuation, cone and N.L are all at most one for valid lights.
                    if (peak>=0.0 && intensity>=0.0 && peak*intensity/(1.0+dist2)<=scores[3]) { continue; }
                    let dist=sqrt(dist2); let ld=delta/dist; var cone=1.0;
                    if (dynamic[l+12u]==1u) { let axis=vec3<f32>(df(l+4u),df(l+5u),df(l+6u)); let outer=df(l+7u); cone=smoothstep(outer,max(bitcast<f32>(dynamic[l+14u]),outer+0.001),dot(-ld,normalize(axis))); }
                    let falloff=1.0-dist/range;
                    let score=peak*intensity*falloff*falloff*cone*max(dot(hit.normal,ld),0.0)/(1.0+dist2);
                    for (var j=0u;j<4u;j++) { if (score>scores[j]) { for (var s=3u;s>j;s--) { scores[s]=scores[s-1u]; chosen[s]=chosen[s-1u]; } scores[j]=score; chosen[j]=k; break; } }
                }
                for (var j=0u;j<4u;j++) { if (scores[j]>0.0) { let l=dynamic[6u]+chosen[j]*16u; let rgb=vec3<f32>(df(l+8u),df(l+9u),df(l+10u)); color+=sv3(m)*rgb/max(max(max(rgb.x,rgb.y),rgb.z),0.0001)*scores[j]; } }
            }
        }
    }
    let r1=hash(seed); let r2=hash(seed+2891336453u); let angle=6.2831853*r1;
    let tangent=normalize(cross(select(vec3<f32>(0.0,1.0,0.0),vec3<f32>(1.0,0.0,0.0),abs(n.y)>0.9),n));
    let bitangent=cross(n,tangent); let radius=sqrt(r2);
    let ao_dir=tangent*cos(angle)*radius+bitangent*sin(angle)*radius+n*sqrt(1.0-r2);
    let ao=select(1.0,0.0,trace(origin,ao_dir,2.0,true).kind!=0u);
    textureStore(reflection_out,vec2<i32>(id.xy),vec4<f32>(color,f32(hit.kind)));
    textureStore(auxiliary_out,vec2<i32>(id.xy),vec4<f32>(ao,1.0,1.0,0.0));
}
