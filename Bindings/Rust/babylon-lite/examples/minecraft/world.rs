use std::collections::HashMap;

use crate::blocks::{Block, block_light};
use crate::constants::{CHUNK_CELLS, CHUNK_SX, CHUNK_SZ, ChunkKey, WORLD_H, block_index};
use crate::world_light::{LightSource, WorldLight};
use crate::worldgen::generate_chunk;

#[derive(Clone, Debug)]
pub struct Chunk {
    pub cx: i32,
    pub cz: i32,
    pub blocks: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnPosition {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Default)]
struct ChunkEdits {
    entries: Vec<(usize, i32)>,
    positions: HashMap<usize, usize>,
}

impl ChunkEdits {
    fn set(&mut self, idx: usize, id: i32) {
        if let Some(&pos) = self.positions.get(&idx) {
            self.entries[pos].1 = id;
        } else {
            self.positions.insert(idx, self.entries.len());
            self.entries.push((idx, id));
        }
    }
}

#[derive(Default)]
struct Store {
    chunks: HashMap<ChunkKey, Chunk>,
    edits: HashMap<ChunkKey, ChunkEdits>,
    edit_order: Vec<ChunkKey>,
    any_emitters: bool,
}

impl Store {
    fn get_chunk(&mut self, seed: f64, cx: i32, cz: i32) -> &mut Chunk {
        self.chunks.entry((cx, cz)).or_insert_with(|| {
            let mut blocks = vec![0; CHUNK_CELLS];
            generate_chunk(seed, cx, cz, &mut blocks);
            if let Some(e) = self.edits.get(&(cx, cz)) {
                for &(idx, id) in &e.entries {
                    blocks[idx] = id as u8;
                }
            }
            Chunk { cx, cz, blocks }
        })
    }

    fn record_edit(&mut self, cx: i32, cz: i32, idx: usize, id: i32) {
        if u8::try_from(id).is_ok_and(|id| block_light(id) > 0) {
            self.any_emitters = true;
        }
        let e = self.edits.entry((cx, cz)).or_insert_with(|| {
            self.edit_order.push((cx, cz));
            ChunkEdits::default()
        });
        e.set(idx, id);
    }
}

struct Source<'a> {
    seed: f64,
    store: &'a mut Store,
}

impl LightSource for Source<'_> {
    fn any_emitters(&self) -> bool {
        self.store.any_emitters
    }

    fn ensure_chunk(&mut self, cx: i32, cz: i32) {
        self.store.get_chunk(self.seed, cx, cz);
    }

    fn chunk_blocks(&self, cx: i32, cz: i32) -> &[u8] {
        &self.store.chunks[&(cx, cz)].blocks
    }
}

/// Lazy chunk store with persistent, insertion-ordered player edits.
pub struct World {
    pub seed: f64,
    pub light: WorldLight,
    store: Store,
}

impl World {
    pub fn new(seed: f64) -> Self {
        Self {
            seed,
            light: WorldLight::new(),
            store: Store::default(),
        }
    }

    pub fn any_emitters(&self) -> bool {
        self.store.any_emitters
    }

    /// Direct block writes, like TypeScript chunk-array writes, bypass edit/light tracking.
    pub fn get_chunk(&mut self, cx: i32, cz: i32) -> &mut Chunk {
        self.store.get_chunk(self.seed, cx, cz)
    }

    pub fn has_chunk(&self, cx: i32, cz: i32) -> bool {
        self.store.chunks.contains_key(&(cx, cz))
    }

    pub fn drop_chunk(&mut self, cx: i32, cz: i32) {
        self.store.chunks.remove(&(cx, cz));
        self.light.invalidate(cx, cz);
    }

    pub fn get_block(&mut self, wx: i32, wy: i32, wz: i32) -> u8 {
        if !(0..WORLD_H).contains(&wy) {
            return Block::AIR;
        }
        let cx = wx.div_euclid(CHUNK_SX);
        let cz = wz.div_euclid(CHUNK_SZ);
        let lx = wx.rem_euclid(CHUNK_SX);
        let lz = wz.rem_euclid(CHUNK_SZ);
        self.get_chunk(cx, cz).blocks[block_index(lx, wy, lz)]
    }

    /// Packed `(skylight << 4) | blocklight`, with each channel in 0..15.
    pub fn get_light_packed(&mut self, wx: i32, wy: i32, wz: i32) -> u8 {
        self.light.get_packed(
            &mut Source {
                seed: self.seed,
                store: &mut self.store,
            },
            wx,
            wy,
            wz,
        )
    }

