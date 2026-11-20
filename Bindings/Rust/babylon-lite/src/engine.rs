use crate::{Result, Runtime, get, sys};
use std::ffi::c_void;
use std::fmt;
use std::ptr;
use std::rc::Rc;

/// Options for an engine that owns bgfx and presents to a window's swap chain
/// with the Direct3D 11 backend.
#[derive(Clone, Debug)]
pub struct EngineOptions {
    /// Native window handle (`HWND`).
    pub window: *mut c_void,
    /// Swap chain size in physical pixels.
    pub width: u32,
    pub height: u32,
    /// 1, 2, 4, 8 or 16. The original Babylon Lite default is 4.
    pub sample_count: u8,
    pub vsync: bool,
    /// Original `useHighPrecisionMatrix`; `None` keeps the source default (false).
    pub high_precision_matrix: Option<bool>,
}

impl EngineOptions {
    pub fn new(window: *mut c_void, width: u32, height: u32) -> Self {
        Self {
            window,
            width,
            height,
            sample_count: 4,
            vsync: true,
            high_precision_matrix: None,
        }
    }
}

/// Draw statistics from the most recent frame.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct EngineStats {
    pub draw_call_count: u64,
    /// 0 unless a supported GPU timer is enabled.
    pub gpu_frame_time_ms: f64,
}

/// A rendering engine (`bl_EngineContext`).
#[derive(Clone)]
pub struct Engine {
    pub(crate) raw: sys::bl_EngineContext,
    pub(crate) runtime: Rc<crate::runtime::RuntimeInner>,
    /// Creation target; `resize` changes only its size.
    target: sys::bl_NativeTarget,
}

unsafe extern "C" fn host_bgfx_not_initialized(_: *mut c_void) -> bool {
    false
}

impl Runtime {
    /// Creates an engine that initializes bgfx on `options.window` and owns it.
    ///
    /// bgfx is process-global, so at most one such engine can exist at a time;
    /// creating another returns [`crate::Status::Busy`].
    ///
    /// # Safety
    /// `options.window` must be a valid `HWND` that stays valid until the
    /// engine is disposed, either explicitly or when the runtime is dropped.
    pub unsafe fn create_engine(&self, options: &EngineOptions) -> Result<Engine> {
        let target = sys::bl_NativeTarget {
            kind: sys::bl_NativeTargetKind::BL_TARGET_SWAPCHAIN,
            framebufferIndex: sys::BL_INVALID_BGFX_HANDLE as u16,
            width: options.width,
            height: options.height,
            colorFormat: sys::bl_ColorFormat::BL_COLOR_RGBA8,
            depthFormat: sys::bl_DepthFormat::BL_DEPTH_D24S8,
            sampleCount: options.sample_count,
            firstViewId: 0,
            viewCount: 8,
        };
        let native = sys::bl_NativeEngineOptions {
            ownership: sys::bl_BgfxOwnership::BL_BGFX_OWNED,
            backend: sys::bl_RendererBackend::BL_RENDERER_D3D11,
            platform: sys::bl_NativePlatformData {
                nativeWindowHandle: options.window,
                ..Default::default()
            },
            target,
            vsync: options.vsync,
            singleThreadedRenderer: false,
            hostUserData: ptr::null_mut(),
            isExternalBgfxInitialized: Some(host_bgfx_not_initialized),
            waitForSubmittedWork: None,
        };
        let engine_options = sys::bl_EngineOptions {
            useHighPrecisionMatrix: match options.high_precision_matrix {
                None => sys::bl_OptionalBool::BL_BOOL_DEFAULT,
                Some(false) => sys::bl_OptionalBool::BL_BOOL_FALSE,
                Some(true) => sys::bl_OptionalBool::BL_BOOL_TRUE,
            },
        };
        let raw = get!(self.inner, |engine: sys::bl_EngineContext| {
            sys::bl_createEngine(self.raw(), &native, &engine_options, &mut engine)
        })?;
        Ok(Engine {
            raw,
            runtime: self.inner.clone(),
            target,
        })
    }
}

impl Engine {
    /// The underlying C handle, for use with [`crate::sys`].
    pub fn raw(&self) -> sys::bl_EngineContext {
        self.raw
    }

    pub fn runtime(&self) -> Runtime {
        Runtime {
            inner: self.runtime.clone(),
        }
    }

    /// Arms host-driven frames. The first [`Engine::frame`] after starting
    /// uses a 0 ms delta, as the original first animation frame does.
    pub fn start(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_startEngine(self.raw, None, ptr::null_mut()) })
    }

    pub fn stop(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_stopEngine(self.raw) })
    }

    /// Updates and draws every registered scene, then presents. Returns
    /// [`crate::Status::Stopped`] unless the engine was started.
    pub fn frame(&self, delta_ms: f64) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_frame(self.raw, delta_ms) })
    }

    /// Renders one frame with `delta_ms` regardless of the running state.
    pub fn render_frame(&self, delta_ms: f64) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_renderFrame(self.raw, delta_ms) })
    }

    /// Resizes the swap chain (physical pixels, each at least 1).
    pub fn resize(&self, width: u32, height: u32) -> Result<()> {
        let target = sys::bl_NativeTarget {
            width,
            height,
            ..self.target
        };
        self.runtime
            .call(|| unsafe { sys::bl_setNativeTarget(self.raw, &target) })
    }

    pub fn stats(&self) -> Result<EngineStats> {
        let stats = get!(self.runtime, |stats: sys::bl_EngineStats| {
            sys::bl_getEngineStats(self.raw, &mut stats)
        })?;
        Ok(EngineStats {
            draw_call_count: stats.drawCallCount,
            gpu_frame_time_ms: stats.gpuFrameTimeMs,
        })
    }

    pub fn wait_for_gpu_idle(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_waitForGpuIdle(self.raw) })
    }

    /// Disposes the engine and shuts bgfx down. Later calls return
    /// [`crate::Status::Disposed`].
    pub fn dispose(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_disposeEngine(self.raw) })
    }
}

impl PartialEq for Engine {
    fn eq(&self, other: &Self) -> bool {
        self.raw._id == other.raw._id && ptr::eq(self.raw._runtime, other.raw._runtime)
    }
}

impl Eq for Engine {}

impl fmt::Debug for Engine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Engine({:#x})", self.raw._id)
    }
}
