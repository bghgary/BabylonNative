//! Port of `highlight.ts`: a pulsing, additively blended outline around the
//! targeted block, hidden by collapsing its scale.

use babylon_lite::{
    BlendMode, BoxOptions, Engine, Mesh, Result, Runtime, SceneNode, ShaderMaterial,
    ShaderMaterialOptions, ShaderUniform, UniformType, VertexAttribute, vec3,
};

const VERTEX: &str = "struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};
@vertex fn mainVertex(input: VertexInput) -> VertexOutput {
  var out: VertexOutput;
  out.position = shaderSystem.worldViewProjection * vec4<f32>(input.position, 1.0);
  out.uv = input.uv;
  return out;
}";

const FRAGMENT: &str = "struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) uv: vec2<f32>,
};
@fragment fn mainFragment(input: VertexOutput) -> @location(0) vec4<f32> {
  // Distance to the nearest face border in UV space (0 at the edge, 0.5 at center).
  let d = min(min(input.uv.x, 1.0 - input.uv.x), min(input.uv.y, 1.0 - input.uv.y));
  // Bright crisp core line plus a wider soft halo that fades outward.
  let core = 1.0 - smoothstep(0.0, 0.022, d);
  let halo = pow(1.0 - smoothstep(0.0, 0.17, d), 1.5);
  let pulse = 0.78 + 0.22 * sin(shaderUniforms.uTime * 4.5);
  let intensity = (core + halo * 0.5) * pulse;
  if (intensity < 0.01) { discard; }
  // Cyan-white energy glow, additively blended (alpha is the additive weight).
  let glow = vec3<f32>(0.55, 0.95, 1.0);
  return vec4<f32>(glow, intensity);
}";

pub struct Highlight {
    pub mesh: Mesh,
    node: SceneNode,
    material: ShaderMaterial,
}

impl Highlight {
    pub fn new(runtime: &Runtime, engine: &Engine) -> Result<Self> {
        let mesh = engine.create_box(&BoxOptions {
            size: Some(1.01),
            ..Default::default()
        })?;
        let material = runtime.create_shader_material(&ShaderMaterialOptions {
            name: "mcHighlight",
            vertex_source: VERTEX,
            fragment_source: FRAGMENT,
            attributes: &[VertexAttribute::Position, VertexAttribute::Uv],
            uniforms: &[
                ShaderUniform::System("worldViewProjection"),
                ShaderUniform::Custom {
                    name: "uTime",
                    ty: UniformType::F32,
                    default: &[0.0],
                },
            ],
            back_face_culling: Some(false),
            depth_write: Some(false),
            need_alpha_blending: Some(true),
            blend_mode: BlendMode::Additive,
            ..Default::default()
        })?;
        mesh.set_material(Some(&material.as_material()?))?;
        mesh.set_render_order(Some(2000.0))?;
        let node = mesh.node()?;
        node.set_scaling(vec3(0.0, 0.0, 0.0))?;
        Ok(Self {
            mesh,
            node,
            material,
        })
    }

    pub fn show(&self, bx: i32, by: i32, bz: i32) -> Result<()> {
        self.node.set_scaling(vec3(1.0, 1.0, 1.0))?;
        self.node.set_position(vec3(
            f64::from(bx) + 0.5,
            f64::from(by) + 0.5,
            f64::from(bz) + 0.5,
        ))
    }

    pub fn hide(&self) -> Result<()> {
        self.node.set_scaling(vec3(0.0, 0.0, 0.0))
    }

    pub fn set_time(&self, t: f64) -> Result<()> {
        self.material.set_float("uTime", t)
    }
}
