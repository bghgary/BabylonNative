//! Port of `controls.ts`: first-person creative controls with mouse look, WASD
//! walking with gravity, jump and voxel AABB collision, swimming, and
//! raycast-based block break/place. Particles, audio, falling blocks and water
//! simulation hooks of the original are not ported.

use crate::blocks::{Block, HOTBAR, block_def, is_indestructible};
use crate::chunk_renderer::ChunkRenderer;
use crate::constants::WORLD_H;
use crate::highlight::Highlight;
use crate::raycast::{RayHit, raycast_voxel};
use crate::world::World;
use babylon_lite::{FreeCamera, Result, Runtime, Scene, SceneNode, vec3};
use std::collections::HashSet;
use winit::keyboard::KeyCode;

const REACH: f64 = 7.0;
const WALK_SPEED: f64 = 5.6;
const SPRINT: f64 = 1.6;
const LOOK_SENS: f64 = 0.0022;
const GRAVITY: f64 = 30.0;
const JUMP_VELOCITY: f64 = 9.2;
const MAX_FALL: f64 = 55.0;
const SWIM_UP_SPEED: f64 = 4.6;
const SWIM_SINK_SPEED: f64 = 1.7;
const WATER_DRAG: f64 = 9.0;
const PLAYER_HALF_W: f64 = 0.3;
const PLAYER_HEIGHT: f64 = 1.8;
const EYE_HEIGHT: f64 = 1.62;

pub struct Player {
    pub camera: FreeCamera,
    camera_node: SceneNode,
    pub x: f64,
    pub feet_y: f64,
    pub z: f64,
    vy: f64,
    on_ground: bool,
    yaw: f64,
    pitch: f64,
    pub locked: bool,
    keys: HashSet<KeyCode>,
    target: Option<RayHit>,
    pub selected_slot: usize,
}

impl Player {
    pub fn new(runtime: &Runtime, scene: &Scene, world: &mut World) -> Result<Self> {
        let spawn = world.find_spawn(8, 8);
        let eye = spawn.y + EYE_HEIGHT;
        let camera = runtime.create_free_camera(
            vec3(spawn.x, eye, spawn.z),
            vec3(spawn.x + 1.0, eye, spawn.z),
        )?;
        let mut properties = camera.properties()?;
        properties.nearPlane = 0.1;
        properties.farPlane = 1000.0;
        camera.set_properties(&properties)?;
        scene.set_camera(Some(&camera.as_camera()?))?;
        let camera_node = camera.node()?;
        Ok(Self {
            camera,
            camera_node,
            x: spawn.x,
            feet_y: spawn.y,
            z: spawn.z,
            vy: 0.0,
            on_ground: false,
            yaw: 0.0,
            pitch: -0.2,
            locked: false,
            keys: HashSet::new(),
            target: None,
            selected_slot: 0,
        })
    }

    pub fn eye(&self) -> [f64; 3] {
        [self.x, self.feet_y + EYE_HEIGHT, self.z]
    }

    pub fn key(&mut self, code: KeyCode, pressed: bool, shift: bool) {
        if !pressed {
            self.keys.remove(&code);
            return;
        }
        self.keys.insert(code);
        let digit = match code {
            KeyCode::Digit1 => Some(1),
            KeyCode::Digit2 => Some(2),
            KeyCode::Digit3 => Some(3),
            KeyCode::Digit4 => Some(4),
            KeyCode::Digit5 => Some(5),
            KeyCode::Digit6 => Some(6),
            KeyCode::Digit7 => Some(7),
            KeyCode::Digit8 => Some(8),
            KeyCode::Digit9 => Some(9),
            KeyCode::Digit0 => Some(10),
            _ => None,
        };
        if let Some(n) = digit.filter(|&n| n <= HOTBAR.len()) {
            self.selected_slot = n - 1;
        }
        if code == KeyCode::Tab {
            self.cycle_slot(if shift { -1 } else { 1 });
        }
    }

