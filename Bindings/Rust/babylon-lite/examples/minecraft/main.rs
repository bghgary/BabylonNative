//! Rust port of the Babylon Lite "Minecraft" voxel sandbox demo
//! (`Apps/LiteTests/LitePlayground/JavaScript/Demos/minecraft.ts`).
//!
//! Click to capture the mouse (Esc releases it), WASD to walk, Space to jump or
//! swim up, Left Shift to sprint, left click to break, right click to place,
//! 1-0 / Tab / wheel to pick a block. The window title replaces the HUD.
//!
//! Ported: terrain, lighting, meshing, atlas, voxel/sky/cloud/highlight shaders,
//! day-night cycle, chunk streaming and the player controller. Not ported: HUD,
//! audio, mobs, particles, falling blocks, water simulation, save/load and the
//! underwater overlay.
//!
//! `--time <0..1>` sets the start time of day (0.5 = noon), as the original's
//! `?time=` parameter does; `--frames <n>` exits after n frames.

mod atlas;
#[allow(dead_code)]
mod blocks;
mod chunk_renderer;
mod clouds;
#[allow(dead_code)]
mod constants;
mod highlight;
mod lighting;
#[allow(dead_code)]
mod mesher;
#[allow(dead_code)]
mod noise;
#[cfg(test)]
mod oracle_tests;
mod player;
mod raycast;
mod sky;
mod voxel_material;
#[allow(dead_code)]
mod world;
#[allow(dead_code)]
mod world_light;
mod worldgen;

use babylon_lite::{Engine, EngineOptions, Result, Runtime, Scene, SceneNode, color4, vec3};
use chunk_renderer::ChunkRenderer;
use clouds::Clouds;
use highlight::Highlight;
use lighting::Lighting;
use player::Player;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use sky::Sky;
use std::path::PathBuf;
use std::time::Instant;
use voxel_material::{Rgb, VoxelMaterialOptions};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{
    DeviceEvent, DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent,
};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorGrabMode, Window, WindowId};

const SEED: f64 = 1337.0;
const RENDER_RADIUS: i32 = 6;
const FOG_COLOR: Rgb = [0.7, 0.82, 0.92];
const FOG_START: f64 = (RENDER_RADIUS - 1) as f64 * 16.0 * 0.6;
const FOG_END: f64 = RENDER_RADIUS as f64 * 16.0 * 0.95;

struct Game {
    engine: Engine,
    world: world::World,
    renderer: ChunkRenderer,
    sky: Sky,
    sky_node: SceneNode,
    lighting: Lighting,
    highlight: Highlight,
    clouds: Clouds,
    scene: Scene,
    player: Player,
    time: f64,
    fps: f64,
    frames: u64,
    last_frame: Instant,
    last_title: Instant,
    shift: bool,
    // Dropped last: the engine presents to this window until the runtime goes.
    window: Window,
}

