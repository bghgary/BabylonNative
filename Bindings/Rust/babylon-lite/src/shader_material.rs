use crate::{
    Material, Result, Runtime, Texture2D, Vec3, get, handle, optional_number, string, sys,
};

handle!(
    /// Original `ShaderMaterial`: user WGSL with `mainVertex`/`mainFragment`.
    ShaderMaterial,
    sys::bl_ShaderMaterial
);

/// A vertex attribute a shader reads. Declaration order sets `@location`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum VertexAttribute {
    /// `vec3<f32>`
    Position,
    /// `vec3<f32>`
    Normal,
    /// `vec2<f32>`
    Uv,
    /// `vec2<f32>`
    Uv2,
    /// `vec4<f32>`
    Tangent,
    /// `vec4<f32>`
    Color,
}

/// WGSL type of a custom uniform.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum UniformType {
    F32,
    U32,
    I32,
    Vec2,
    Vec3,
    Vec4,
    Mat4,
}

/// One entry of the original `uniforms` array.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ShaderUniform<'a> {
    /// A bare system name the renderer fills per draw, such as
    /// `worldViewProjection`, `worldView`, `cameraPosition` or `alphaCutoff`.
    System(&'a str),
    /// A custom uniform in `shaderUniforms`; an empty default means zero.
    Custom {
        name: &'a str,
        ty: UniformType,
        default: &'a [f64],
    },
}

/// Original `blendMode`.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum BlendMode {
    #[default]
    Alpha,
    Additive,
}

/// Original `createShaderMaterial` options. `None` keeps the source defaults:
/// no blending or alpha test, back-face culling on, depth write unless blending.
#[derive(Copy, Clone, Debug, Default)]
pub struct ShaderMaterialOptions<'a> {
    pub name: &'a str,
    pub vertex_source: &'a str,
    pub fragment_source: &'a str,
    pub attributes: &'a [VertexAttribute],
    pub uniforms: &'a [ShaderUniform<'a>],
    /// Texture names; each `foo` declares WGSL variables `foo` and `fooSampler`.
    pub samplers: &'a [&'a str],
    pub need_alpha_blending: Option<bool>,
    pub need_alpha_testing: Option<bool>,
    pub back_face_culling: Option<bool>,
    pub depth_write: Option<bool>,
    pub blend_mode: BlendMode,
}

fn optional_bool(value: Option<bool>) -> sys::bl_OptionalBool {
    match value {
        None => sys::bl_OptionalBool::BL_BOOL_DEFAULT,
        Some(false) => sys::bl_OptionalBool::BL_BOOL_FALSE,
        Some(true) => sys::bl_OptionalBool::BL_BOOL_TRUE,
    }
}

fn attribute(attribute: VertexAttribute) -> sys::bl_VertexSemantic {
    match attribute {
        VertexAttribute::Position => sys::bl_VertexSemantic::BL_ATTRIBUTE_POSITION,
        VertexAttribute::Normal => sys::bl_VertexSemantic::BL_ATTRIBUTE_NORMAL,
        VertexAttribute::Uv => sys::bl_VertexSemantic::BL_ATTRIBUTE_UV,
        VertexAttribute::Uv2 => sys::bl_VertexSemantic::BL_ATTRIBUTE_UV2,
        VertexAttribute::Tangent => sys::bl_VertexSemantic::BL_ATTRIBUTE_TANGENT,
        VertexAttribute::Color => sys::bl_VertexSemantic::BL_ATTRIBUTE_COLOR,
    }
}

fn uniform_type(ty: UniformType) -> sys::bl_ShaderUniformType {
    match ty {
        UniformType::F32 => sys::bl_ShaderUniformType::BL_UNIFORM_F32,
        UniformType::U32 => sys::bl_ShaderUniformType::BL_UNIFORM_U32,
        UniformType::I32 => sys::bl_ShaderUniformType::BL_UNIFORM_I32,
        UniformType::Vec2 => sys::bl_ShaderUniformType::BL_UNIFORM_VEC2,
        UniformType::Vec3 => sys::bl_ShaderUniformType::BL_UNIFORM_VEC3,
        UniformType::Vec4 => sys::bl_ShaderUniformType::BL_UNIFORM_VEC4,
        UniformType::Mat4 => sys::bl_ShaderUniformType::BL_UNIFORM_MAT4,
    }
}

