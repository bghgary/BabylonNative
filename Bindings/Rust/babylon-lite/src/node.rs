use crate::{Mat4, Quat, Result, Vec3, copy_string, get, handle, string, sys};

handle!(
    /// A structural scene node (`bl_SceneNode`): transform, mesh, camera or
    /// light. Obtain one from a typed object's `node()`.
    SceneNode,
    sys::bl_SceneNode
);

impl SceneNode {
    pub fn name(&self) -> Result<String> {
        let name = get!(self.runtime, |name: sys::bl_String| sys::bl_getNodeName(
            self.raw, &mut name
        ))?;
        // The name is borrowed until it is replaced or the node is disposed.
        Ok(unsafe { copy_string(name) })
    }

    pub fn set_name(&self, name: &str) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setNodeName(self.raw, string(name)) })
    }

    pub fn position(&self) -> Result<Vec3> {
        get!(self.runtime, |v: sys::bl_Vec3| sys::bl_getNodePosition(
            self.raw, &mut v
        ))
    }

    pub fn set_position(&self, position: Vec3) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setNodePosition(self.raw, position) })
    }

    /// Euler XYZ rotation in radians.
    pub fn rotation(&self) -> Result<Vec3> {
        get!(self.runtime, |v: sys::bl_Vec3| sys::bl_getNodeRotation(
            self.raw, &mut v
        ))
    }

    pub fn set_rotation(&self, euler_xyz: Vec3) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setNodeRotation(self.raw, euler_xyz) })
    }

    pub fn rotation_quaternion(&self) -> Result<Quat> {
        get!(self.runtime, |q: sys::bl_Quat| {
            sys::bl_getNodeRotationQuaternion(self.raw, &mut q)
        })
    }

    pub fn set_rotation_quaternion(&self, quaternion: Quat) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setNodeRotationQuaternion(self.raw, quaternion) })
    }

    pub fn scaling(&self) -> Result<Vec3> {
        get!(self.runtime, |v: sys::bl_Vec3| sys::bl_getNodeScaling(
            self.raw, &mut v
        ))
    }

    pub fn set_scaling(&self, scaling: Vec3) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setNodeScaling(self.raw, scaling) })
    }

    pub fn parent(&self) -> Result<Option<SceneNode>> {
        let parent = get!(self.runtime, |p: sys::bl_SceneNode| sys::bl_getNodeParent(
            self.raw, &mut p
        ))?;
        Ok((parent._id != 0).then(|| SceneNode::wrap(&self.runtime, parent)))
    }

    /// Original `node.parent = parent`: sets the field only. It neither appends
    /// to the parent's children nor preserves the world transform; see
    /// [`SceneNode::set_parent_preserving_world`].
    pub fn set_parent(&self, parent: Option<&SceneNode>) -> Result<()> {
        let parent = parent.map_or_else(Default::default, |p| p.raw);
        self.runtime
            .call(|| unsafe { sys::bl_setNodeParent(self.raw, parent) })
    }

    /// Original `setParent()`: updates both sides of the link and preserves
    /// the world transform.
    pub fn set_parent_preserving_world(&self, parent: Option<&SceneNode>) -> Result<()> {
        let parent = parent.map_or_else(Default::default, |p| p.raw);
        self.runtime
            .call(|| unsafe { sys::bl_setParent(self.raw, parent) })
    }

    pub fn visible(&self) -> Result<bool> {
        get!(self.runtime, |v: bool| sys::bl_getNodeVisible(
            self.raw, &mut v
        ))
    }

    pub fn set_visible(&self, visible: bool) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setNodeVisible(self.raw, visible) })
    }

    /// Sets the visibility of this node and every descendant.
    pub fn set_subtree_visible(&self, visible: bool) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setSubtreeVisible(self.raw, visible) })
    }

    /// Column-major world matrix and its version, which changes whenever the
    /// matrix does.
    pub fn world_matrix(&self) -> Result<(Mat4, u64)> {
        let mut version = 0;
        let matrix = get!(self.runtime, |m: sys::bl_Mat4| sys::bl_getNodeWorldMatrix(
            self.raw,
            &mut m,
            &mut version
        ))?;
        Ok((matrix, version))
    }

    /// Disposes the node. Returns [`crate::Status::Busy`] while it is in a
    /// scene or hierarchy; remove it first.
    pub fn dispose(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_disposeNode(self.raw) })
    }
}
