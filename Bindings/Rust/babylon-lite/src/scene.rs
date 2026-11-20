use crate::{ArcRotateCamera, Camera, Color4, Engine, Result, SceneNode, get, handle, sys};

handle!(
    /// A scene (`bl_SceneContext`). Scenes draw only after [`Scene::register`].
    Scene,
    sys::bl_SceneContext
);

impl Engine {
    pub fn create_scene(&self) -> Result<Scene> {
        let raw = get!(self.runtime, |scene: sys::bl_SceneContext| {
            sys::bl_createSceneContext(self.raw, &mut scene)
        })?;
        Ok(Scene::wrap(&self.runtime, raw))
    }
}

impl Scene {
    /// Adds a mesh, transform, camera or light node and its children. As in
    /// the original `meshes.push`, adding the same node twice adds it twice.
    pub fn add(&self, node: &SceneNode) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_addToScene(self.raw, node.raw) })
    }

    /// Removes a node and its children. Removing a mesh from its last scene
    /// permanently retires it.
    pub fn remove(&self, node: &SceneNode) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_removeFromScene(self.raw, node.raw) })
    }

    /// Prepares shaders and pipelines and adds the scene to the engine's draw order.
    pub fn register(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_registerScene(self.raw) })
    }

    pub fn unregister(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_unregisterScene(self.raw) })
    }

    pub fn dispose(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_disposeScene(self.raw) })
    }

    fn properties(&self) -> Result<sys::bl_SceneProperties2> {
        get!(self.runtime, |p: sys::bl_SceneProperties2| {
            sys::bl_getSceneProperties2(self.raw, &mut p)
        })
    }

    fn update(&self, change: impl FnOnce(&mut sys::bl_SceneProperties2)) -> Result<()> {
        let mut properties = self.properties()?;
        change(&mut properties);
        self.runtime
            .call(|| unsafe { sys::bl_setSceneProperties2(self.raw, &properties) })
    }

    pub fn clear_color(&self) -> Result<Color4> {
        Ok(self.properties()?.clearColor)
    }

    pub fn set_clear_color(&self, color: Color4) -> Result<()> {
        self.update(|p| p.clearColor = color)
    }

    /// The active camera, if any.
    pub fn camera(&self) -> Result<Option<Camera>> {
        let camera = self.properties()?.camera;
        Ok((camera._id != 0).then(|| Camera::wrap(&self.runtime, camera)))
    }

    pub fn set_camera(&self, camera: Option<&Camera>) -> Result<()> {
        self.update(|p| p.camera = camera.map_or_else(Default::default, |c| c.raw))
    }

    /// A positive value overrides the frame delta for this scene only.
    pub fn fixed_delta_ms(&self) -> Result<f64> {
        Ok(self.properties()?.fixedDeltaMs)
    }

    pub fn set_fixed_delta_ms(&self, delta_ms: f64) -> Result<()> {
        self.update(|p| p.fixedDeltaMs = delta_ms)
    }

    /// Creates the original default arc-rotate camera framing the scene's
    /// content and makes it the active camera.
    pub fn create_default_camera(&self) -> Result<ArcRotateCamera> {
        let raw = get!(self.runtime, |camera: sys::bl_ArcRotateCamera| {
            sys::bl_createDefaultCamera(self.raw, &mut camera)
        })?;
        Ok(ArcRotateCamera::wrap(&self.runtime, raw))
    }
}
