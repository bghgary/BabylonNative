use std::collections::HashMap;

use crate::blocks::{block_light, light_opaque};
use crate::constants::{CHUNK_CELLS, CHUNK_SX, CHUNK_SZ, ChunkKey, WORLD_H, block_index};

pub const MAX_LIGHT: u8 = 15;
const RW: i32 = CHUNK_SX * 3;
const RD: i32 = CHUNK_SZ * 3;
const REGION_CELLS: usize = (RW * RD * WORLD_H) as usize;

/// Raw blocks are the only input to a light flood; neighbouring light is never read.
pub trait LightSource {
    fn any_emitters(&self) -> bool;
    fn ensure_chunk(&mut self, cx: i32, cz: i32);
    /// Called only after `ensure_chunk` for this key; return `CHUNK_CELLS` bytes.
    fn chunk_blocks(&self, cx: i32, cz: i32) -> &[u8];
}

fn ri(rx: i32, ry: i32, rz: i32) -> usize {
    ((ry * RD + rz) * RW + rx) as usize
}

/// Cached two-channel light, with reused scratch buffers and frame flood budget.
pub struct WorldLight {
    cache: HashMap<ChunkKey, Vec<u8>>,
    sky: Vec<u8>,
    blk: Vec<u8>,
    floor: Vec<i16>,
    q: Vec<i32>,
    qb: Vec<i32>,
    frame_computes: usize,
}

impl Default for WorldLight {
    fn default() -> Self {
        Self::new()
    }
}

impl WorldLight {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
            sky: vec![0; REGION_CELLS],
            blk: vec![0; REGION_CELLS],
            floor: vec![0; (RW * RD) as usize],
            q: vec![0; 1 << 16],
            qb: vec![0; 1 << 12],
            frame_computes: 0,
        }
    }

    pub fn begin_frame(&mut self) {
        self.frame_computes = 0;
    }

    pub fn warm_for(
        &mut self,
        source: &mut impl LightSource,
        cx: i32,
        cz: i32,
        max_computes: f64,
    ) -> bool {
        for dz in -1..=1 {
            for dx in -1..=1 {
                let key = (cx + dx, cz + dz);
                if self.cache.contains_key(&key) {
                    continue;
                }
                if self.frame_computes as f64 >= max_computes {
                    return false;
                }
                let out = self.compute(source, cx + dx, cz + dz);
                self.cache.insert(key, out);
                self.frame_computes += 1;
            }
        }
        true
    }

    /// Above the world returns full sky (240); below it returns zero.
    pub fn get_packed(&mut self, source: &mut impl LightSource, wx: i32, wy: i32, wz: i32) -> u8 {
        if wy >= WORLD_H {
            return MAX_LIGHT << 4;
        }
        if wy < 0 {
            return 0;
        }
        let cx = wx.div_euclid(CHUNK_SX);
        let cz = wz.div_euclid(CHUNK_SZ);
        let arr = self.ensure(source, cx, cz);
        arr[block_index(wx.rem_euclid(CHUNK_SX), wy, wz.rem_euclid(CHUNK_SZ))]
    }

    pub fn invalidate(&mut self, cx: i32, cz: i32) {
        self.cache.remove(&(cx, cz));
    }

    /// Clear only cached results, retaining the TypeScript scratch-buffer state.
    pub fn clear(&mut self) {
        self.cache.clear();
    }

    pub fn invalidate_around(&mut self, cx: i32, cz: i32) {
        for dz in -1..=1 {
            for dx in -1..=1 {
                self.cache.remove(&(cx + dx, cz + dz));
            }
        }
    }

    /// Interior edits retain neighbour caches; seam edits also drop touching neighbours.
    pub fn invalidate_edit(&mut self, wx: i32, wz: i32) {
        let cx = wx.div_euclid(CHUNK_SX);
        let cz = wz.div_euclid(CHUNK_SZ);
        self.cache.remove(&(cx, cz));
        let lx = wx.rem_euclid(CHUNK_SX);
        let lz = wz.rem_euclid(CHUNK_SZ);
        let nx = if lx == 0 {
            -1
        } else if lx == CHUNK_SX - 1 {
            1
        } else {
            0
        };
        let nz = if lz == 0 {
            -1
        } else if lz == CHUNK_SZ - 1 {
            1
        } else {
            0
        };
        if nx != 0 {
            self.cache.remove(&(cx + nx, cz));
        }
        if nz != 0 {
            self.cache.remove(&(cx, cz + nz));
        }
        if nx != 0 && nz != 0 {
            self.cache.remove(&(cx + nx, cz + nz));
        }
    }

    fn ensure(&mut self, source: &mut impl LightSource, cx: i32, cz: i32) -> &[u8] {
        let key = (cx, cz);
        if !self.cache.contains_key(&key) {
            let out = self.compute(source, cx, cz);
            self.cache.insert(key, out);
        }
        &self.cache[&key]
    }

    fn compute(&mut self, source: &mut impl LightSource, cx: i32, cz: i32) -> Vec<u8> {
        let has_emitters = source.any_emitters();
        self.sky.fill(0);
        if has_emitters {
            self.blk.fill(0);
        }
        for gz in 0..3 {
            for gx in 0..3 {
                source.ensure_chunk(cx - 1 + gx, cz - 1 + gz);
            }
        }
        let region: [&[u8]; 9] = std::array::from_fn(|i| {
            source.chunk_blocks(cx - 1 + (i % 3) as i32, cz - 1 + (i / 3) as i32)
        });
        let block_at = |rx: i32, ry: i32, rz: i32| {
            let gx = rx >> 4;
            let gz = rz >> 4;
            region[(gz * 3 + gx) as usize][block_index(rx & 15, ry, rz & 15)]
        };

        for rz in 0..RD {
            for rx in 0..RW {
                let mut f = 0;
                for ry in (0..WORLD_H).rev() {
                    if light_opaque(block_at(rx, ry, rz)) {
                        f = ry + 1;
                        break;
                    }
                }
                self.floor[(rz * RW + rx) as usize] = f as i16;
            }
        }

        let mut sky_head = 0;
        for rz in 0..RD {
            for rx in 0..RW {
                let base = (rz * RW + rx) as usize;
                let f = self.floor[base];
                let fw = if rx > 0 { self.floor[base - 1] } else { f };
                let fe = if rx < RW - 1 { self.floor[base + 1] } else { f };
                let fnorth = if rz > 0 {
                    self.floor[base - RW as usize]
                } else {
                    f
                };
                let fs = if rz < RD - 1 {
                    self.floor[base + RW as usize]
                } else {
                    f
                };
                let max_adj = fw.max(fe).max(fnorth).max(fs);
                for ry in (i32::from(f)..WORLD_H).rev() {
                    let i = ri(rx, ry, rz);
                    self.sky[i] = MAX_LIGHT;
                    if ry < i32::from(max_adj) {
                        sky_head = push(&mut self.q, sky_head, i);
                    }
                }
            }
        }

        let mut blk_head = 0;
        if has_emitters {
            for rz in 0..RD {
                for rx in 0..RW {
                    for ry in (0..WORLD_H).rev() {
                        let e = block_light(block_at(rx, ry, rz));
                        if e > 0 {
                            let i = ri(rx, ry, rz);
                            self.blk[i] = e;
                            blk_head = push(&mut self.qb, blk_head, i);
                        }
                    }
                }
            }
        }

        bfs(&mut self.sky, &mut self.q, sky_head, &block_at, true);
        if has_emitters {
            bfs(&mut self.blk, &mut self.qb, blk_head, &block_at, false);
        }

        let mut out = vec![0; CHUNK_CELLS];
        for y in 0..WORLD_H {
            for lz in 0..CHUNK_SZ {
                for lx in 0..CHUNK_SX {
                    let i = ri(lx + CHUNK_SX, y, lz + CHUNK_SZ);
                    out[block_index(lx, y, lz)] = (self.sky[i] << 4) | self.blk[i];
                }
            }
        }
        out
    }
}

