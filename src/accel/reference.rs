use crate::contract::{BvhNodeGpu, TriangleGpu, bindings::BVH_LEAF_BIT};
use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
    pub t_min: f32,
    pub t_max: f32,
}
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub triangle: usize,
}
pub fn aabb(ray: Ray, min: Vec3, max: Vec3) -> Option<f32> {
    let (mut lo, mut hi) = (ray.t_min, ray.t_max);
    for i in 0..3 {
        if ray.direction[i] == 0.0 {
            if ray.origin[i] < min[i] || ray.origin[i] > max[i] {
                return None;
            }
        } else {
            let a = (min[i] - ray.origin[i]) / ray.direction[i];
            let b = (max[i] - ray.origin[i]) / ray.direction[i];
            lo = lo.max(a.min(b));
            hi = hi.min(a.max(b));
            if hi < lo {
                return None;
            }
        }
    }
    Some(lo)
}
pub fn triangle(ray: Ray, tri: &TriangleGpu, shadow: bool) -> Option<f32> {
    if shadow && tri.flags & 4 != 0 {
        return None;
    }
    let p = |a: [f32; 4]| Vec3::new(a[0], a[1], a[2]);
    let a = p(tri.p0);
    let e1 = p(tri.p1) - a;
    let e2 = p(tri.p2) - a;
    let h = ray.direction.cross(e2);
    let det = e1.dot(h);
    if if tri.flags & 2 != 0 {
        det.abs() < 1e-8
    } else {
        det < 1e-8
    } {
        return None;
    }
    let inv = det.recip();
    let s = ray.origin - a;
    let u = s.dot(h) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = ray.direction.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    if t >= ray.t_min && t <= ray.t_max && t.is_finite() {
        Some(t)
    } else {
        None
    }
}
pub fn brute_force(ray: Ray, triangles: &[TriangleGpu], shadow: bool) -> Option<Hit> {
    triangles
        .iter()
        .enumerate()
        .filter_map(|(i, t)| triangle(ray, t, shadow).map(|t| Hit { t, triangle: i }))
        .min_by(|a, b| a.t.total_cmp(&b.t))
}
/// Returns None for empty or malformed trees. The builder emits trees of depth <=48.
pub fn traverse(
    ray: Ray,
    nodes: &[BvhNodeGpu],
    triangles: &[TriangleGpu],
    shadow: bool,
) -> Option<Hit> {
    if nodes.is_empty() {
        return None;
    }
    let mut stack = vec![0usize];
    let mut hit: Option<Hit> = None;
    let mut visited = 0usize;
    while let Some(i) = stack.pop() {
        visited += 1;
        if visited > nodes.len() {
            return None;
        }
        let n = nodes.get(i)?;
        let mut clipped = ray;
        if let Some(h) = hit {
            clipped.t_max = h.t;
        }
        if aabb(clipped, Vec3::from_array(n.min), Vec3::from_array(n.max)).is_none() {
            continue;
        }
        if n.right_or_count & BVH_LEAF_BIT != 0 {
            let first = n.left_or_first as usize;
            let end = first.checked_add((n.right_or_count & !BVH_LEAF_BIT) as usize)?;
            for (j, t) in triangles.get(first..end)?.iter().enumerate() {
                if let Some(d) = triangle(clipped, t, shadow) {
                    if hit.is_none_or(|h| d < h.t) {
                        hit = Some(Hit {
                            t: d,
                            triangle: first + j,
                        });
                        clipped.t_max = d;
                    }
                }
            }
        } else {
            if stack.len() + 2 > 64 {
                return None;
            }
            let l = n.left_or_first as usize;
            let r = n.right_or_count as usize;
            let ln = nodes.get(l)?;
            let rn = nodes.get(r)?;
            let ld = aabb(clipped, Vec3::from_array(ln.min), Vec3::from_array(ln.max));
            let rd = aabb(clipped, Vec3::from_array(rn.min), Vec3::from_array(rn.max));
            match (ld, rd) {
                (Some(a), Some(b)) => {
                    if a <= b {
                        stack.push(r);
                        stack.push(l);
                    } else {
                        stack.push(l);
                        stack.push(r);
                    }
                }
                (Some(_), None) => stack.push(l),
                (None, Some(_)) => stack.push(r),
                _ => {}
            }
        }
    }
    hit
}