impl Game {
    fn new(event_loop: &ActiveEventLoop, start_time_of_day: f64) -> Result<Self> {
        let attributes = Window::default_attributes()
            .with_title("Voxel Sandbox (Babylon Lite from Rust)")
            .with_inner_size(LogicalSize::new(1280.0, 720.0));
        let window = event_loop.create_window(attributes).expect("create window");
        let RawWindowHandle::Win32(handle) =
            window.window_handle().expect("window handle").as_raw()
        else {
            panic!("Babylon Lite's runtime shader compiler currently requires Win32/D3D11");
        };
        let size = window.inner_size();

        let runtime = Runtime::new()?;
        let options = EngineOptions::new(
            handle.hwnd.get() as *mut _,
            size.width.max(1),
            size.height.max(1),
        );
        // The window outlives the engine: it is the last field of `Game`.
        let engine = unsafe { runtime.create_engine(&options)? };
        let scene = engine.create_scene()?;
        scene.set_clear_color(color4(FOG_COLOR[0], FOG_COLOR[1], FOG_COLOR[2], 1.0))?;

        let pack = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../Apps/LiteTests/LitePlayground/JavaScript/Demos/minecraft/voxelpack");
        let atlas = atlas::build_block_atlas(&engine, &pack, &blocks::all_referenced_tiles())?;
        let mut world = world::World::new(SEED);

        let mut renderer = ChunkRenderer::new(
            &runtime,
            &engine,
            &scene,
            atlas,
            RENDER_RADIUS,
            3,
            VoxelMaterialOptions {
                fog_color: FOG_COLOR,
                fog_start: FOG_START,
                fog_end: FOG_END,
                ..Default::default()
            },
        )?;

        let sky = Sky::new(&runtime, &engine, FOG_COLOR)?;
        let sky_node = sky.mesh.node()?;
        scene.add(&sky_node)?;

        let lighting = Lighting::new(150.0, start_time_of_day);
        let highlight = Highlight::new(&runtime, &engine)?;
        scene.add(&highlight.mesh.node()?)?;

        let player = Player::new(&runtime, &scene, &mut world)?;
        let clouds = Clouds::new(&runtime, &engine, &scene)?;

        // Warm the spawn region so the world is solid on the first frame.
        renderer.update(&mut world, player.x, player.z)?;
        let side = (2 * RENDER_RADIUS + 1) as usize;
        for _ in 0..side * side + 4 {
            renderer.process_queue(&mut world, Some(f64::INFINITY))?;
        }

        let game = Self {
            engine,
            world,
            renderer,
            sky,
            sky_node,
            lighting,
            highlight,
            clouds,
            scene,
            player,
            time: 0.0,
            fps: 60.0,
            frames: 0,
            last_frame: Instant::now(),
            last_title: Instant::now(),
            shift: false,
            window,
        };
        game.apply_lighting()?;
        game.scene.register()?;
        game.engine.start()?;
        Ok(game)
    }

    /// Pushes the current lighting to the sky, voxel materials and clear colour.
    fn apply_lighting(&self) -> Result<()> {
        let l = &self.lighting;
        self.sky.set_sun(l.sun_dir)?;
        self.sky.set_colors(l.fog, l.zenith)?;
        self.renderer.set_sun(l.sun_dir, l.sun_color)?;
        self.renderer.set_ambient(l.ambient)?;
        self.renderer.set_fog(l.fog)?;
        self.scene
            .set_clear_color(color4(l.fog[0], l.fog[1], l.fog[2], 1.0))
    }

    fn frame(&mut self) -> Result<()> {
        let now = Instant::now();
        let delta_ms = (now - self.last_frame).as_secs_f64() * 1000.0;
        self.last_frame = now;
        let dt = (delta_ms / 1000.0).min(0.1);
        self.time += dt;
        self.fps = self.fps * 0.9 + (1.0 / dt.max(1e-4)) * 0.1;

        self.player
            .update(dt, &mut self.world, &mut self.renderer, &self.highlight)?;
        self.renderer.process_queue(&mut self.world, None)?;
        self.renderer.set_time(self.time)?;
        let eye = self.player.eye();
        self.renderer.set_camera_pos(eye)?;
        self.highlight.set_time(self.time)?;

        self.lighting.tick(dt);
        self.apply_lighting()?;

        let l = &self.lighting;
        let day = ((l.sun_dir[1] + 0.05) / 0.3).clamp(0.0, 1.0);
        let off = 0.03 + 0.32 * day;
        self.clouds.set_tint([
            (l.ambient[0] * 0.6 + l.sun_color[0] * 0.7 + off).min(1.0),
            (l.ambient[1] * 0.6 + l.sun_color[1] * 0.7 + off).min(1.0),
            (l.ambient[2] * 0.6 + l.sun_color[2] * 0.7 + off + 0.02).min(1.0),
        ])?;
        self.clouds.update(eye[0], eye[2], self.time)?;
        self.sky_node.set_position(vec3(eye[0], eye[1], eye[2]))?;

        self.engine.frame(delta_ms)?;
        self.frames += 1;

        if (now - self.last_title).as_secs_f64() >= 0.5 {
            self.last_title = now;
            self.window.set_title(&format!(
                "Voxel Sandbox (Babylon Lite from Rust)  fps {:.0}  chunks {}  time {}  {}{}",
                self.fps,
                self.renderer.active_count(),
                self.lighting.clock_text(),
                self.player.status(),
                if self.player.locked {
                    ""
                } else {
                    "  (click to play)"
                }
            ));
        }
        Ok(())
    }

