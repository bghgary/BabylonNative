//! Safe Rust projection of the Babylon Lite native API (`babylon_lite.h`).
//!
//! The C API's handles are generation-checked identities, so a handle to an
//! object that was explicitly disposed reports [`Status::Disposed`] instead of
//! touching freed memory. The only lifetime the C API cannot check is the
//! runtime's own, so every handle here keeps its [`Runtime`] alive; the native
//! runtime is disposed when the last handle referencing it is dropped.
//!
//! All Babylon Lite calls are bound to the creating thread, which is why none of
//! these types are `Send` or `Sync`.
//!
//! Only part of the API has a safe wrapper so far. Every handle exposes `raw()`,
//! and the complete C API is available through [`sys`] for everything else.

pub use babylon_lite_sys as sys;

mod camera;
mod engine;
mod error;
mod light;
mod material;
mod mesh;
mod node;
mod runtime;
mod scene;
mod shader_material;
mod texture;

pub use camera::{ArcRotateCamera, ArcRotateControl, Camera, FreeCamera, PointerEffects};
pub use engine::{Engine, EngineOptions, EngineStats};
pub use error::{Error, Result, Status};
pub use light::{DirectionalLight, HemisphericLight, Light};
pub use material::{Material, StandardMaterial};
pub use mesh::{BoxOptions, GroundOptions, Mesh, MeshGeometry, SphereOptions};
pub use node::SceneNode;
pub use runtime::Runtime;
pub use scene::Scene;
pub use shader_material::{
    BlendMode, ShaderMaterial, ShaderMaterialOptions, ShaderUniform, UniformType, VertexAttribute,
};
pub use texture::{AddressMode, FilterMode, PixelsTexture2DOptions, Texture2D, Texture2DInfo};

/// Plain-old-data types shared with the C API.
pub type Vec2 = sys::bl_Vec2;
pub type Vec3 = sys::bl_Vec3;
pub type Vec4 = sys::bl_Vec4;
pub type Quat = sys::bl_Quat;
pub type Color4 = sys::bl_Color4;
pub type Mat4 = sys::bl_Mat4;
pub type ArcRotateCameraProperties = sys::bl_ArcRotateCameraProperties;
pub type CameraProperties = sys::bl_CameraProperties;
pub type HemisphericLightProperties = sys::bl_HemisphericLightProperties;
pub type DirectionalLightProperties = sys::bl_DirectionalLightProperties;
pub type StandardMaterialProperties = sys::bl_StandardMaterialProperties;

/// Shorthand for a [`Vec3`].
pub const fn vec3(x: f64, y: f64, z: f64) -> Vec3 {
    Vec3 { x, y, z }
}

/// Shorthand for a [`Color4`].
pub const fn color4(r: f64, g: f64, b: f64, a: f64) -> Color4 {
    Color4 { r, g, b, a }
}

pub(crate) fn optional_number(value: Option<f64>) -> sys::bl_OptionalNumber {
    sys::bl_OptionalNumber {
        present: value.is_some(),
        value: value.unwrap_or(0.0),
    }
}

pub(crate) fn string(value: &str) -> sys::bl_String {
    sys::bl_String {
        data: value.as_ptr().cast(),
        length: value.len(),
    }
}

/// Copies a borrowed C string view.
///
/// # Safety
/// `value` must describe `length` readable bytes, or be null with length 0.
pub(crate) unsafe fn copy_string(value: sys::bl_String) -> String {
    if value.data.is_null() || value.length == 0 {
        return String::new();
    }
    let bytes = unsafe { std::slice::from_raw_parts(value.data.cast::<u8>(), value.length) };
    String::from_utf8_lossy(bytes).into_owned()
}

/// Declares a typed handle that keeps its runtime alive.
macro_rules! handle {
    ($(#[$meta:meta])* $name:ident, $raw:ty) => {
        $(#[$meta])*
        #[derive(Clone)]
        pub struct $name {
            pub(crate) raw: $raw,
            pub(crate) runtime: std::rc::Rc<crate::runtime::RuntimeInner>,
        }

        impl $name {
            pub(crate) fn wrap(runtime: &std::rc::Rc<crate::runtime::RuntimeInner>, raw: $raw) -> Self {
                Self { raw, runtime: runtime.clone() }
            }

            /// The underlying C handle, for use with [`crate::sys`]. It is valid
            /// only while this value (or another handle to the same runtime) lives.
            pub fn raw(&self) -> $raw {
                self.raw
            }

            /// The runtime that owns this object.
            pub fn runtime(&self) -> crate::Runtime {
                crate::Runtime { inner: self.runtime.clone() }
            }
        }

        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.raw._id == other.raw._id && std::ptr::eq(self.raw._runtime, other.raw._runtime)
            }
        }

        impl Eq for $name {}

        impl std::hash::Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.raw._id.hash(state);
                std::ptr::hash(self.raw._runtime, state);
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, concat!(stringify!($name), "({:#x})"), self.raw._id)
            }
        }
    };
}
pub(crate) use handle;

/// Runs a C getter that fills `T` on success.
macro_rules! get {
    ($runtime:expr, |$out:ident: $ty:ty| $call:expr) => {{
        let mut $out: $ty = Default::default();
        $runtime.call(|| unsafe { $call }).map(|()| $out)
    }};
}
pub(crate) use get;
