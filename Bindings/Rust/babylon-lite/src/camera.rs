use crate::{
    ArcRotateCameraProperties, CameraProperties, Result, Runtime, Scene, SceneNode, Vec3, get,
    handle, sys,
};

handle!(
    /// Any camera (`bl_Camera`); convert to a concrete family with `as_*`.
    Camera,
    sys::bl_Camera
);
handle!(
    /// Original `ArcRotateCamera`.
    ArcRotateCamera,
    sys::bl_ArcRotateCamera
);
handle!(
    /// Original `FreeCamera`.
    FreeCamera,
    sys::bl_FreeCamera
);
handle!(
    /// Pointer/wheel input attached to an [`ArcRotateCamera`].
    ArcRotateControl,
    sys::bl_ArcRotateControl
);

/// What the host should do with the input event that produced these effects.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PointerEffects {
    pub prevent_default: bool,
    pub capture_pointer: bool,
    pub release_pointer: bool,
    pub pointer_id: i64,
}

impl Runtime {
    pub fn create_arc_rotate_camera(
        &self,
        alpha: f64,
        beta: f64,
        radius: f64,
        target: Vec3,
    ) -> Result<ArcRotateCamera> {
        let raw = get!(self.inner, |camera: sys::bl_ArcRotateCamera| {
            sys::bl_createArcRotateCamera(self.raw(), alpha, beta, radius, target, &mut camera)
        })?;
        Ok(ArcRotateCamera::wrap(&self.inner, raw))
    }

    pub fn create_free_camera(&self, position: Vec3, target: Vec3) -> Result<FreeCamera> {
        let raw = get!(self.inner, |camera: sys::bl_FreeCamera| {
            sys::bl_createFreeCamera(self.raw(), position, target, &mut camera)
        })?;
        Ok(FreeCamera::wrap(&self.inner, raw))
    }
}

impl Camera {
    pub fn as_arc_rotate(&self) -> Result<ArcRotateCamera> {
        let raw = get!(self.runtime, |c: sys::bl_ArcRotateCamera| {
            sys::bl_cameraAsArcRotateCamera(self.raw, &mut c)
        })?;
        Ok(ArcRotateCamera::wrap(&self.runtime, raw))
    }

    pub fn as_free(&self) -> Result<FreeCamera> {
        let raw = get!(self.runtime, |c: sys::bl_FreeCamera| {
            sys::bl_cameraAsFreeCamera(self.raw, &mut c)
        })?;
        Ok(FreeCamera::wrap(&self.runtime, raw))
    }
}

impl ArcRotateCamera {
    pub fn as_camera(&self) -> Result<Camera> {
        let raw = get!(self.runtime, |c: sys::bl_Camera| {
            sys::bl_arcRotateCameraAsCamera(self.raw, &mut c)
        })?;
        Ok(Camera::wrap(&self.runtime, raw))
    }

    /// The camera's node. Its position, rotation and scaling cannot be set;
    /// change the orbit through [`ArcRotateCamera::set_properties`].
    pub fn node(&self) -> Result<SceneNode> {
        let raw = get!(self.runtime, |n: sys::bl_SceneNode| {
            sys::bl_arcRotateCameraNode(self.raw, &mut n)
        })?;
        Ok(SceneNode::wrap(&self.runtime, raw))
    }

    pub fn properties(&self) -> Result<ArcRotateCameraProperties> {
        get!(self.runtime, |p: ArcRotateCameraProperties| {
            sys::bl_getArcRotateCameraProperties(self.raw, &mut p)
        })
    }

    pub fn set_properties(&self, properties: &ArcRotateCameraProperties) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setArcRotateCameraProperties(self.raw, properties) })
    }

    /// Attaches the original pointer/wheel/pinch controls. With a scene, the
    /// control applies inertia once per frame of that scene.
    pub fn attach_control(&self, scene: Option<&Scene>) -> Result<ArcRotateControl> {
        let scene = scene.map_or_else(Default::default, |s| s.raw);
        let raw = get!(self.runtime, |c: sys::bl_ArcRotateControl| {
            sys::bl_attachControl(self.raw, scene, std::ptr::null(), &mut c)
        })?;
        Ok(ArcRotateControl::wrap(&self.runtime, raw))
    }
}

