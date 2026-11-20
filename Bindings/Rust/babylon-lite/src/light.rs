use crate::{
    DirectionalLightProperties, HemisphericLightProperties, Result, Runtime, SceneNode, Vec3, get,
    handle, optional_number, sys,
};

handle!(
    /// Any light (`bl_Light`).
    Light,
    sys::bl_Light
);
handle!(
    /// Original `HemisphericLight`.
    HemisphericLight,
    sys::bl_HemisphericLight
);
handle!(
    /// Original `DirectionalLight`.
    DirectionalLight,
    sys::bl_DirectionalLight
);

impl Runtime {
    /// `None` keeps the source defaults (direction (0, 1, 0), intensity 1).
    pub fn create_hemispheric_light(
        &self,
        direction: Option<Vec3>,
        intensity: Option<f64>,
    ) -> Result<HemisphericLight> {
        let options = sys::bl_HemisphericLightOptions {
            hasDirection: direction.is_some(),
            direction: direction.unwrap_or_default(),
            intensity: optional_number(intensity),
        };
        let raw = get!(self.inner, |light: sys::bl_HemisphericLight| {
            sys::bl_createHemisphericLight(self.raw(), &options, &mut light)
        })?;
        Ok(HemisphericLight::wrap(&self.inner, raw))
    }

    /// `intensity` of `None` keeps the source default of 1.
    pub fn create_directional_light(
        &self,
        direction: Vec3,
        intensity: Option<f64>,
    ) -> Result<DirectionalLight> {
        let options = sys::bl_DirectionalLightOptions {
            direction,
            intensity: optional_number(intensity),
        };
        let raw = get!(self.inner, |light: sys::bl_DirectionalLight| {
            sys::bl_createDirectionalLight(self.raw(), &options, &mut light)
        })?;
        Ok(DirectionalLight::wrap(&self.inner, raw))
    }
}

impl Light {
    /// The light's node; add it to a scene to light that scene.
    pub fn node(&self) -> Result<SceneNode> {
        let raw = get!(self.runtime, |n: sys::bl_SceneNode| sys::bl_lightNode(
            self.raw, &mut n
        ))?;
        Ok(SceneNode::wrap(&self.runtime, raw))
    }

    /// Original `setLightIntensity`.
    pub fn set_intensity(&self, intensity: f64) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setLightIntensity(self.raw, intensity) })
    }

    /// Original `markLightUboDirty`: required after plain diffuse, specular or
    /// intensity property writes for them to reach rendering.
    pub fn mark_dirty(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_markLightUboDirty(self.raw) })
    }

    pub fn as_directional(&self) -> Result<DirectionalLight> {
        let raw = get!(self.runtime, |l: sys::bl_DirectionalLight| {
            sys::bl_lightAsDirectionalLight(self.raw, &mut l)
        })?;
        Ok(DirectionalLight::wrap(&self.runtime, raw))
    }
}

impl HemisphericLight {
    pub fn as_light(&self) -> Result<Light> {
        let raw = get!(self.runtime, |l: sys::bl_Light| {
            sys::bl_hemisphericLightAsLight(self.raw, &mut l)
        })?;
        Ok(Light::wrap(&self.runtime, raw))
    }

    pub fn node(&self) -> Result<SceneNode> {
        self.as_light()?.node()
    }

    pub fn properties(&self) -> Result<HemisphericLightProperties> {
        get!(self.runtime, |p: HemisphericLightProperties| {
            sys::bl_getHemisphericLightProperties(self.raw, &mut p)
        })
    }

    pub fn set_properties(&self, properties: &HemisphericLightProperties) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setHemisphericLightProperties(self.raw, properties) })
    }
}

impl DirectionalLight {
    pub fn as_light(&self) -> Result<Light> {
        let raw = get!(self.runtime, |l: sys::bl_Light| {
            sys::bl_directionalLightAsLight(self.raw, &mut l)
        })?;
        Ok(Light::wrap(&self.runtime, raw))
    }

    pub fn node(&self) -> Result<SceneNode> {
        self.as_light()?.node()
    }

    pub fn properties(&self) -> Result<DirectionalLightProperties> {
        get!(self.runtime, |p: DirectionalLightProperties| {
            sys::bl_getDirectionalLightProperties(self.raw, &mut p)
        })
    }

    /// A changed direction reaches rendering immediately; other fields need
    /// [`Light::mark_dirty`], as in the original.
    pub fn set_properties(&self, properties: &DirectionalLightProperties) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_setDirectionalLightProperties(self.raw, properties) })
    }
}
