//! Port of `voxel-material.ts`: the atlas-sampling voxel surface material with
//! baked per-vertex light in `color` (r = ambient occlusion, g = skylight,
//! b = blocklight, a = fluid/sway flag), dynamic sun/ambient, torch tint and fog.

use babylon_lite::{
    Result, Runtime, ShaderMaterial, ShaderMaterialOptions, ShaderUniform, Texture2D, UniformType,
    VertexAttribute, vec3,
};

pub type Rgb = [f64; 3];

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Mode {
    Opaque,
    Cutout,
    Blend,
}

fn vertex_source(mode: Mode) -> String {
    let deform = match mode {
        Mode::Blend => {
            "// Drop the fluid (water) surface slightly and ripple it for a liquid feel.
  // Gated by the per-vertex fluid flag so solid translucent blocks (glass, ice)
  // stay perfectly still.
  let fluid = input.color.a;
  let wave = sin(p.x * 0.6 + shaderUniforms.uTime * 1.6) * 0.5 + sin(p.z * 0.7 + shaderUniforms.uTime * 1.3) * 0.5;
  p.y = p.y + (-0.12 + wave * 0.05) * fluid;"
        }
        Mode::Cutout => {
            "// Gentle wind sway for foliage (leaves), gated by the per-vertex sway flag so
  // solid cutout blocks (cactus) stay rigid.
  let sway = input.color.a;
  let t = shaderUniforms.uTime;
  let sx = sin(p.x * 0.5 + p.y * 0.3 + t * 1.3) + sin(p.z * 0.7 + t * 0.9);
  let sz = sin(p.z * 0.5 + p.y * 0.3 + t * 1.1) + sin(p.x * 0.6 + t * 0.7);
  p.x = p.x + sx * 0.045 * sway;
  p.z = p.z + sz * 0.045 * sway;"
        }
        Mode::Opaque => "",
    };
    format!(
        "struct VertexOutput {{
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) ao: vec3<f32>,
  @location(2) viewDepth: f32,
  @location(3) worldPos: vec3<f32>,
  @location(4) normal: vec3<f32>,
  @location(5) fluid: f32,
}};
@vertex fn mainVertex(input: VertexInput) -> VertexOutput {{
  var out: VertexOutput;
  var p = input.position;
  {deform}
  out.position = shaderSystem.worldViewProjection * vec4<f32>(p, 1.0);
  out.viewDepth = (shaderSystem.worldView * vec4<f32>(p, 1.0)).z;
  out.uv = input.uv;
  out.ao = input.color.rgb;
  out.worldPos = p;
  out.normal = input.normal;
  out.fluid = input.color.a;
  return out;
}}"
    )
}

const FACE_SHADE_FN: &str = "fn mcFaceShade(n: vec3<f32>) -> f32 {
  if (n.y > 0.5) { return 1.0; }
  if (n.y < -0.5) { return 0.5; }
  if (abs(n.x) > 0.5) { return 0.8; }
  return 0.7;
}";