    fn set_locked(&mut self, locked: bool) {
        let mode = if locked {
            CursorGrabMode::Confined
        } else {
            CursorGrabMode::None
        };
        if self.window.set_cursor_grab(mode).is_ok() {
            self.window.set_cursor_visible(!locked);
            self.player.locked = locked;
        }
    }

    fn handle(&mut self, event: &WindowEvent) -> Result<()> {
        match event {
            WindowEvent::Resized(size) => self.engine.resize(size.width.max(1), size.height.max(1)),
            WindowEvent::Focused(false) => {
                self.set_locked(false);
                Ok(())
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button,
                ..
            } => {
                if !self.player.locked {
                    self.set_locked(true);
                    return Ok(());
                }
                match button {
                    MouseButton::Left => {
                        self.player.break_block(&mut self.world, &mut self.renderer)
                    }
                    MouseButton::Right => {
                        self.player.place_block(&mut self.world, &mut self.renderer)
                    }
                    _ => Ok(()),
                }
            }
            WindowEvent::MouseWheel { delta, .. } if self.player.locked => {
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_, lines) => -f64::from(*lines),
                    MouseScrollDelta::PixelDelta(pixels) => -pixels.y,
                };
                if delta_y != 0.0 {
                    self.player.cycle_slot(delta_y.signum() as i32);
                }
                Ok(())
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return Ok(());
                };
                let pressed = event.state == ElementState::Pressed;
                if matches!(code, KeyCode::ShiftLeft | KeyCode::ShiftRight) {
                    self.shift = pressed;
                }
                if code == KeyCode::Escape && pressed {
                    self.set_locked(false);
                }
                self.player.key(code, pressed, self.shift);
                Ok(())
            }
            WindowEvent::RedrawRequested => self.frame(),
            _ => Ok(()),
        }
    }
}

struct App {
    game: Option<Game>,
    start_time_of_day: f64,
    frame_limit: Option<u64>,
    result: Result<()>,
}

impl App {
    fn finish(&mut self, event_loop: &ActiveEventLoop, result: Result<()>) {
        if let Some(game) = &self.game {
            let draws = game.engine.stats().map_or(0, |s| s.draw_call_count);
            println!(
                "{} frames, {} active chunks, last frame drew {draws} calls",
                game.frames,
                game.renderer.active_count()
            );
        }
        self.game = None;
        self.result = result;
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.game.is_none() {
            match Game::new(event_loop, self.start_time_of_day) {
                Ok(game) => self.game = Some(game),
                Err(error) => self.finish(event_loop, Err(error)),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(game) = self.game.as_mut() else {
            return;
        };
        if let WindowEvent::CloseRequested = event {
            return self.finish(event_loop, Ok(()));
        }
        let result = game.handle(&event);
        let done = self.frame_limit.is_some_and(|limit| game.frames >= limit);
        if result.is_err() || done {
            self.finish(event_loop, result);
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let (Some(game), DeviceEvent::MouseMotion { delta }) = (self.game.as_mut(), event) {
            game.player.mouse_moved(delta.0, delta.1);
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(game) = &self.game {
            game.window.request_redraw();
        }
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut app = App {
        game: None,
        start_time_of_day: 0.32,
        frame_limit: None,
        result: Ok(()),
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--frames" => {
                app.frame_limit = Some(args.next().ok_or("--frames needs a count")?.parse()?)
            }
            "--time" => {
                app.start_time_of_day = args.next().ok_or("--time needs a value")?.parse()?
            }
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut app)?;
    Ok(app.result?)
}
