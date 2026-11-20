use crate::{Result, Runtime, StandardMaterialProperties, get, handle, sys};

handle!(
    /// Any material (`bl_Material`).
    Material,
    sys::bl_Material
);
handle!(
    /// Original `StandardMaterial`.
    StandardMaterial,
    sys::bl_StandardMaterial
);

impl Runtime {
    pub fn create_standard_material(&self) -> Result<StandardMaterial> {
        let raw = get!(self.inner, |m: sys::bl_StandardMaterial| {
            sys::bl_createStandardMaterial(self.raw(), &mut m)
        })?;
        Ok(StandardMaterial::wrap(&self.inner, raw))
    }
}

impl Material {
    pub fn as_standard(&self) -> Result<StandardMaterial> {
        let raw = get!(self.runtime, |m: sys::bl_StandardMaterial| {
            sys::bl_materialAsStandardMaterial(self.raw, &mut m)
        })?;
        Ok(StandardMaterial::wrap(&self.runtime, raw))
    }

    /// Original `markMaterialUboDirty`.
    pub fn mark_dirty(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_markMaterialUboDirty(self.raw) })
    }
}

impl StandardMaterial {
    pub fn as_material(&self) -> Result<Material> {
        let raw = get!(self.runtime, |m: sys::bl_Material| {
            sys::bl_standardMaterialAsMaterial(self.raw, &mut m)
        })?;
        Ok(Material::wrap(&self.runtime, raw))
    }

    pub fn properties(&self) -> Result<StandardMaterialProperties> {
        get!(self.runtime, |p: StandardMaterialProperties| {
            sys::bl_getStandardMaterialProperties(self.raw, &mut p)
        })
    }

    pub fn set_properties(&self, properties: &StandardMaterialProperties) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setStandardMaterialProperties(self.raw, properties) })
    }

    pub fn dispose(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_disposeStandardMaterial(self.raw) })
    }
}
