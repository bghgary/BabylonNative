//! Contract checks that need no window or GPU.

use babylon_lite::{Runtime, Status, vec3};

#[test]
fn node_transforms_round_trip() {
    let runtime = Runtime::new().unwrap();
    let node = runtime.create_transform_node("pivot").unwrap();
    assert_eq!(node.name().unwrap(), "pivot");
    node.set_position(vec3(1.0, 2.0, 3.0)).unwrap();
    let position = node.position().unwrap();
    assert_eq!((position.x, position.y, position.z), (1.0, 2.0, 3.0));
    let (world, _) = node.world_matrix().unwrap();
    assert_eq!(&world.values[12..15], &[1.0, 2.0, 3.0]);
}

#[test]
fn disposed_handles_report_disposed() {
    let runtime = Runtime::new().unwrap();
    let node = runtime.create_transform_node("").unwrap();
    node.dispose().unwrap();
    assert_eq!(node.position().unwrap_err().status, Status::Disposed);
    node.dispose().unwrap();
}

#[test]
fn handles_keep_the_runtime_alive() {
    let node = Runtime::new()
        .unwrap()
        .create_transform_node("orphan")
        .unwrap();
    node.set_scaling(vec3(2.0, 2.0, 2.0)).unwrap();
    assert_eq!(node.scaling().unwrap().x, 2.0);
    assert_eq!(node.runtime().raw(), node.raw()._runtime);
}

#[test]
fn family_conversions_check_the_concrete_kind() {
    let runtime = Runtime::new().unwrap();
    let camera = runtime
        .create_free_camera(vec3(0.0, 0.0, -5.0), vec3(0.0, 0.0, 0.0))
        .unwrap();
    let generic = camera.as_camera().unwrap();
    assert_eq!(generic.as_free().unwrap(), camera);
    assert_eq!(
        generic.as_arc_rotate().unwrap_err().status,
        Status::InvalidHandle
    );
}

#[test]
fn arc_rotate_properties_use_source_defaults() {
    let runtime = Runtime::new().unwrap();
    let camera = runtime
        .create_arc_rotate_camera(0.0, 1.0, 5.0, vec3(0.0, 0.0, 0.0))
        .unwrap();
    let mut properties = camera.properties().unwrap();
    assert_eq!(
        (properties.fov, properties.nearPlane, properties.farPlane),
        (0.8, 0.1, 1000.0)
    );
    properties.radius = 9.0;
    camera.set_properties(&properties).unwrap();
    assert_eq!(camera.properties().unwrap().radius, 9.0);
}

#[test]
fn arc_rotate_control_forwards_pointer_and_wheel_input() {
    let runtime = Runtime::new().unwrap();
    let camera = runtime
        .create_arc_rotate_camera(0.0, 1.0, 5.0, vec3(0.0, 0.0, 0.0))
        .unwrap();
    let control = camera.attach_control(None).unwrap();
    assert!(
        control
            .pointer_down(7, 0, 10.0, 20.0)
            .unwrap()
            .capture_pointer
    );
    control.pointer_move(7, 110.0, 70.0).unwrap();
    control.wheel(120.0).unwrap();
    let properties = camera.properties().unwrap();
    let close = |actual: f64, expected: f64| (actual - expected).abs() < 1e-12;
    assert!(
        close(properties.inertialAlphaOffset, -0.1),
        "{}",
        properties.inertialAlphaOffset
    );
    assert!(
        close(properties.inertialBetaOffset, -0.05),
        "{}",
        properties.inertialBetaOffset
    );
    assert!(
        close(properties.inertialRadiusOffset, -0.2),
        "{}",
        properties.inertialRadiusOffset
    );
    control.detach().unwrap();
}

#[test]
fn shader_material_declares_uniforms_and_rejects_unknown_ones() {
    use babylon_lite::{ShaderMaterialOptions, ShaderUniform, UniformType, VertexAttribute};
    let runtime = Runtime::new().unwrap();
    let options = ShaderMaterialOptions {
        name: "tint",
        vertex_source: "@vertex fn mainVertex(input: VertexInput) -> @builtin(position) vec4<f32> { return shaderSystem.worldViewProjection * vec4<f32>(input.position, 1.0); }",
        fragment_source: "@fragment fn mainFragment() -> @location(0) vec4<f32> { return vec4<f32>(shaderUniforms.tint, 1.0); }",
        attributes: &[VertexAttribute::Position],
        uniforms: &[
            ShaderUniform::System("worldViewProjection"),
            ShaderUniform::Custom {
                name: "tint",
                ty: UniformType::Vec3,
                default: &[1.0, 0.0, 0.0],
            },
        ],
        ..Default::default()
    };
    let material = runtime.create_shader_material(&options).unwrap();
    material.set_vector3("tint", vec3(0.0, 1.0, 0.0)).unwrap();
    let unknown = material.set_float("missing", 1.0).unwrap_err();
    assert_eq!(
        (unknown.status, unknown.source_error_code),
        (Status::InvalidArgument, 400)
    );
    assert_eq!(
        material.as_material().unwrap().as_shader().unwrap(),
        material
    );
}

#[test]
fn error_detail_comes_only_from_the_failing_call() {
    let runtime = Runtime::new().unwrap();
    let light = runtime
        .create_hemispheric_light(None, None)
        .unwrap()
        .as_light()
        .unwrap();
    let detailed = light.set_intensity(f64::NAN).unwrap_err();
    assert_eq!(detailed.status, Status::InvalidArgument);
    assert_eq!(detailed.source_error_code, 134);
    assert!(!detailed.message.is_empty());

    // Same status, but this check reports no detail; the runtime's last error
    // still holds the intensity failure and must not be attached here.
    let node = runtime.create_transform_node("").unwrap();
    let bare = node.set_position(vec3(f64::NAN, 0.0, 0.0)).unwrap_err();
    assert_eq!(bare.status, Status::InvalidArgument);
    assert_eq!(
        (
            bare.operation.as_str(),
            bare.message.as_str(),
            bare.source_error_code
        ),
        ("", "", 0)
    );
}