fn fragment_source(mode: Mode) -> String {
    let alpha_test = if mode == Mode::Cutout {
        "if (tex.a < 0.5) { discard; }"
    } else {
        ""
    };
    let out_color = if mode == Mode::Blend {
        "let shimmer = 1.0 - input.fluid * (0.08 - 0.08 * sin(input.worldPos.x * 0.8 + input.worldPos.z * 0.8 + shaderUniforms.uTime * 2.0));
  var surf = tex.rgb;
  var outA = tex.a;
  if (input.fluid > 0.5) {
    // Depth-darkened water: shallows stay bright, deeps turn a richer blue, and
    // get a touch more opaque so deep water reads as a real body of water.
    let depth = clamp((30.0 - input.worldPos.y) / 9.0, 0.0, 1.0);
    surf = mix(surf, surf * vec3<f32>(0.35, 0.5, 0.7), depth * 0.8);
    outA = clamp(outA + depth * 0.18, 0.0, 1.0);
    // Animated ripple normal for moving sun sparkle on the surface.
    let t = shaderUniforms.uTime;
    let nx = 0.35 * sin(input.worldPos.x * 1.7 + t * 2.3) + 0.25 * sin(input.worldPos.z * 2.3 - t * 1.7);
    let nz = 0.35 * sin(input.worldPos.z * 1.9 + t * 2.1) + 0.25 * sin(input.worldPos.x * 2.1 + t * 1.3);
    let rn = normalize(vec3<f32>(nx, 4.0, nz));
    let hvec = normalize(normalize(shaderUniforms.sunDir) + vec3<f32>(0.0, 1.0, 0.0));
    let spec = pow(max(dot(rn, hvec), 0.0), 48.0);
    let sunUpW = smoothstep(-0.04, 0.12, shaderUniforms.sunDir.y);
    surf = surf + shaderUniforms.sunColor * spec * 1.6 * sunUpW;
    // Fresnel sky reflection: at grazing angles the surface mirrors the sky
    // horizon colour, giving water a reflective sheen that strengthens with the
    // ripple normal. Brighter by day, subdued at night.
    let viewDir = normalize(shaderUniforms.cameraPos - input.worldPos);
    let fres = pow(1.0 - clamp(dot(rn, viewDir), 0.0, 1.0), 5.0);
    let skyRefl = mix(shaderUniforms.fogColor, vec3<f32>(0.55, 0.7, 0.95), 0.35) * (0.35 + 0.65 * sunUpW);
    surf = mix(surf, skyRefl, fres * 0.5);
    outA = clamp(outA + fres * 0.22, 0.0, 1.0);
  }
  let lit = surf * input.ao.r * irradiance * shimmer;
  let foggy = mix(lit, shaderUniforms.fogColor, fogAmt);
  return vec4<f32>(foggy, outA * shaderUniforms.alpha);"
    } else {
        "let lit = tex.rgb * input.ao.r * irradiance;
  let foggy = mix(lit, shaderUniforms.fogColor, fogAmt);
  return vec4<f32>(foggy, 1.0);"
    };
    format!(
        "{FACE_SHADE_FN}
struct VertexOutput {{
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
  @location(1) ao: vec3<f32>,
  @location(2) viewDepth: f32,
  @location(3) worldPos: vec3<f32>,
  @location(4) normal: vec3<f32>,
  @location(5) fluid: f32,
}};
@fragment fn mainFragment(input: VertexOutput) -> @location(0) vec4<f32> {{
  let tex = textureSample(atlasTex, atlasTexSampler, input.uv);
  {alpha_test}
  let fogStart = shaderUniforms.fogParams.x;
  let fogEnd = shaderUniforms.fogParams.y;
  let fog = clamp((input.viewDepth - fogStart) / (fogEnd - fogStart), 0.0, 1.0);
  // Atmospheric height-haze: low-lying distant terrain (valleys, shorelines, water)
  // gains a little extra aerial perspective, deepening the sense of distance without
  // washing out nearby blocks.
  let haze = clamp((36.0 - input.worldPos.y) / 26.0, 0.0, 1.0)
           * clamp((input.viewDepth - fogStart * 0.4) / (fogEnd - fogStart * 0.4), 0.0, 1.0) * 0.22;
  let fogAmt = clamp(fog + haze, 0.0, 1.0);
  // Directional sun (gated by sun altitude) + face-tinted ambient sky light,
  // both scaled by per-vertex skylight visibility, plus warm blocklight (torches)
  // and a tiny floor so fully-enclosed dug-out areas read near-black.
  let n = normalize(input.normal);
  let L = normalize(shaderUniforms.sunDir);
  let sunUp = smoothstep(-0.04, 0.12, L.y);
  let ndl = max(dot(n, L), 0.0) * sunUp;
  let ambientFace = mix(1.0, mcFaceShade(n), 0.55);
  // Shape the skylight so it falls off fast: a covered tunnel that loses a few
  // levels of skylight goes genuinely dark instead of merely dim (cube curve),
  // while fully sky-lit surfaces (skyVis == 1) stay at full brightness.
  let skyVis = input.ao.g;
  let skyShaped = skyVis * skyVis * skyVis;
  let blockVis = input.ao.b;
  let skyTerm = (shaderUniforms.ambientColor * ambientFace + shaderUniforms.sunColor * ndl) * skyShaped;
  let torchTerm = vec3<f32>(1.5, 0.86, 0.42) * (blockVis * blockVis);
  let irradiance = skyTerm + torchTerm + vec3<f32>(0.006, 0.007, 0.011);
  {out_color}
}}"
    )
}