    /// Forward the renderer's neighbourhood warm-up to the owned light solver.
    /// Use `f64::INFINITY` for unlimited initial warm-up.
    pub fn warm_light_for(&mut self, cx: i32, cz: i32, max_computes: f64) -> bool {
        self.light.warm_for(
            &mut Source {
                seed: self.seed,
                store: &mut self.store,
            },
            cx,
            cz,
            max_computes,
        )
    }

    /// `record = false` performs a simulation-only write. IDs wrap on byte storage,
    /// but the original integer is retained in recorded/exported edits.
    pub fn set_block(
        &mut self,
        wx: i32,
        wy: i32,
        wz: i32,
        id: impl Into<i32>,
        record: bool,
    ) -> Option<ChunkKey> {
        if !(0..WORLD_H).contains(&wy) {
            return None;
        }
        let id = id.into();
        let cx = wx.div_euclid(CHUNK_SX);
        let cz = wz.div_euclid(CHUNK_SZ);
        let idx = block_index(wx.rem_euclid(CHUNK_SX), wy, wz.rem_euclid(CHUNK_SZ));
        self.get_chunk(cx, cz).blocks[idx] = id as u8;
        if record {
            self.store.record_edit(cx, cz, idx, id);
        }
        self.light.invalidate_edit(wx, wz);
        Some((cx, cz))
    }

    /// Flat `[wx, wy, wz, id, ...]` array in TS Map insertion order, including overwrites.
    pub fn export_edits(&self) -> Vec<i32> {
        let mut out = Vec::new();
        for &(cx, cz) in &self.store.edit_order {
            for &(idx, id) in &self.store.edits[&(cx, cz)].entries {
                let idx = idx as i32;
                let lx = idx % CHUNK_SX;
                let r = (idx - lx) / CHUNK_SX;
                let lz = r % CHUNK_SZ;
                let wy = (r - lz) / CHUNK_SZ;
                out.extend_from_slice(&[cx * CHUNK_SX + lx, wy, cz * CHUNK_SZ + lz, id]);
            }
        }
        out
    }

    /// Replace seed and edits, clearing loaded chunks and cached light.
    /// Out-of-height records and an incomplete trailing record are ignored.
    pub fn reset(&mut self, seed: f64, edits: &[i32]) {
        self.seed = seed;
        self.store = Store::default();
        self.light.clear();
        for record in edits.chunks_exact(4) {
            let [wx, wy, wz, id] = [record[0], record[1], record[2], record[3]];
            if !(0..WORLD_H).contains(&wy) {
                continue;
            }
            let cx = wx.div_euclid(CHUNK_SX);
            let cz = wz.div_euclid(CHUNK_SZ);
            let idx = block_index(wx.rem_euclid(CHUNK_SX), wy, wz.rem_euclid(CHUNK_SZ));
            self.store.record_edit(cx, cz, idx, id);
        }
    }

    pub fn surface_y(&mut self, wx: i32, wz: i32) -> i32 {
        for y in (0..WORLD_H).rev() {
            if self.get_block(wx, y, wz) != Block::AIR {
                return y;
            }
        }
        -1
    }

    /// Ring-ordered search for solid ground with two clear cells above it.
    pub fn find_spawn(&mut self, wx: i32, wz: i32) -> SpawnPosition {
        let blocked = |b| {
            matches!(
                b,
                Block::LOG | Block::LEAVES | Block::WATER | Block::CACTUS | Block::AIR
            )
        };
        for r in 0i32..=24 {
            for dz in -r..=r {
                for dx in -r..=r {
                    if dx.abs().max(dz.abs()) != r {
                        continue;
                    }
                    let cx = wx + dx;
                    let cz = wz + dz;
                    let top = self.surface_y(cx, cz);
                    if top < 0 {
                        continue;
                    }
                    if blocked(self.get_block(cx, top, cz)) {
                        continue;
                    }
                    if self.get_block(cx, top + 1, cz) != Block::AIR {
                        continue;
                    }
                    if self.get_block(cx, top + 2, cz) != Block::AIR {
                        continue;
                    }
                    return SpawnPosition {
                        x: f64::from(cx) + 0.5,
                        y: f64::from(top + 1),
                        z: f64::from(cz) + 0.5,
                    };
                }
            }
        }
        let fy = self.surface_y(wx, wz);
        SpawnPosition {
            x: f64::from(wx) + 0.5,
            y: f64::from(if fy >= 0 { fy } else { 40 }) + 1.0,
            z: f64::from(wz) + 0.5,
        }
    }
}

impl LightSource for World {
    fn any_emitters(&self) -> bool {
        self.any_emitters()
    }

    fn ensure_chunk(&mut self, cx: i32, cz: i32) {
        self.get_chunk(cx, cz);
    }

    fn chunk_blocks(&self, cx: i32, cz: i32) -> &[u8] {
        &self.store.chunks[&(cx, cz)].blocks
    }
}