    pub fn cycle_slot(&mut self, direction: i32) {
        let len = HOTBAR.len() as i32;
        self.selected_slot = (self.selected_slot as i32 + direction + len).rem_euclid(len) as usize;
    }

    pub fn mouse_moved(&mut self, dx: f64, dy: f64) {
        if !self.locked {
            return;
        }
        self.yaw -= dx * LOOK_SENS;
        self.pitch = (self.pitch - dy * LOOK_SENS).clamp(-1.55, 1.55);
    }

    fn forward(&self) -> [f64; 3] {
        let cp = self.pitch.cos();
        [cp * self.yaw.cos(), self.pitch.sin(), cp * self.yaw.sin()]
    }

    pub fn break_block(&mut self, world: &mut World, renderer: &mut ChunkRenderer) -> Result<()> {
        let Some(hit) = self.target else {
            return Ok(());
        };
        if is_indestructible(world.get_block(hit.bx, hit.by, hit.bz)) {
            return Ok(());
        }
        if world
            .set_block(hit.bx, hit.by, hit.bz, Block::AIR, true)
            .is_some()
        {
            renderer.remesh_edit(world, hit.bx, hit.bz)?;
        }
        Ok(())
    }

    pub fn place_block(&mut self, world: &mut World, renderer: &mut ChunkRenderer) -> Result<()> {
        let Some(hit) = self.target else {
            return Ok(());
        };
        if !(0..WORLD_H).contains(&hit.py) || self.intersects_player(hit.px, hit.py, hit.pz) {
            return Ok(());
        }
        let block = HOTBAR
            .get(self.selected_slot)
            .copied()
            .unwrap_or(Block::STONE);
        if world
            .set_block(hit.px, hit.py, hit.pz, block, true)
            .is_some()
        {
            renderer.remesh_edit(world, hit.px, hit.pz)?;
        }
        Ok(())
    }

    fn solid_at(world: &mut World, bx: i32, by: i32, bz: i32) -> bool {
        if by < 0 {
            return true;
        }
        if by >= WORLD_H {
            return false;
        }
        block_def(world.get_block(bx, by, bz)).is_some_and(|d| d.collidable)
    }