impl Runtime {
    /// Declares a shader material. This is data-only; WGSL compiles when a
    /// registered scene first needs the pipeline.
    pub fn create_shader_material(
        &self,
        options: &ShaderMaterialOptions<'_>,
    ) -> Result<ShaderMaterial> {
        let attributes: Vec<_> = options.attributes.iter().copied().map(attribute).collect();
        let uniforms: Vec<_> = options
            .uniforms
            .iter()
            .map(|uniform| match *uniform {
                ShaderUniform::System(name) => sys::bl_ShaderUniformDecl {
                    name: string(name),
                    system: true,
                    ..Default::default()
                },
                ShaderUniform::Custom { name, ty, default } => sys::bl_ShaderUniformDecl {
                    name: string(name),
                    type_: uniform_type(ty),
                    defaultValue: sys::bl_NumberSpan {
                        data: default.as_ptr(),
                        count: default.len(),
                    },
                    system: false,
                },
            })
            .collect();
        let samplers: Vec<_> = options
            .samplers
            .iter()
            .map(|name| sys::bl_ShaderSamplerDecl { name: string(name) })
            .collect();
        let raw_options = sys::bl_ShaderMaterialOptions {
            name: string(options.name),
            vertexSource: string(options.vertex_source),
            fragmentSource: string(options.fragment_source),
            attributes: attributes.as_ptr(),
            attributeCount: attributes.len(),
            uniforms: uniforms.as_ptr(),
            uniformCount: uniforms.len(),
            samplers: samplers.as_ptr(),
            samplerCount: samplers.len(),
            needAlphaBlending: optional_bool(options.need_alpha_blending),
            needAlphaTesting: optional_bool(options.need_alpha_testing),
            backFaceCulling: optional_bool(options.back_face_culling),
            depthWrite: optional_bool(options.depth_write),
            blendMode: match options.blend_mode {
                BlendMode::Alpha => sys::bl_BlendMode::BL_BLEND_ALPHA,
                BlendMode::Additive => sys::bl_BlendMode::BL_BLEND_ADDITIVE,
            },
            depthBias: optional_number(None),
            depthBiasSlopeScale: optional_number(None),
            ..Default::default()
        };
        let raw = get!(self.inner, |m: sys::bl_ShaderMaterial| {
            sys::bl_createShaderMaterial(self.raw(), &raw_options, &mut m)
        })?;
        Ok(ShaderMaterial::wrap(&self.inner, raw))
    }
}

impl ShaderMaterial {
    pub fn as_material(&self) -> Result<Material> {
        let raw = get!(self.runtime, |m: sys::bl_Material| {
            sys::bl_shaderMaterialAsMaterial(self.raw, &mut m)
        })?;
        Ok(Material::wrap(&self.runtime, raw))
    }

    /// Sets a custom uniform of any type; `value` must have its element count.
    pub fn set_uniform(&self, name: &str, value: &[f64]) -> Result<()> {
        let value = sys::bl_NumberSpan {
            data: value.as_ptr(),
            count: value.len(),
        };
        self.runtime
            .call(|| unsafe { sys::bl_setShaderUniform(self.raw, string(name), value) })
    }

    pub fn set_float(&self, name: &str, value: f64) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setShaderFloat(self.raw, string(name), value) })
    }

    pub fn set_vector3(&self, name: &str, value: Vec3) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setShaderVector3(self.raw, string(name), value) })
    }

    /// Binds a texture to a declared sampler; `None` clears it.
    pub fn set_texture(&self, name: &str, texture: Option<&Texture2D>) -> Result<()> {
        let texture = texture.map_or_else(Default::default, |t| t.raw);
        self.runtime
            .call(|| unsafe { sys::bl_setShaderTexture(self.raw, string(name), texture) })
    }

    /// Returns [`crate::Status::Busy`] while a live mesh uses it.
    pub fn dispose(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_disposeShaderMaterial(self.raw) })
    }
}

impl Material {
    pub fn as_shader(&self) -> Result<ShaderMaterial> {
        let raw = get!(self.runtime, |m: sys::bl_ShaderMaterial| {
            sys::bl_materialAsShaderMaterial(self.raw, &mut m)
        })?;
        Ok(ShaderMaterial::wrap(&self.runtime, raw))
    }
}
