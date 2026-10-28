use super::pack::pack;
use crate::contract::{bindings::*, *};
use glam::{Mat3, Vec3};

/// Each expansion item is one triangle; each partition item is one BVH node.
/// Node partitioning and final serialization are not time-bounded.
pub struct BvhBuild {
    scene: SceneCpu,
    instance: usize,
    index: usize,
    triangles: Vec<TriangleGpu>,
    nodes: Vec<BvhNodeGpu>,
    pending: Vec<(usize, usize, usize, usize)>,
    phase: u8,
}
impl BvhBuild {
    pub fn new(scene: &SceneCpu) -> Result<Self, SceneError> {
        if scene.materials.len() > MAX_MATERIALS {
            return Err(SceneError::DataOutOfBounds("too many materials".into()));
        }
        Ok(Self {
            scene: scene.clone(),
            instance: 0,
            index: 0,
            triangles: Vec::new(),
            nodes: Vec::new(),
            pending: Vec::new(),
            phase: 0,
        })
    }
    pub fn step(&mut self, work_items: u32) -> Result<BuildPoll<PackedScene>, SceneError> {
        if self.phase == 3 {
            return Err(SceneError::Other("builder already consumed".into()));
        }
        for _ in 0..work_items {
            if self.phase == 0 {
                if self.instance == self.scene.instances.len() {
                    if !self.triangles.is_empty() {
                        self.nodes.push(BvhNodeGpu::default());
                        self.pending.push((0, 0, self.triangles.len(), 0));
                    }
                    self.phase = 1;
                    continue;
                }
                let inst = &self.scene.instances[self.instance];
                if inst.flags & 1 != 0 {
                    self.instance += 1;
                    self.index = 0;
                    continue;
                }
                let mesh = self
                    .scene
                    .meshes
                    .get(inst.mesh as usize)
                    .ok_or_else(|| SceneError::DataOutOfBounds("mesh index".into()))?;
                if inst.material as usize >= self.scene.materials.len() {
                    return Err(SceneError::DataOutOfBounds("material index".into()));
                }
                if mesh.indices.len() % 3 != 0 {
                    return Err(SceneError::DataOutOfBounds(
                        "triangle indices not divisible by three".into(),
                    ));
                }
                if self.index == mesh.indices.len() {
                    self.instance += 1;
                    self.index = 0;
                    continue;
                }
                let m = inst.world_from_local;
                let linear = Mat3::from_mat4(m);
                if !m.is_finite()
                    || m.x_axis.w != 0.0
                    || m.y_axis.w != 0.0
                    || m.z_axis.w != 0.0
                    || m.w_axis.w != 1.0
                    || linear.determinant().abs() < 1e-12
                {
                    return Err(SceneError::Other(
                        "invalid or singular instance transform".into(),
                    ));
                }
                let normal = linear.inverse().transpose();
                let mut p = [[0.0; 4]; 3];
                let mut n = [[0.0; 4]; 3];
                let mut uv = [[0.0; 2]; 3];
                for j in 0..3 {
                    let v = mesh
                        .vertices
                        .get(mesh.indices[self.index + j] as usize)
                        .ok_or_else(|| SceneError::DataOutOfBounds("vertex index".into()))?;
                    let pos = m.transform_point3(Vec3::from_array(v.position));
                    let norm = normal * Vec3::from_array(v.normal);
                    if !pos.is_finite() || !norm.is_finite() || !v.uv.iter().all(|x| x.is_finite())
                    {
                        return Err(SceneError::Other("non-finite vertex".into()));
                    }
                    p[j] = pos.extend(0.0).to_array();
                    n[j] = norm.normalize_or_zero().extend(0.0).to_array();
                    uv[j] = v.uv;
                }
                self.index += 3;
                let a = Vec3::from_array(p[0][..3].try_into().unwrap());
                let b = Vec3::from_array(p[1][..3].try_into().unwrap());
                let c = Vec3::from_array(p[2][..3].try_into().unwrap());
                if (b - a).cross(c - a).length_squared() > 1e-20 {
                    self.triangles.push(TriangleGpu {
                        p0: p[0],
                        p1: p[1],
                        p2: p[2],
                        n0: n[0],
                        n1: n[1],
                        n2: n[2],
                        uv0: uv[0],
                        uv1: uv[1],
                        uv2: uv[2],
                        material_id: inst.material,
                        flags: inst.flags,
                    });
                }
            } else if self.phase == 1 {
                if let Some((node, start, end, depth)) = self.pending.pop() {
                    let mut min = Vec3::splat(f32::INFINITY);
                    let mut max = Vec3::splat(f32::NEG_INFINITY);
                    for t in &self.triangles[start..end] {
                        for p in [t.p0, t.p1, t.p2] {
                            let v = Vec3::new(p[0], p[1], p[2]);
                            min = min.min(v);
                            max = max.max(v);
                        }
                    }
                    self.nodes[node].min = min.to_array();
                    self.nodes[node].max = max.to_array();
                    if end - start <= 8 || depth >= BVH_MAX_DEPTH {
                        if end - start >= BVH_LEAF_BIT as usize {
                            return Err(SceneError::DataOutOfBounds("leaf too large".into()));
                        }
                        self.nodes[node].left_or_first = u32::try_from(start)
                            .map_err(|_| SceneError::DataOutOfBounds("triangle offset".into()))?;
                        self.nodes[node].right_or_count = BVH_LEAF_BIT | (end - start) as u32;
                    } else {
                        let ext = max - min;
                        let axis = if ext.x >= ext.y && ext.x >= ext.z {
                            0
                        } else if ext.y >= ext.z {
                            1
                        } else {
                            2
                        };
                        let mid = start + (end - start) / 2;
                        self.triangles[start..end].select_nth_unstable_by(mid - start, |a, b| {
                            let centroid = |t: &TriangleGpu| {
                                t.p0[axis] as f64 + t.p1[axis] as f64 + t.p2[axis] as f64
                            };
                            centroid(a).total_cmp(&centroid(b))
                        });
                        let left = self.nodes.len();
                        if left + 1 >= BVH_LEAF_BIT as usize {
                            return Err(SceneError::DataOutOfBounds("node index".into()));
                        }
                        self.nodes
                            .extend([BvhNodeGpu::default(), BvhNodeGpu::default()]);
                        self.nodes[node].left_or_first = left as u32;
                        self.nodes[node].right_or_count = (left + 1) as u32;
                        self.pending.push((left + 1, mid, end, depth + 1));
                        self.pending.push((left, start, mid, depth + 1));
                    }
                } else {
                    self.phase = 2;
                }
            } else if self.phase == 2 {
                let packed = pack(&self.nodes, &self.triangles, &self.scene.materials)?;
                self.phase = 3;
                return Ok(BuildPoll::Ready(packed));
            }
        }
        let progress = if self.phase == 0 {
            0.5 * self.instance as f32 / self.scene.instances.len().max(1) as f32
        } else {
            0.5
        };
        Ok(BuildPoll::Pending { progress })
    }
}
