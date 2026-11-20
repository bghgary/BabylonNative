//! Port of `clouds.ts`: one large camera-centred quad with procedural fbm
//! clouds sampled in world space, tinted by the day-night colours.

use crate::voxel_material::{Rgb, v3};
use babylon_lite::{
    Engine, MeshGeometry, Result, Runtime, Scene, SceneNode, ShaderMaterial, ShaderMaterialOptions,
    ShaderUniform, UniformType, VertexAttribute, vec3,
};

const CLOUD_Y: f64 = 80.0;
const HALF: f32 = 420.0;

const VERTEX: &str = "struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) local: vec2<f32>,
};
@vertex fn mainVertex(input: VertexInput) -> VertexOutput {
  var out: VertexOutput;
  out.position = shaderSystem.worldViewProjection * vec4<f32>(input.position, 1.0);
  out.local = input.position.xz;
  return out;
}";

const FRAGMENT: &str = "fn hash(p: vec2<f32>) -> f32 {
  var q = fract(p * vec2<f32>(123.34, 345.45));
  q = q + dot(q, q + 34.345);
  return fract(q.x * q.y);
}
fn vnoise(p: vec2<f32>) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = f * f * (3.0 - 2.0 * f);
  let a = hash(i);
  let b = hash(i + vec2<f32>(1.0, 0.0));
  let c = hash(i + vec2<f32>(0.0, 1.0));
  let d = hash(i + vec2<f32>(1.0, 1.0));
  return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}
fn fbm(p: vec2<f32>) -> f32 {
  var v = 0.0;
  var amp = 0.5;
  var q = p;
  for (var i = 0; i < 5; i = i + 1) {
    v = v + amp * vnoise(q);
    q = q * 2.02;
    amp = amp * 0.5;
  }
  return v;
}
struct VertexOutput {
  @builtin(position) position: vec4<f32>,
  @location(0) local: vec2<f32>,
};
@fragment fn mainFragment(input: VertexOutput) -> @location(0) vec4<f32> {
  // World-anchored sample: camera offset + slow wind drift.
  let wind = vec2<f32>(shaderUniforms.uTime * 1.4, shaderUniforms.uTime * 0.5);
  let sample = (input.local + shaderUniforms.uOffset.xy + wind) * 0.024;
  let n = fbm(sample);
  // Puffy coverage with a soft inner gradient for a hint of body.
  let coverage = smoothstep(0.44, 0.60, n);
  let body = smoothstep(0.46, 0.92, n);
  // Radial edge fade so the quad border is invisible.
  let r = length(input.local) / shaderUniforms.uRadius;
  let edge = 1.0 - smoothstep(0.5, 1.0, r);
  let alpha = coverage * edge;
  if (alpha < 0.01) { discard; }
  // Shade: darker undersides toward gaps, bright tops; tinted by sky colour.
  let col = mix(shaderUniforms.uTint * 0.7, shaderUniforms.uTint, body);
  return vec4<f32>(col, alpha);
}";

pub struct Clouds {
    node: SceneNode,
    material: ShaderMaterial,
}

impl Clouds {
    pub fn new(runtime: &Runtime, engine: &Engine, scene: &Scene) -> Result<Self> {
        let positions = [
            -HALF, 0.0, -HALF, HALF, 0.0, -HALF, HALF, 0.0, HALF, -HALF, 0.0, HALF,
        ];
        let normals = [0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0];
        let indices = [0, 1, 2, 0, 2, 3];
        let mesh = engine.create_mesh_from_data(
            "mc_clouds",
            &MeshGeometry {
                positions: &positions,
                normals: &normals,
                indices: &indices,
                ..Default::default()
            },
        )?;
        let node = mesh.node()?;
        node.set_position(vec3(0.0, CLOUD_Y, 0.0))?;
        let half = [f64::from(HALF)];
        let material = runtime.create_shader_material(&ShaderMaterialOptions {
            name: "mcClouds",
            vertex_source: VERTEX,
            fragment_source: FRAGMENT,
            attributes: &[VertexAttribute::Position],
            uniforms: &[
                ShaderUniform::System("worldViewProjection"),
                ShaderUniform::Custom {
                    name: "uTime",
                    ty: UniformType::F32,
                    default: &[0.0],
                },
                ShaderUniform::Custom {
                    name: "uOffset",
                    ty: UniformType::Vec3,
                    default: &[0.0, 0.0, 0.0],
                },
                ShaderUniform::Custom {
                    name: "uTint",
                    ty: UniformType::Vec3,
                    default: &[1.0, 1.0, 1.0],
                },
                ShaderUniform::Custom {
                    name: "uRadius",
                    ty: UniformType::F32,
                    default: &half,
                },
            ],
            back_face_culling: Some(false),
            depth_write: Some(false),
            need_alpha_blending: Some(true),
            ..Default::default()
        })?;
        material.set_float("uRadius", half[0])?;
        mesh.set_material(Some(&material.as_material()?))?;
        // After opaque terrain (hills occlude clouds at the horizon), before water.
        mesh.set_render_order(Some(500.0))?;
        scene.add(&node)?;
        Ok(Self { node, material })
    }

    pub fn update(&self, cam_x: f64, cam_z: f64, time: f64) -> Result<()> {
        self.node.set_position(vec3(cam_x, CLOUD_Y, cam_z))?;
        self.material.set_float("uTime", time)?;
        self.material
            .set_vector3("uOffset", vec3(cam_x, cam_z, 0.0))
    }

    pub fn set_tint(&self, color: Rgb) -> Result<()> {
        self.material.set_vector3("uTint", v3(color))
    }
}
