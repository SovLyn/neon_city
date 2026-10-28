use crate::contract::{bindings::*, *};
use bytemuck::Pod;

fn words<T: Pod>(data: &[T]) -> Vec<u32> {
    bytemuck::cast_slice::<T, u8>(data)
        .chunks_exact(4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .collect()
}
pub(super) fn pack(
    nodes: &[BvhNodeGpu],
    triangles: &[TriangleGpu],
    materials: &[MaterialGpu],
) -> Result<PackedScene, SceneError> {
    let rect_count = materials
        .iter()
        .map(|m| m.atlas_region as usize + 1)
        .max()
        .unwrap_or(0)
        .max(materials.len());
    // Atlas region indices are untrusted; keep allocations bounded.
    if rect_count > MAX_MATERIALS {
        return Err(SceneError::DataOutOfBounds("atlas region".into()));
    }
    let mut result = vec![0; 16];
    let mut header = StaticHeaderGpu {
        magic: STATIC_MAGIC,
        version: CONTRACT_VERSION,
        ..Default::default()
    };
    header.nodes_offset = result.len() as u32;
    header.nodes_count =
        u32::try_from(nodes.len()).map_err(|_| SceneError::DataOutOfBounds("nodes".into()))?;
    result.extend(words(nodes));
    header.triangles_offset =
        u32::try_from(result.len()).map_err(|_| SceneError::DataOutOfBounds("offset".into()))?;
    header.triangles_count = u32::try_from(triangles.len())
        .map_err(|_| SceneError::DataOutOfBounds("triangles".into()))?;
    result.extend(words(triangles));
    header.materials_offset =
        u32::try_from(result.len()).map_err(|_| SceneError::DataOutOfBounds("offset".into()))?;
    header.materials_count = materials.len() as u32;
    result.extend(words(materials));
    header.atlas_rects_offset =
        u32::try_from(result.len()).map_err(|_| SceneError::DataOutOfBounds("offset".into()))?;
    header.atlas_rects_count = rect_count as u32;
    result.extend(words(&vec![[0.0f32, 0.0, 1.0, 1.0]; rect_count]));
    header.total_words =
        u32::try_from(result.len()).map_err(|_| SceneError::DataOutOfBounds("size".into()))?;
    result[..16].copy_from_slice(&words(&[header]));
    Ok(PackedScene { words: result })
}
/// Replace atlas rectangles while preserving all preceding sections. Each material's
/// region must remain addressable; failure leaves the input unchanged.
pub fn set_atlas_rects(scene: &mut PackedScene, rects: &[[f32; 4]]) -> Result<(), SceneError> {
    let w = &scene.words;
    let bad = || SceneError::DataOutOfBounds("invalid static packed scene or atlas".into());
    if w.len() < 16
        || w[0] != STATIC_MAGIC
        || w[1] != CONTRACT_VERSION
        || w[2] as usize != w.len()
        || rects.len() > MAX_MATERIALS
    {
        return Err(bad());
    }
    let (no, nc, to, tc, mo, mc, ao, ac) = (
        w[4] as usize,
        w[5] as usize,
        w[6] as usize,
        w[7] as usize,
        w[8] as usize,
        w[9] as usize,
        w[10] as usize,
        w[11] as usize,
    );
    let end = |offset: usize, count: usize, stride: usize| {
        count
            .checked_mul(stride)
            .and_then(|n| offset.checked_add(n))
    };
    if no != 16
        || end(no, nc, 8) != Some(to)
        || end(to, tc, 32) != Some(mo)
        || end(mo, mc, 16) != Some(ao)
        || end(ao, ac, 4) != Some(w.len())
        || rects.iter().any(|r| {
            !r.iter().all(|v| v.is_finite() && *v >= 0.0 && *v <= 1.0) || r[0] > r[2] || r[1] > r[3]
        })
    {
        return Err(bad());
    }
    for i in 0..mc {
        if w[mo + i * 16 + 13] as usize >= rects.len() {
            return Err(bad());
        }
    }
    let total = ao
        .checked_add(rects.len().checked_mul(4).ok_or_else(bad)?)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(bad)?;
    let mut updated = w[..ao].to_vec();
    updated.extend(words(rects));
    updated[2] = total;
    updated[11] = rects.len() as u32;
    scene.words = updated;
    Ok(())
}
