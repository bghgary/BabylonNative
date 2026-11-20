//! Port of `sky.ts`: a camera-centered sphere pushed to the reversed-Z far
//! plane, with a horizon-to-zenith gradient, sun glow, moon and stars.

use crate::voxel_material::{Rgb, v3};
use babylon_lite::{
    Engine, Mesh, Result, Runtime, ShaderMaterial, ShaderMaterialOptions, ShaderUniform,
    SphereOptions, UniformType, VertexAttribute,
};

const VERTEX: &str = "struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) dir: vec3<f32>,
};
@vertex fn mainVertex(input: VertexInput) -> VertexOutput {
  var out: VertexOutput;
  let clip = shaderSystem.worldViewProjection * vec4<f32>(input.position, 1.0);
  out.position = vec4<f32>(clip.xy, 0.0, clip.w);
  out.dir = input.position;
  return out;
}";

const FRAGMENT: &str = "struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) dir: vec3<f32>,
};
fn hash3(p: vec3<f32>) -> f32 {
  var q = fract(p * 0.3183099 + vec3<f32>(0.1, 0.2, 0.3));
  q = q * 17.0;
  return fract(q.x * q.y * q.z * (q.x + q.y + q.z));
}
// Sparse twinkling star field in the upper sky.
fn starField(d: vec3<f32>) -> f32 {
  let p = d * 90.0;
  let cell = floor(p);
  let f = fract(p) - 0.5;
  let r = hash3(cell);
  let present = step(0.986, r);
  let dist = length(f);
  let b = smoothstep(0.10, 0.0, dist);
  return present * b * (0.5 + 0.7 * hash3(cell + 7.0));
}
@fragment fn mainFragment(input: VertexOutput) -> @location(0) vec4<f32> {
  let d = normalize(input.dir);
  let h = clamp(d.y, 0.0, 1.0);
  let horizon = shaderUniforms.horizonColor;
  let zenith = shaderUniforms.zenithColor;
  var col = mix(horizon, zenith, pow(h, 0.6));
  let sd = normalize(shaderUniforms.sunDir);
  let sunUp = smoothstep(-0.06, 0.10, sd.y);
  // How deep into night we are (sun well below the horizon).
  let night = smoothstep(0.07, -0.12, sd.y);

  // Stars: only the upper hemisphere, fading in at night.
  let stars = starField(d) * night * smoothstep(0.0, 0.15, d.y);
  col = col + vec3<f32>(0.9, 0.93, 1.0) * stars * 1.3;

  // Moon opposite the sun: a soft halo plus a crisp bright disc, night only.
  let md = -sd;
  let moonUp = smoothstep(-0.06, 0.10, md.y);
  let m = max(dot(d, md), 0.0);
  col = col + vec3<f32>(0.55, 0.62, 0.8) * pow(m, 8.0) * 0.12 * moonUp * night;
  let moonDisc = smoothstep(0.9988, 0.9992, m) * moonUp * night;
  // Faint mare shading so the disc reads as a moon, not a blank dot.
  let moonShade = 0.82 + 0.18 * hash3(floor(d * 200.0));
  col = mix(col, vec3<f32>(0.95, 0.95, 0.88) * moonShade, moonDisc);

  // Sun: a purely additive glow that smoothly saturates to a white core, so there
  // is no distinct inner disc — just one soft, bright sun. The center blows out to
  // white naturally (LDR clamp) while the warm tint shows through the mid-glow.
  let s = max(dot(d, sd), 0.0);
  col = col + vec3<f32>(1.0, 0.9, 0.7) * pow(s, 8.0) * 0.16 * sunUp;
  col = col + vec3<f32>(1.0, 0.95, 0.82) * pow(s, 64.0) * 0.7 * sunUp;
  col = col + vec3<f32>(1.0, 0.98, 0.92) * pow(s, 320.0) * 2.2 * sunUp;
  return vec4<f32>(col, 1.0);
}";

pub struct Sky {
    pub mesh: Mesh,
    material: ShaderMaterial,
}

impl Sky {
    pub fn new(runtime: &Runtime, engine: &Engine, horizon: Rgb) -> Result<Self> {
        let zenith = [0.28, 0.5, 0.86];
        let sun_dir = [0.5, 0.6, 0.3];
        // createSphereData + createMeshFromData in the original; createSphere
        // generates the same geometry.
        let mesh = engine.create_sphere(&SphereOptions {
            segments: Some(16.0),
            diameter: Some(4000.0),
            ..Default::default()
        })?;
        let material = runtime.create_shader_material(&ShaderMaterialOptions {
            name: "mcSky",
            vertex_source: VERTEX,
            fragment_source: FRAGMENT,
            attributes: &[VertexAttribute::Position],
            uniforms: &[
                ShaderUniform::System("worldViewProjection"),
                ShaderUniform::Custom {
                    name: "horizonColor",
                    ty: UniformType::Vec3,
                    default: &horizon,
                },
                ShaderUniform::Custom {
                    name: "zenithColor",
                    ty: UniformType::Vec3,
                    default: &zenith,
                },
                ShaderUniform::Custom {
                    name: "sunDir",
                    ty: UniformType::Vec3,
                    default: &sun_dir,
                },
            ],
            back_face_culling: Some(false),
            depth_write: Some(false),
            ..Default::default()
        })?;
        mesh.set_material(Some(&material.as_material()?))?;
        mesh.set_render_order(Some(-1000.0))?;
        Ok(Self { mesh, material })
    }

    pub fn set_sun(&self, dir: Rgb) -> Result<()> {
        self.material.set_vector3("sunDir", v3(dir))
    }

    pub fn set_colors(&self, horizon: Rgb, zenith: Rgb) -> Result<()> {
        self.material.set_vector3("horizonColor", v3(horizon))?;
        self.material.set_vector3("zenithColor", v3(zenith))
    }
}
