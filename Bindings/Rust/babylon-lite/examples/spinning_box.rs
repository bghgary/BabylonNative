//! Opens a window and renders a spinning, lit box on a ground plane. Drag with
//! the left mouse button to orbit, the right button to pan, and use the wheel
//! to zoom. `--frames <n>` exits after n frames and prints the draw count.

use babylon_lite::{
    ArcRotateControl, BoxOptions, Engine, EngineOptions, GroundOptions, Result, Runtime, SceneNode,
    vec3,
};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use std::f64::consts::PI;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

struct Demo {
    engine: Engine,
    control: ArcRotateControl,
    spinner: SceneNode,
    last_frame: Instant,
    cursor: (f64, f64),
    frames: u64,
    // Declared last so it is dropped after every Babylon Lite handle, and with
    // them the runtime and the engine that presents to this window.
    window: Window,
}

impl Demo {
    fn new(event_loop: &ActiveEventLoop) -> Result<Self> {
        let attributes = Window::default_attributes()
            .with_title("Babylon Lite from Rust")
            .with_inner_size(LogicalSize::new(1024.0, 640.0));
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
        // The window outlives the engine: it is the last field of `Demo`.
        let engine = unsafe { runtime.create_engine(&options)? };
        let scene = engine.create_scene()?;

        let camera = runtime.create_arc_rotate_camera(-PI / 2.0, 1.1, 6.0, vec3(0.0, 0.5, 0.0))?;
        scene.set_camera(Some(&camera.as_camera()?))?;
        let control = camera.attach_control(Some(&scene))?;

        let light = runtime.create_hemispheric_light(None, Some(0.9))?;
        scene.add(&light.node()?)?;

        let orange = runtime.create_standard_material()?;
        let mut properties = orange.properties()?;
        properties.diffuseColor = vec3(0.95, 0.45, 0.1);
        orange.set_properties(&properties)?;
        let cube = engine.create_box(&BoxOptions::default())?;
        cube.set_material(Some(&orange.as_material()?))?;
        let spinner = cube.node()?;
        spinner.set_position(vec3(0.0, 0.5, 0.0))?;
        scene.add(&spinner)?;

        let grey = runtime.create_standard_material()?;
        let mut properties = grey.properties()?;
        properties.diffuseColor = vec3(0.5, 0.5, 0.5);
        grey.set_properties(&properties)?;
        let ground = engine.create_ground(&GroundOptions {
            width: Some(6.0),
            height: Some(6.0),
            ..Default::default()
        })?;
        ground.set_material(Some(&grey.as_material()?))?;
        scene.add(&ground.node()?)?;

        scene.register()?;
        engine.start()?;
        Ok(Self {
            engine,
            control,
            spinner,
            last_frame: Instant::now(),
            cursor: (0.0, 0.0),
            frames: 0,
            window,
        })
    }

    fn render(&mut self) -> Result<()> {
        let now = Instant::now();
        let delta_ms = (now - self.last_frame).as_secs_f64() * 1000.0;
        self.last_frame = now;
        let mut rotation = self.spinner.rotation()?;
        rotation.y += delta_ms * 0.001;
        self.spinner.set_rotation(rotation)?;
        self.engine.frame(delta_ms)?;
        self.frames += 1;
        Ok(())
    }

    fn handle(&mut self, event: &WindowEvent) -> Result<()> {
        match *event {
            WindowEvent::Resized(size) => self.engine.resize(size.width.max(1), size.height.max(1)),
            WindowEvent::CursorMoved { position, .. } => {
                let position = position.to_logical::<f64>(self.window.scale_factor());
                self.cursor = (position.x, position.y);
                self.control
                    .pointer_move(1, position.x, position.y)
                    .map(drop)
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    MouseButton::Left => 0,
                    MouseButton::Middle => 1,
                    MouseButton::Right => 2,
                    _ => return Ok(()),
                };
                let (x, y) = self.cursor;
                match state {
                    ElementState::Pressed => self.control.pointer_down(1, button, x, y),
                    ElementState::Released => self.control.pointer_up(1, button, x, y),
                }
                .map(drop)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let delta_y = match delta {
                    MouseScrollDelta::LineDelta(_, lines) => -f64::from(lines) * 100.0,
                    MouseScrollDelta::PixelDelta(pixels) => -pixels.y,
                };
                self.control.wheel(delta_y).map(drop)
            }
            WindowEvent::RedrawRequested => self.render(),
            _ => Ok(()),
        }
    }
}

struct App {
    demo: Option<Demo>,
    frame_limit: Option<u64>,
    result: Result<()>,
}

impl App {
    fn finish(&mut self, event_loop: &ActiveEventLoop, result: Result<()>) {
        if let Some(demo) = &self.demo {
            match demo.engine.stats() {
                Ok(stats) => println!(
                    "{} frames, last frame drew {} calls",
                    demo.frames, stats.draw_call_count
                ),
                Err(error) => println!("{} frames; stats unavailable: {error}", demo.frames),
            }
        }
        self.demo = None;
        self.result = result;
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.demo.is_none() {
            match Demo::new(event_loop) {
                Ok(demo) => self.demo = Some(demo),
                Err(error) => self.finish(event_loop, Err(error)),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(demo) = self.demo.as_mut() else {
            return;
        };
        if let WindowEvent::CloseRequested = event {
            return self.finish(event_loop, Ok(()));
        }
        let result = demo.handle(&event);
        let done = self.frame_limit.is_some_and(|limit| demo.frames >= limit);
        if result.is_err() || done {
            self.finish(event_loop, result);
        }
    }

    fn about_to_wait(&mut self, _: &ActiveEventLoop) {
        if let Some(demo) = &self.demo {
            demo.window.request_redraw();
        }
    }
}

fn main() -> std::result::Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut frame_limit = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--frames" => frame_limit = Some(args.next().ok_or("--frames needs a count")?.parse()?),
            other => return Err(format!("unknown argument {other}").into()),
        }
    }
    let event_loop = EventLoop::new()?;
    let mut app = App {
        demo: None,
        frame_limit,
        result: Ok(()),
    };
    event_loop.run_app(&mut app)?;
    Ok(app.result?)
}
