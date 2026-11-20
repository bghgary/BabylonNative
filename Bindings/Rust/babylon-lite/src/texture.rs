use crate::{Engine, Result, get, handle, sys};

handle!(
    /// A 2D texture (`bl_Texture2D`).
    Texture2D,
    sys::bl_Texture2D
);

/// Sampler address mode.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum AddressMode {
    #[default]
    ClampToEdge,
    Repeat,
    MirrorRepeat,
}

/// Sampler filter.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum FilterMode {
    #[default]
    Nearest,
    Linear,
}

/// Original `createTexture2DFromPixels` options. The defaults match the source:
/// nearest filtering, clamp-to-edge, linear (non-sRGB) data, no mipmaps.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct PixelsTexture2DOptions {
    pub address_mode_u: AddressMode,
    pub address_mode_v: AddressMode,
    pub min_filter: FilterMode,
    pub mag_filter: FilterMode,
    /// Sample with hardware sRGB-to-linear conversion.
    pub srgb: bool,
}

fn address(mode: AddressMode) -> sys::bl_AddressMode {
    match mode {
        AddressMode::ClampToEdge => sys::bl_AddressMode::BL_ADDRESS_CLAMP_TO_EDGE,
        AddressMode::Repeat => sys::bl_AddressMode::BL_ADDRESS_REPEAT,
        AddressMode::MirrorRepeat => sys::bl_AddressMode::BL_ADDRESS_MIRROR_REPEAT,
    }
}

fn filter(mode: FilterMode) -> sys::bl_FilterMode {
    match mode {
        FilterMode::Nearest => sys::bl_FilterMode::BL_FILTER_NEAREST,
        FilterMode::Linear => sys::bl_FilterMode::BL_FILTER_LINEAR,
    }
}

/// Texture size and color space.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct Texture2DInfo {
    pub width: u32,
    pub height: u32,
    pub srgb: bool,
}

impl Engine {
    /// Creates a texture from tightly packed, top-to-bottom, straight-alpha
    /// RGBA8 pixels (`width * height * 4` bytes). The pixels are copied.
    pub fn create_texture_2d_from_pixels(
        &self,
        rgba8: &[u8],
        width: u32,
        height: u32,
        options: &PixelsTexture2DOptions,
    ) -> Result<Texture2D> {
        let options = sys::bl_PixelsTexture2DOptions {
            addressModeU: address(options.address_mode_u),
            addressModeV: address(options.address_mode_v),
            minFilter: filter(options.min_filter),
            magFilter: filter(options.mag_filter),
            srgb: options.srgb,
        };
        let pixels = sys::bl_Bytes {
            data: rgba8.as_ptr(),
            count: rgba8.len(),
        };
        let raw = get!(self.runtime, |t: sys::bl_Texture2D| {
            sys::bl_createTexture2DFromPixels(self.raw, pixels, width, height, &options, &mut t)
        })?;
        Ok(Texture2D::wrap(&self.runtime, raw))
    }

    /// Replaces a `width` x `height` region at (`x`, `y`) with RGBA8 pixels.
    pub fn update_texture_2d_from_pixels(
        &self,
        texture: &Texture2D,
        rgba8: &[u8],
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let pixels = sys::bl_Bytes {
            data: rgba8.as_ptr(),
            count: rgba8.len(),
        };
        self.runtime.call(|| unsafe {
            sys::bl_updateTexture2DFromPixels(self.raw, texture.raw, pixels, x, y, width, height)
        })
    }
}

impl Texture2D {
    pub fn info(&self) -> Result<Texture2DInfo> {
        let info = get!(self.runtime, |i: sys::bl_Texture2DInfo| {
            sys::bl_getTexture2DInfo(self.raw, &mut i)
        })?;
        Ok(Texture2DInfo {
            width: info.width,
            height: info.height,
            srgb: info.srgb,
        })
    }

    /// Returns [`crate::Status::Busy`] while a material still refers to it.
    pub fn dispose(&self) -> Result<()> {
        self.runtime
            .call(|| unsafe { sys::bl_disposeTexture2D(self.raw) })
    }
}
