pub const CHUNK_SX: i32 = 16;
pub const CHUNK_SZ: i32 = 16;
pub const WORLD_H: i32 = 96;
/// Water fills up to, but not including, this Y.
pub const SEA_LEVEL: i32 = 30;
pub const CHUNK_CELLS: usize = (CHUNK_SX * CHUNK_SZ * WORLD_H) as usize;

/// Linear index `(y * CHUNK_SZ + z) * CHUNK_SX + x` for in-chunk coordinates.
pub fn block_index(x: i32, y: i32, z: i32) -> usize {
    ((y * CHUNK_SZ + z) * CHUNK_SX + x) as usize
}

pub type ChunkKey = (i32, i32);

/// Tuple replacement for the TypeScript `"cx,cz"` key.
pub fn chunk_key(cx: i32, cz: i32) -> ChunkKey {
    (cx, cz)
}