impl ArcRotateControl {
    fn process(&self, input: &sys::bl_ArcRotateInput) -> Result<PointerEffects> {
        let effects = get!(self.runtime, |e: sys::bl_ArcRotateInputEffects| {
            sys::bl_processArcRotateInput(self.raw, input, &mut e)
        })?;
        Ok(PointerEffects {
            prevent_default: effects.preventDefault,
            capture_pointer: effects.capturePointer,
            release_pointer: effects.releasePointer,
            pointer_id: effects.pointerId,
        })
    }

    fn mouse(
        kind: sys::bl_ArcRotateInputKind,
        pointer_id: i64,
        button: u32,
        x: f64,
        y: f64,
    ) -> sys::bl_ArcRotateInput {
        sys::bl_ArcRotateInput {
            kind,
            pointerType: sys::bl_ArcRotatePointerType::BL_ARC_POINTER_MOUSE,
            pointerId: pointer_id,
            button,
            clientX: x,
            clientY: y,
            ..Default::default()
        }
    }

    /// Mouse button press. `x`/`y` are client coordinates in CSS (logical)
    /// pixels; `button` is 0 left, 1 middle, 2 right as in DOM events.
    pub fn pointer_down(
        &self,
        pointer_id: i64,
        button: u32,
        x: f64,
        y: f64,
    ) -> Result<PointerEffects> {
        self.process(&Self::mouse(
            sys::bl_ArcRotateInputKind::BL_ARC_POINTER_DOWN,
            pointer_id,
            button,
            x,
            y,
        ))
    }

    pub fn pointer_move(&self, pointer_id: i64, x: f64, y: f64) -> Result<PointerEffects> {
        self.process(&Self::mouse(
            sys::bl_ArcRotateInputKind::BL_ARC_POINTER_MOVE,
            pointer_id,
            0,
            x,
            y,
        ))
    }

    pub fn pointer_up(
        &self,
        pointer_id: i64,
        button: u32,
        x: f64,
        y: f64,
    ) -> Result<PointerEffects> {
        self.process(&Self::mouse(
            sys::bl_ArcRotateInputKind::BL_ARC_POINTER_UP,
            pointer_id,
            button,
            x,
            y,
        ))
    }

    /// Mouse wheel with the raw DOM `deltaY` (positive scrolls down/zooms out).
    pub fn wheel(&self, delta_y: f64) -> Result<PointerEffects> {
        let input = sys::bl_ArcRotateInput {
            kind: sys::bl_ArcRotateInputKind::BL_ARC_WHEEL,
            deltaY: delta_y,
            ..Default::default()
        };
        self.process(&input)
    }

    /// Detaches the control. Detaching twice is allowed.
    pub fn detach(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_detachControl(self.raw) })
    }
}

impl FreeCamera {
    pub fn as_camera(&self) -> Result<Camera> {
        let raw = get!(
            self.runtime,
            |c: sys::bl_Camera| sys::bl_freeCameraAsCamera(self.raw, &mut c)
        )?;
        Ok(Camera::wrap(&self.runtime, raw))
    }

    pub fn node(&self) -> Result<SceneNode> {
        let raw = get!(self.runtime, |n: sys::bl_SceneNode| sys::bl_cameraNode(
            self.raw, &mut n
        ))?;
        Ok(SceneNode::wrap(&self.runtime, raw))
    }

    pub fn properties(&self) -> Result<CameraProperties> {
        get!(self.runtime, |p: CameraProperties| {
            sys::bl_getCameraProperties(self.raw, &mut p)
        })
    }

    pub fn set_properties(&self, properties: &CameraProperties) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setCameraProperties(self.raw, properties) })
    }

    pub fn target(&self) -> Result<Vec3> {
        get!(self.runtime, |t: sys::bl_Vec3| sys::bl_getCameraTarget(
            self.raw, &mut t
        ))
    }

    pub fn set_target(&self, target: Vec3) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setCameraTarget(self.raw, target) })
    }
}
