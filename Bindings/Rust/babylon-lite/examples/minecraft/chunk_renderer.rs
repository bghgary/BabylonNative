//! Port of `chunk-renderer.ts`: streams up to three meshes per chunk (opaque,
//! cutout, blend) around the player under a per-frame budget, and retires
//! meshes a few frames after hiding them so no in-flight submit uses them.

use crate::atlas::BlockAtlas;
use crate::constants::{CHUNK_SX, CHUNK_SZ, ChunkKey};
use crate::mesher::{MeshGeometry, mesh_chunk};
use crate::voxel_material::{Mode, Rgb, VoxelMaterialOptions, create_voxel_material, v3};
use crate::world::World;
use babylon_lite::{Engine, Material, Result, Runtime, Scene, SceneNode, ShaderMaterial};
use std::collections::HashMap;

struct Pending {
    cx: i32,
    cz: i32,
    dist: i32,
}

pub struct ChunkRenderer {
    engine: Engine,
    scene: Scene,
    atlas: BlockAtlas,
    radius: i32,
    budget: usize,
    compute_budget: f64,
    opaque: (ShaderMaterial, Material),
    cutout: (ShaderMaterial, Material),
    blend: (ShaderMaterial, Material),
    active: HashMap<ChunkKey, Vec<SceneNode>>,
    pending: Vec<Pending>,
    graveyard: Vec<(Vec<SceneNode>, u32)>,
    counter: u64,
}

impl ChunkRenderer {
    pub fn new(
        runtime: &Runtime,
        engine: &Engine,
        scene: &Scene,
        atlas: BlockAtlas,
        radius: i32,
        budget_per_frame: usize,
        options: VoxelMaterialOptions,
    ) -> Result<Self> {
        let make =
            |name, mode, options: &VoxelMaterialOptions| -> Result<(ShaderMaterial, Material)> {
                let material = create_voxel_material(runtime, name, &atlas.texture, mode, options)?;
                let family = material.as_material()?;
                Ok((material, family))
            };
        let opaque = make("voxelOpaque", Mode::Opaque, &options)?;
        let cutout = make("voxelCutout", Mode::Cutout, &options)?;
        let blend = make(
            "voxelBlend",
            Mode::Blend,
            &VoxelMaterialOptions {
                alpha: 0.72,
                ..options
            },
        )?;
        Ok(Self {
            engine: engine.clone(),
            scene: scene.clone(),
            atlas,
            radius,
            budget: budget_per_frame,
            compute_budget: 4.0,
            opaque,
            cutout,
            blend,
            active: HashMap::new(),
            pending: Vec::new(),
            graveyard: Vec::new(),
            counter: 0,
        })
    }

    pub fn set_time(&self, t: f64) -> Result<()> {
        self.blend.0.set_float("uTime", t)?;
        self.cutout.0.set_float("uTime", t)
    }

    pub fn set_camera_pos(&self, pos: Rgb) -> Result<()> {
        self.blend.0.set_vector3("cameraPos", v3(pos))
    }

    fn all(&self) -> [&ShaderMaterial; 3] {
        [&self.opaque.0, &self.cutout.0, &self.blend.0]
    }

    pub fn set_sun(&self, dir: Rgb, color: Rgb) -> Result<()> {
        for material in self.all() {
            material.set_vector3("sunDir", v3(dir))?;
            material.set_vector3("sunColor", v3(color))?;
        }
        Ok(())
    }

    pub fn set_ambient(&self, color: Rgb) -> Result<()> {
        for material in self.all() {
            material.set_vector3("ambientColor", v3(color))?;
        }
        Ok(())
    }

    pub fn set_fog(&self, color: Rgb) -> Result<()> {
        for material in self.all() {
            material.set_vector3("fogColor", v3(color))?;
        }
        Ok(())
    }

    /// Recomputes the desired chunk set around a world position and queues work.
    pub fn update(&mut self, world: &mut World, center_wx: f64, center_wz: f64) -> Result<()> {
        let ccx = (center_wx / f64::from(CHUNK_SX)).floor() as i32;
        let ccz = (center_wz / f64::from(CHUNK_SZ)).floor() as i32;
        let keep = self.radius + 1;

        let far: Vec<ChunkKey> = self
            .active
            .keys()
            .copied()
            .filter(|&(cx, cz)| (cx - ccx).abs() > keep || (cz - ccz).abs() > keep)
            .collect();
        for key in far {
            let nodes = self.active.remove(&key).unwrap();
            self.retire(nodes)?;
            world.drop_chunk(key.0, key.1);
        }

        self.pending.clear();
        for dz in -self.radius..=self.radius {
            for dx in -self.radius..=self.radius {
                let (cx, cz) = (ccx + dx, ccz + dz);
                if !self.active.contains_key(&(cx, cz)) {
                    self.pending.push(Pending {
                        cx,
                        cz,
                        dist: dx * dx + dz * dz,
                    });
                }
            }
        }
        self.pending.sort_by_key(|p| p.dist);
        Ok(())
    }

