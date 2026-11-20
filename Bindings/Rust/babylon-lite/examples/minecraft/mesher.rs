use std::collections::HashMap;

use crate::blocks::{Block, RenderMode, block_def, light_opaque};
use crate::constants::{CHUNK_SX, CHUNK_SZ, WORLD_H};
use crate::world::World;
use crate::world_light::MAX_LIGHT;

/// Per-tile UV bounds, already inset by the renderer's half texel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TileRect {
    pub u0: f64,
    pub v0: f64,
    pub u1: f64,
    pub v1: f64,
}

/// Texture-free counterpart of `BlockAtlas.rects` and `BlockAtlas.fallback`.
#[derive(Clone, Debug)]
pub struct AtlasLayout {
    pub rects: HashMap<String, TileRect>,
    pub fallback: TileRect,
}

pub trait AtlasLookup {
    /// Return the tile bounds or the atlas fallback for an unknown tile.
    fn rect(&self, tile: &str) -> TileRect;
}

impl AtlasLookup for AtlasLayout {
    fn rect(&self, tile: &str) -> TileRect {
        self.rects.get(tile).copied().unwrap_or(self.fallback)
    }
}

impl<F: Fn(&str) -> TileRect> AtlasLookup for F {
    fn rect(&self, tile: &str) -> TileRect {
        self(tile)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MeshGeometry {
    /// TS `positions` / shader `position`: world-space XYZ triples, 3 floats/vertex.
    pub positions: Vec<f32>,
    /// TS `normals` / shader `normal`: outward unit XYZ triples, 3 floats/vertex.
    pub normals: Vec<f32>,
    /// TS `uvs` / shader `uv`: atlas UV pairs, 2 floats/vertex, V reversed per quad.
    pub uvs: Vec<f32>,
    /// TS `colors` / shader `color`: [AO multiplier, sky/15, block/15, animation flag].
    /// The fourth component means water in blend mode, leaf sway in cutout mode.
    pub colors: Vec<f32>,
    /// TS `indices`: 6 u32 indices/quad into this batch, with the AO-selected diagonal.
    pub indices: Vec<u32>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChunkMeshData {
    /// Solid, depth-writing, back-face-culled geometry (render order 0).
    pub opaque: Option<MeshGeometry>,
    /// Alpha-tested leaves/cactus, depth-writing and back-face-culled (order 1).
    pub cutout: Option<MeshGeometry>,
    /// Alpha-blended water/glass/ice, no depth write or back-face cull (order 1000).
    pub blend: Option<MeshGeometry>,
}

#[derive(Clone, Copy)]
enum Group {
    Top,
    Side,
    Bottom,
}

struct FaceSpec {
    n: [i32; 3],
    u: [i32; 3],
    v: [i32; 3],
    group: Group,
}

const FACES: [FaceSpec; 6] = [
    FaceSpec {
        n: [0, 1, 0],
        u: [1, 0, 0],
        v: [0, 0, 1],
        group: Group::Top,
    },
    FaceSpec {
        n: [0, -1, 0],
        u: [1, 0, 0],
        v: [0, 0, -1],
        group: Group::Bottom,
    },
    FaceSpec {
        n: [1, 0, 0],
        u: [0, 0, 1],
        v: [0, 1, 0],
        group: Group::Side,
    },
    FaceSpec {
        n: [-1, 0, 0],
        u: [0, 0, -1],
        v: [0, 1, 0],
        group: Group::Side,
    },
    FaceSpec {
        n: [0, 0, 1],
        u: [-1, 0, 0],
        v: [0, 1, 0],
        group: Group::Side,
    },
    FaceSpec {
        n: [0, 0, -1],
        u: [1, 0, 0],
        v: [0, 1, 0],
        group: Group::Side,
    },
];

const AO_LUT: [f64; 4] = [0.5, 0.68, 0.84, 1.0];

#[derive(Default)]
struct FaceSamples {
    corners: [[f64; 3]; 4],
    ao: [u8; 4],
    sky: [f64; 4],
    blk: [f64; 4],
}

#[derive(Default)]
struct Batch(MeshGeometry);

impl Batch {
    fn add_face(&mut self, face: FaceSamples, rect: TileRect, normal: [i32; 3], animated: bool) {
        let base = (self.0.positions.len() / 3) as u32;
        let ao_mul = face.ao.map(|ao| AO_LUT[usize::from(ao)]);
        let fluid_flag = if animated { 1.0 } else { 0.0 };
        let uv_u = [rect.u0, rect.u1, rect.u1, rect.u0];
        let uv_v = [rect.v1, rect.v1, rect.v0, rect.v0];
        for (i, corner) in face.corners.iter().enumerate() {
            self.0.positions.extend(corner.map(|v| v as f32));
            self.0.normals.extend(normal.map(|v| v as f32));
            self.0.uvs.extend([uv_u[i] as f32, uv_v[i] as f32]);
            self.0.colors.extend([
                ao_mul[i] as f32,
                face.sky[i] as f32,
                face.blk[i] as f32,
                fluid_flag,
            ]);
        }
        if face.ao[0] + face.ao[2] > face.ao[1] + face.ao[3] {
            self.0
                .indices
                .extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        } else {
            self.0
                .indices
                .extend([base + 1, base + 2, base + 3, base + 1, base + 3, base]);
        }
    }

    fn into_geometry(self) -> Option<MeshGeometry> {
        if self.0.indices.is_empty() {
            return None;
        }
        Some(self.0)
    }
}

fn occludes(id: u8) -> bool {
    block_def(id).is_some_and(|d| d.casts_ao)
}

fn vertex_ao(side1: bool, side2: bool, corner: bool) -> u8 {
    if side1 && side2 {
        return 0;
    }
    3 - (u8::from(side1) + u8::from(side2) + u8::from(corner))
}

fn face_visible(cur: u8, nb: u8) -> bool {
    if nb == Block::AIR {
        return true;
    }
    let Some(nd) = block_def(nb) else {
        return true;
    };
    if nd.hides_neighbor_faces {
        return false;
    }
    if nb == cur {
        return false;
    }
    true
}

/// Emit faces in TS block x/z/y order, then +Y, -Y, +X, -X, +Z, -Z order.
/// Empty batches are `None` (TS `null`). Mesh positions are not chunk-relative;
/// lowering water, foliage sway and directional face shade belong to the shader.
pub fn mesh_chunk(world: &mut World, cx: i32, cz: i32, atlas: &impl AtlasLookup) -> ChunkMeshData {
    let mut opaque = Batch::default();
    let mut cutout = Batch::default();
    let mut blend = Batch::default();
    let base_x = cx * CHUNK_SX;
    let base_z = cz * CHUNK_SZ;
    for x in 0..CHUNK_SX {
        for z in 0..CHUNK_SZ {
            let wx = base_x + x;
            let wz = base_z + z;
            for y in 0..WORLD_H {
                let cur = world.get_block(wx, y, wz);
                if cur == Block::AIR {
                    continue;
                }
                let Some(cd) = block_def(cur) else {
                    continue;
                };
                for f in &FACES {
                    let nx = wx + f.n[0];
                    let ny = y + f.n[1];
                    let nz = wz + f.n[2];
                    let nb = world.get_block(nx, ny, nz);
                    if !face_visible(cur, nb) {
                        continue;
                    }
                    let tile_name = match f.group {
                        Group::Top => cd.faces.top,
                        Group::Side => cd.faces.side,
                        Group::Bottom => cd.faces.bottom,
                    };
                    let rect = atlas.rect(tile_name);
                    let mut samples = FaceSamples::default();
                    let do_ao = cd.render_mode != RenderMode::Blend;
                    let base_fx = f64::from(wx)
                        + f64::from(u8::from(f.n[0] > 0))
                        + f64::from(u8::from(f.u[0] < 0))
                        + f64::from(u8::from(f.v[0] < 0));
                    let base_fy = f64::from(y)
                        + f64::from(u8::from(f.n[1] > 0))
                        + f64::from(u8::from(f.u[1] < 0))
                        + f64::from(u8::from(f.v[1] < 0));
                    let base_fz = f64::from(wz)
                        + f64::from(u8::from(f.n[2] > 0))
                        + f64::from(u8::from(f.u[2] < 0))
                        + f64::from(u8::from(f.v[2] < 0));
                    for (c, corner) in samples.corners.iter_mut().enumerate() {
                        let cu = i32::from(c == 1 || c == 2);
                        let cv = i32::from(c == 2 || c == 3);
                        *corner = [
                            base_fx + f64::from(cu * f.u[0]) + f64::from(cv * f.v[0]),
                            base_fy + f64::from(cu * f.u[1]) + f64::from(cv * f.v[1]),
                            base_fz + f64::from(cu * f.u[2]) + f64::from(cv * f.v[2]),
                        ];
                        let su = if cu == 1 { 1 } else { -1 };
                        let sv = if cv == 1 { 1 } else { -1 };
                        let s1x = nx + su * f.u[0];
                        let s1y = ny + su * f.u[1];
                        let s1z = nz + su * f.u[2];
                        let s2x = nx + sv * f.v[0];
                        let s2y = ny + sv * f.v[1];
                        let s2z = nz + sv * f.v[2];
                        let cnx = nx + su * f.u[0] + sv * f.v[0];
                        let cny = ny + su * f.u[1] + sv * f.v[1];
                        let cnz = nz + su * f.u[2] + sv * f.v[2];
                        let b1 = world.get_block(s1x, s1y, s1z);
                        let b2 = world.get_block(s2x, s2y, s2z);
                        let bc = world.get_block(cnx, cny, cnz);
                        samples.ao[c] = if do_ao {
                            vertex_ao(occludes(b1), occludes(b2), occludes(bc))
                        } else {
                            3
                        };
                        let o1 = light_opaque(b1);
                        let o2 = light_opaque(b2);
                        let oc = light_opaque(bc);
                        let pf = world.get_light_packed(nx, ny, nz);
                        let mut sky_sum = u32::from(pf >> 4);
                        let mut blk_sum = u32::from(pf & 15);
                        let mut cnt = 1;
                        if !o1 {
                            let p = world.get_light_packed(s1x, s1y, s1z);
                            sky_sum += u32::from(p >> 4);
                            blk_sum += u32::from(p & 15);
                            cnt += 1;
                        }
                        if !o2 {
                            let p = world.get_light_packed(s2x, s2y, s2z);
                            sky_sum += u32::from(p >> 4);
                            blk_sum += u32::from(p & 15);
                            cnt += 1;
                        }
                        if !oc && !(o1 && o2) {
                            let p = world.get_light_packed(cnx, cny, cnz);
                            sky_sum += u32::from(p >> 4);
                            blk_sum += u32::from(p & 15);
                            cnt += 1;
                        }
                        samples.sky[c] = f64::from(sky_sum) / f64::from(cnt) / f64::from(MAX_LIGHT);
                        samples.blk[c] = f64::from(blk_sum) / f64::from(cnt) / f64::from(MAX_LIGHT);
                    }
                    let batch = match cd.render_mode {
                        RenderMode::Blend => &mut blend,
                        RenderMode::Cutout => &mut cutout,
                        RenderMode::Opaque => &mut opaque,
                    };
                    batch.add_face(samples, rect, f.n, cd.fluid || cd.sway);
                }
            }
        }
    }
    ChunkMeshData {
        opaque: opaque.into_geometry(),
        cutout: cutout.into_geometry(),
        blend: blend.into_geometry(),
    }
}