fn push(q: &mut Vec<i32>, head: usize, value: usize) -> usize {
    if head >= q.len() {
        q.resize(q.len() * 2, 0);
    }
    q[head] = value as i32;
    head + 1
}

fn spread(
    buf: &mut [u8],
    q: &mut Vec<i32>,
    block_at: &impl Fn(i32, i32, i32) -> u8,
    cell: [i32; 3],
    level: u8,
    tail: usize,
) -> usize {
    if level == 0 {
        return tail;
    }
    let [rx, ry, rz] = cell;
    if light_opaque(block_at(rx, ry, rz)) {
        return tail;
    }
    let i = ri(rx, ry, rz);
    if buf[i] >= level {
        return tail;
    }
    buf[i] = level;
    push(q, tail, i)
}

fn bfs(
    buf: &mut [u8],
    q: &mut Vec<i32>,
    mut tail: usize,
    block_at: &impl Fn(i32, i32, i32) -> u8,
    vertical: bool,
) {
    let mut head = 0;
    while head < tail {
        let i = q[head];
        head += 1;
        let level = buf[i as usize];
        if level <= 1 {
            continue;
        }
        let rx = i % RW;
        let t = i / RW;
        let rz = t % RD;
        let ry = t / RD;
        let n = level - 1;
        if ry > 0 {
            let down = if vertical && level == MAX_LIGHT {
                level
            } else {
                n
            };
            tail = spread(buf, q, block_at, [rx, ry - 1, rz], down, tail);
        }
        if ry < WORLD_H - 1 {
            tail = spread(buf, q, block_at, [rx, ry + 1, rz], n, tail);
        }
        if rx > 0 {
            tail = spread(buf, q, block_at, [rx - 1, ry, rz], n, tail);
        }
        if rx < RW - 1 {
            tail = spread(buf, q, block_at, [rx + 1, ry, rz], n, tail);
        }
        if rz > 0 {
            tail = spread(buf, q, block_at, [rx, ry, rz - 1], n, tail);
        }
        if rz < RD - 1 {
            tail = spread(buf, q, block_at, [rx, ry, rz + 1], n, tail);
        }
    }
}