    /// Meshes up to the per-frame budget of queued chunks; `compute_budget`
    /// caps light-region floods (`None` keeps the steady-state default).
    pub fn process_queue(&mut self, world: &mut World, compute_budget: Option<f64>) -> Result<()> {
        let compute_budget = compute_budget.unwrap_or(self.compute_budget);
        self.drain_graveyard()?;
        world.light.begin_frame();
        let mut done = 0;
        while done < self.budget && !self.pending.is_empty() {
            let (cx, cz) = (self.pending[0].cx, self.pending[0].cz);
            if !world.warm_light_for(cx, cz, compute_budget) {
                break;
            }
            self.pending.remove(0);
            if self.active.contains_key(&(cx, cz)) {
                continue;
            }
            self.build_chunk(world, cx, cz)?;
            done += 1;
        }
        Ok(())
    }

    fn remesh(&mut self, world: &mut World, cx: i32, cz: i32) -> Result<()> {
        if let Some(nodes) = self.active.remove(&(cx, cz)) {
            self.retire(nodes)?;
        }
        self.build_chunk(world, cx, cz)
    }

    fn remesh_if_active(&mut self, world: &mut World, cx: i32, cz: i32) -> Result<()> {
        if self.active.contains_key(&(cx, cz)) {
            self.remesh(world, cx, cz)?;
        }
        Ok(())
    }

    /// Re-meshes the edited chunk, plus neighbours only when the edit is on a border.
    pub fn remesh_edit(&mut self, world: &mut World, wx: i32, wz: i32) -> Result<()> {
        let (cx, cz) = (wx.div_euclid(CHUNK_SX), wz.div_euclid(CHUNK_SZ));
        let (lx, lz) = (wx - cx * CHUNK_SX, wz - cz * CHUNK_SZ);
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
        self.remesh_if_active(world, cx, cz)?;
        if nx != 0 {
            self.remesh_if_active(world, cx + nx, cz)?;
        }
        if nz != 0 {
            self.remesh_if_active(world, cx, cz + nz)?;
        }
        if nx != 0 && nz != 0 {
            self.remesh_if_active(world, cx + nx, cz + nz)?;
        }
        Ok(())
    }

    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    fn retire(&mut self, nodes: Vec<SceneNode>) -> Result<()> {
        for node in &nodes {
            node.set_subtree_visible(false)?;
        }
        self.graveyard.push((nodes, 3));
        Ok(())
    }

    fn drain_graveyard(&mut self) -> Result<()> {
        for i in (0..self.graveyard.len()).rev() {
            self.graveyard[i].1 -= 1;
            if self.graveyard[i].1 > 0 {
                continue;
            }
            let (nodes, _) = self.graveyard.remove(i);
            for node in &nodes {
                self.scene.remove(node)?;
            }
        }
        Ok(())
    }

    fn build_chunk(&mut self, world: &mut World, cx: i32, cz: i32) -> Result<()> {
        let data = mesh_chunk(world, cx, cz, &self.atlas.layout);
        let mut nodes = Vec::new();
        let parts = [
            (data.opaque, &self.opaque.1, "op", 0.0),
            (data.cutout, &self.cutout.1, "ct", 1.0),
            (data.blend, &self.blend.1, "bl", 1000.0),
        ];
        for (geometry, material, tag, order) in parts {
            let Some(MeshGeometry {
                positions,
                normals,
                uvs,
                colors,
                indices,
            }) = geometry
            else {
                continue;
            };
            let name = format!("mc_{tag}_{}", self.counter);
            self.counter += 1;
            let mesh = self.engine.create_mesh_from_data(
                &name,
                &babylon_lite::MeshGeometry {
                    positions: &positions,
                    normals: &normals,
                    indices: &indices,
                    uvs: Some(&uvs),
                    colors: Some(&colors),
                    ..Default::default()
                },
            )?;
            mesh.set_material(Some(material))?;
            mesh.set_render_order(Some(order))?;
            let node = mesh.node()?;
            self.scene.add(&node)?;
            nodes.push(node);
        }
        self.active.insert((cx, cz), nodes);
        Ok(())
    }
}