    fn collides(world: &mut World, x: f64, fy: f64, z: f64) -> bool {
        let (min_x, max_x) = (
            (x - PLAYER_HALF_W).floor() as i32,
            (x + PLAYER_HALF_W).floor() as i32,
        );
        let (min_y, max_y) = (
            fy.floor() as i32,
            (fy + PLAYER_HEIGHT - 1e-4).floor() as i32,
        );
        let (min_z, max_z) = (
            (z - PLAYER_HALF_W).floor() as i32,
            (z + PLAYER_HALF_W).floor() as i32,
        );
        for bx in min_x..=max_x {
            for by in min_y..=max_y {
                for bz in min_z..=max_z {
                    if Self::solid_at(world, bx, by, bz) {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn intersects_player(&self, px: i32, py: i32, pz: i32) -> bool {
        px >= (self.x - PLAYER_HALF_W).floor() as i32
            && px <= (self.x + PLAYER_HALF_W).floor() as i32
            && py >= self.feet_y.floor() as i32
            && py <= (self.feet_y + PLAYER_HEIGHT - 1e-4).floor() as i32
            && pz >= (self.z - PLAYER_HALF_W).floor() as i32
            && pz <= (self.z + PLAYER_HALF_W).floor() as i32
    }

    fn in_water(&self, world: &mut World) -> bool {
        let (x, z) = (self.x.floor() as i32, self.z.floor() as i32);
        world.get_block(x, self.feet_y.floor() as i32, z) == Block::WATER
            || world.get_block(x, (self.feet_y + 0.9).floor() as i32, z) == Block::WATER
    }

    fn move_horizontal(&mut self, world: &mut World, dx: f64, dz: f64) {
        if dx != 0.0 && !Self::collides(world, self.x + dx, self.feet_y, self.z) {
            self.x += dx;
        }
        if dz != 0.0 && !Self::collides(world, self.x, self.feet_y, self.z + dz) {
            self.z += dz;
        }
    }

    fn move_vertical(&mut self, world: &mut World, dy: f64) {
        let ny = self.feet_y + dy;
        if !Self::collides(world, self.x, ny, self.z) {
            self.feet_y = ny;
            self.on_ground = false;
            return;
        }
        if dy < 0.0 {
            self.feet_y = ny.floor() + 1.0;
            self.on_ground = true;
        } else if dy > 0.0 {
            self.feet_y = (ny + PLAYER_HEIGHT).floor() - PLAYER_HEIGHT;
        }
        self.vy = 0.0;
    }

    pub fn update(
        &mut self,
        dt: f64,
        world: &mut World,
        renderer: &mut ChunkRenderer,
        highlight: &Highlight,
    ) -> Result<()> {
        let [fx, fy, fz] = self.forward();
        let flat_len = fx.hypot(fz);
        let flat_len = if flat_len == 0.0 { 1.0 } else { flat_len };
        let (hx, hz) = (fx / flat_len, fz / flat_len);
        let (rx, rz) = (hz, -hx);

        let (mut mx, mut mz) = (0.0, 0.0);
        let held = |code| self.keys.contains(&code);
        if held(KeyCode::KeyW) {
            mx += hx;
            mz += hz;
        }
        if held(KeyCode::KeyS) {
            mx -= hx;
            mz -= hz;
        }
        if held(KeyCode::KeyD) {
            mx += rx;
            mz += rz;
        }
        if held(KeyCode::KeyA) {
            mx -= rx;
            mz -= rz;
        }

        let (mut dx, mut dz) = (0.0, 0.0);
        let len = f64::hypot(mx, mz);
        if len > 0.0 {
            let sprint = if held(KeyCode::ShiftLeft) {
                SPRINT
            } else {
                1.0
            };
            let speed = WALK_SPEED * sprint * dt / len;
            dx = mx * speed;
            dz = mz * speed;
        }
        let jump = held(KeyCode::Space);

        if self.in_water(world) {
            let target = if jump {
                SWIM_UP_SPEED
            } else {
                -SWIM_SINK_SPEED
            };
            self.vy += (target - self.vy) * (WATER_DRAG * dt).min(1.0);
            self.on_ground = false;
        } else {
            if jump && self.on_ground {
                self.vy = JUMP_VELOCITY;
                self.on_ground = false;
            }
            self.vy = (self.vy - GRAVITY * dt).max(-MAX_FALL);
        }

        self.move_horizontal(world, dx, dz);
        self.move_vertical(world, self.vy * dt);
        self.feet_y = self.feet_y.clamp(1.0, f64::from(WORLD_H) - PLAYER_HEIGHT);

        let eye = self.feet_y + EYE_HEIGHT;
        self.camera_node.set_position(vec3(self.x, eye, self.z))?;
        self.camera
            .set_target(vec3(self.x + fx, eye + fy, self.z + fz))?;

        renderer.update(world, self.x, self.z)?;

        self.target = raycast_voxel(world, [self.x, eye, self.z], [fx, fy, fz], REACH);
        match self.target {
            Some(hit) => highlight.show(hit.bx, hit.by, hit.bz),
            None => highlight.hide(),
        }
    }

    pub fn status(&self) -> String {
        let name = block_def(HOTBAR[self.selected_slot]).map_or("Block", |d| d.name);
        format!(
            "xyz {:.1} {:.1} {:.1} {}  [{}] {}",
            self.x,
            self.feet_y,
            self.z,
            if self.on_ground {
                "grounded"
            } else {
                "airborne"
            },
            self.selected_slot + 1,
            name
        )
    }
}
