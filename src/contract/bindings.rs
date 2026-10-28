pub const CONTRACT_VERSION: u32 = 1;
pub const STATIC_MAGIC: u32 = u32::from_le_bytes(*b"NSTC");
pub const DYNAMIC_MAGIC: u32 = u32::from_le_bytes(*b"NDYN");
pub const BVH_LEAF_BIT: u32 = 1 << 31;
pub const BVH_MAX_DEPTH: usize = 48;
pub const BVH_SHADER_STACK: usize = 64;
pub const MAX_MATERIALS: usize = 2048;
