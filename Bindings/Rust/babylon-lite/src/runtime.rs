use crate::{Error, Result, SceneNode, Status, copy_string, string, sys};
use std::cell::Cell;
use std::ffi::c_void;
use std::fmt;
use std::ptr::{self, NonNull};
use std::rc::Rc;

/// Shared owner of a native `bl_Runtime`.
pub(crate) struct RuntimeInner {
    raw: NonNull<sys::bl_Runtime>,
    /// Receives errors from the runtime's error callback; owned via `Box::into_raw`.
    last_error: *mut Cell<Option<Error>>,
}

impl RuntimeInner {
    pub(crate) fn raw(&self) -> *mut sys::bl_Runtime {
        self.raw.as_ptr()
    }

    /// Runs one C call and converts its status. Error detail is attached only
    /// when the runtime reported it during this call, never from an older error.
    pub(crate) fn call(&self, f: impl FnOnce() -> sys::bl_Status) -> Result<()> {
        let slot = unsafe { &*self.last_error };
        slot.set(None);
        let status = f();
        if status == sys::bl_Status::BL_OK {
            return Ok(());
        }
        match slot.take() {
            Some(error) if error.status == Status::from_raw(status) => Err(error),
            _ => Err(Error::from_status(status)),
        }
    }
}

impl Drop for RuntimeInner {
    fn drop(&mut self) {
        let status = unsafe { sys::bl_disposeRuntime(self.raw.as_ptr()) };
        if status == sys::bl_Status::BL_OK {
            drop(unsafe { Box::from_raw(self.last_error) });
        }
        // Otherwise the runtime still exists and may call the error callback,
        // so its slot is intentionally leaked.
    }
}

unsafe extern "C" {
    fn _aligned_malloc(size: usize, alignment: usize) -> *mut c_void;
    fn _aligned_free(memory: *mut c_void);
}

unsafe extern "C" fn allocate(_: *mut c_void, bytes: usize, alignment: usize) -> *mut c_void {
    unsafe { _aligned_malloc(bytes.max(1), alignment.max(size_of::<*mut c_void>())) }
}

unsafe extern "C" fn deallocate(_: *mut c_void, memory: *mut c_void, _: usize, _: usize) {
    unsafe { _aligned_free(memory) }
}

unsafe extern "C" fn record_error(user: *mut c_void, error: *const sys::bl_Error) {
    let slot = unsafe { &*user.cast::<Cell<Option<Error>>>() };
    let error = unsafe { &*error };
    slot.set(Some(Error {
        status: Status::from_raw(error.status),
        source_error_code: error.sourceErrorCode,
        operation: unsafe { copy_string(error.operation) },
        message: unsafe { copy_string(error.message) },
    }));
}

/// A Babylon Lite runtime: the owner of every engine, scene and resource.
///
/// Cloning shares the same runtime. It is disposed, together with everything it
/// owns, when the last `Runtime` or handle referring to it is dropped.
#[derive(Clone)]
pub struct Runtime {
    pub(crate) inner: Rc<RuntimeInner>,
}

impl Runtime {
    /// Creates a runtime with the process allocator and the runtime WGSL
    /// shader compiler (D3D11) injected.
    pub fn new() -> Result<Self> {
        let slot = Box::into_raw(Box::new(Cell::new(None::<Error>)));
        let compiler = unsafe { sys::bl_shaderCompilerService() };
        let compiler2 = unsafe { sys::bl_shaderCompilerService2() };
        let options = sys::bl_RuntimeOptions2 {
            structSize: size_of::<sys::bl_RuntimeOptions2>() as u32,
            version: sys::BL_RUNTIME_OPTIONS_VERSION_2,
            base: sys::bl_RuntimeOptions {
                allocator: sys::bl_Allocator {
                    userData: ptr::null_mut(),
                    allocate: Some(allocate),
                    deallocate: Some(deallocate),
                },
                onError: Some(record_error),
                errorUserData: slot.cast(),
                shaderCompiler: &compiler,
                audio: ptr::null(),
                io: ptr::null(),
            },
            assets: ptr::null(),
            shaderCompiler2: &compiler2,
        };
        let mut raw = ptr::null_mut();
        let status = unsafe { sys::bl_createRuntime2(&options, &mut raw) };
        match NonNull::new(raw) {
            Some(raw) if status == sys::bl_Status::BL_OK => Ok(Self {
                inner: Rc::new(RuntimeInner {
                    raw,
                    last_error: slot,
                }),
            }),
            _ => {
                drop(unsafe { Box::from_raw(slot) });
                Err(Error::from_status(status))
            }
        }
    }

    /// The underlying runtime pointer, for use with [`crate::sys`]. Valid while
    /// this value (or any handle from this runtime) lives.
    pub fn raw(&self) -> *mut sys::bl_Runtime {
        self.inner.raw()
    }

    /// Creates a transform node with the default TRS.
    pub fn create_transform_node(&self, name: &str) -> Result<SceneNode> {
        let mut node = sys::bl_TransformNode::default();
        self.inner.call(|| unsafe {
            sys::bl_createTransformNode(self.raw(), string(name), ptr::null(), &mut node)
        })?;
        Ok(SceneNode::wrap(&self.inner, node))
    }
}

impl fmt::Debug for Runtime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Runtime({:p})", self.raw())
    }
}
