use crate::{Engine, Material, Result, SceneNode, Vec2, get, handle, optional_number, string, sys};

handle!(
    /// A mesh (`bl_Mesh`). Add [`Mesh::node`] to a scene to draw it.
    Mesh,
    sys::bl_Mesh
);

/// Original `createBox` options; `None` keeps the source default.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct BoxOptions {
    pub size: Option<f64>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub depth: Option<f64>,
}

/// Original `createSphere` options; `None` keeps the source default.
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct SphereOptions {
    pub segments: Option<f64>,
    pub diameter: Option<f64>,
    pub diameter_x: Option<f64>,
    pub diameter_y: Option<f64>,
    pub diameter_z: Option<f64>,
}

/// Original `createGround` options; `None` keeps the source default.
#[derive(Copy, Clone, Debug, Default)]
pub struct GroundOptions {
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub subdivisions: Option<f64>,
    pub uv_scale: Option<Vec2>,
}

/// Float32 triangle-list geometry for [`Engine::create_mesh_from_data`].
/// The data is copied.
#[derive(Copy, Clone, Debug, Default)]
pub struct MeshGeometry<'a> {
    /// XYZ per vertex.
    pub positions: &'a [f32],
    /// XYZ per vertex; same length as `positions`.
    pub normals: &'a [f32],
    pub indices: &'a [u32],
    /// XY per vertex.
    pub uvs: Option<&'a [f32]>,
    pub uvs2: Option<&'a [f32]>,
    /// XYZW per vertex.
    pub tangents: Option<&'a [f32]>,
    /// RGBA per vertex.
    pub colors: Option<&'a [f32]>,
}

fn f32_span(values: Option<&[f32]>) -> sys::bl_F32Span {
    values.map_or_else(Default::default, |v| sys::bl_F32Span {
        data: v.as_ptr(),
        count: v.len(),
    })
}

impl Engine {
    fn mesh(&self, create: impl FnOnce(&mut sys::bl_Mesh) -> sys::bl_Status) -> Result<Mesh> {
        let mut mesh = sys::bl_Mesh::default();
        self.runtime.call(|| create(&mut mesh))?;
        Ok(Mesh::wrap(&self.runtime, mesh))
    }

    pub fn create_box(&self, options: &BoxOptions) -> Result<Mesh> {
        let options = sys::bl_BoxOptions {
            size: optional_number(options.size),
            width: optional_number(options.width),
            height: optional_number(options.height),
            depth: optional_number(options.depth),
        };
        self.mesh(|mesh| unsafe { sys::bl_createBox(self.raw, &options, mesh) })
    }

    pub fn create_sphere(&self, options: &SphereOptions) -> Result<Mesh> {
        let options = sys::bl_SphereOptions {
            segments: optional_number(options.segments),
            diameter: optional_number(options.diameter),
            diameterX: optional_number(options.diameter_x),
            diameterY: optional_number(options.diameter_y),
            diameterZ: optional_number(options.diameter_z),
        };
        self.mesh(|mesh| unsafe { sys::bl_createSphere(self.raw, &options, mesh) })
    }

    pub fn create_ground(&self, options: &GroundOptions) -> Result<Mesh> {
        let options = sys::bl_GroundOptions {
            width: optional_number(options.width),
            height: optional_number(options.height),
            subdivisions: optional_number(options.subdivisions),
            hasUvScale: options.uv_scale.is_some(),
            uvScale: options.uv_scale.unwrap_or_default(),
        };
        self.mesh(|mesh| unsafe { sys::bl_createGround(self.raw, &options, mesh) })
    }

    pub fn create_mesh_from_data(&self, name: &str, geometry: &MeshGeometry<'_>) -> Result<Mesh> {
        let geometry = sys::bl_MeshGeometry {
            positions: f32_span(Some(geometry.positions)),
            normals: f32_span(Some(geometry.normals)),
            indices: sys::bl_U32Span {
                data: geometry.indices.as_ptr(),
                count: geometry.indices.len(),
            },
            uvs: f32_span(geometry.uvs),
            uvs2: f32_span(geometry.uvs2),
            tangents: f32_span(geometry.tangents),
            colors: f32_span(geometry.colors),
        };
        self.mesh(|mesh| unsafe {
            sys::bl_createMeshFromData(self.raw, string(name), &geometry, mesh)
        })
    }
}

impl Mesh {
    pub fn node(&self) -> Result<SceneNode> {
        let raw = get!(self.runtime, |n: sys::bl_SceneNode| sys::bl_meshNode(
            self.raw, &mut n
        ))?;
        Ok(SceneNode::wrap(&self.runtime, raw))
    }

    fn properties(&self) -> Result<sys::bl_MeshProperties2> {
        get!(self.runtime, |p: sys::bl_MeshProperties2| {
            sys::bl_getMeshProperties2(self.raw, &mut p)
        })
    }

    fn update(&self, change: impl FnOnce(&mut sys::bl_MeshProperties2)) -> Result<()> {
        let mut properties = self.properties()?;
        change(&mut properties);
        self.runtime
            .call(|| unsafe { sys::bl_setMeshProperties2(self.raw, &properties) })
    }

    pub fn material(&self) -> Result<Option<Material>> {
        let material = self.properties()?.material;
        Ok((material._id != 0).then(|| Material::wrap(&self.runtime, material)))
    }

    pub fn set_material(&self, material: Option<&Material>) -> Result<()> {
        self.update(|p| p.material = material.map_or_else(Default::default, |m| m.raw))
    }

    /// Original `renderOrder`; `None` is the source's `undefined`, not zero.
    pub fn render_order(&self) -> Result<Option<f64>> {
        let order = self.properties()?.renderOrder;
        Ok(order.present.then_some(order.value))
    }

    pub fn set_render_order(&self, order: Option<f64>) -> Result<()> {
        self.update(|p| p.renderOrder = optional_number(order))
    }
}