pub struct VoxelMaterialOptions {
    pub fog_color: Rgb,
    pub fog_start: f64,
    pub fog_end: f64,
    /// Blend-mode constant alpha multiplier (water/glass translucency).
    pub alpha: f64,
    pub ambient_color: Rgb,
    pub sun_color: Rgb,
    /// Direction to the sun (need not be normalised).
    pub sun_dir: Rgb,
}

impl Default for VoxelMaterialOptions {
    fn default() -> Self {
        Self {
            fog_color: [0.62, 0.74, 0.86],
            fog_start: 40.0,
            fog_end: 120.0,
            alpha: 1.0,
            ambient_color: [0.45, 0.48, 0.55],
            sun_color: [0.55, 0.5, 0.42],
            sun_dir: [0.45, 0.8, 0.35],
        }
    }
}

pub fn v3(c: Rgb) -> babylon_lite::Vec3 {
    vec3(c[0], c[1], c[2])
}

pub fn create_voxel_material(
    runtime: &Runtime,
    name: &str,
    atlas: &Texture2D,
    mode: Mode,
    o: &VoxelMaterialOptions,
) -> Result<ShaderMaterial> {
    let vertex = vertex_source(mode);
    let fragment = fragment_source(mode);
    let fog_params = [o.fog_start, o.fog_end];
    let alpha = [o.alpha];
    let material = runtime.create_shader_material(&ShaderMaterialOptions {
        name,
        vertex_source: &vertex,
        fragment_source: &fragment,
        attributes: &[
            VertexAttribute::Position,
            VertexAttribute::Normal,
            VertexAttribute::Uv,
            VertexAttribute::Color,
        ],
        uniforms: &[
            ShaderUniform::System("worldViewProjection"),
            ShaderUniform::System("worldView"),
            ShaderUniform::Custom {
                name: "fogColor",
                ty: UniformType::Vec3,
                default: &o.fog_color,
            },
            ShaderUniform::Custom {
                name: "fogParams",
                ty: UniformType::Vec2,
                default: &fog_params,
            },
            ShaderUniform::Custom {
                name: "uTime",
                ty: UniformType::F32,
                default: &[0.0],
            },
            ShaderUniform::Custom {
                name: "alpha",
                ty: UniformType::F32,
                default: &alpha,
            },
            ShaderUniform::Custom {
                name: "ambientColor",
                ty: UniformType::Vec3,
                default: &o.ambient_color,
            },
            ShaderUniform::Custom {
                name: "sunColor",
                ty: UniformType::Vec3,
                default: &o.sun_color,
            },
            ShaderUniform::Custom {
                name: "sunDir",
                ty: UniformType::Vec3,
                default: &o.sun_dir,
            },
            ShaderUniform::Custom {
                name: "cameraPos",
                ty: UniformType::Vec3,
                default: &[0.0, 0.0, 0.0],
            },
        ],
        samplers: &["atlasTex"],
        back_face_culling: Some(mode != Mode::Blend),
        depth_write: Some(mode != Mode::Blend),
        need_alpha_blending: Some(mode == Mode::Blend),
        ..Default::default()
    })?;
    material.set_texture("atlasTex", Some(atlas))?;
    material.set_vector3("fogColor", v3(o.fog_color))?;
    material.set_vector3("ambientColor", v3(o.ambient_color))?;
    material.set_vector3("sunColor", v3(o.sun_color))?;
    material.set_vector3("sunDir", v3(o.sun_dir))?;
    material.set_float("alpha", o.alpha)?;
    Ok(material)
}
