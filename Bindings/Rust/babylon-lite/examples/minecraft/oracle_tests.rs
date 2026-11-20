//! Oracle checks against the original TypeScript demo: digests were produced by
//! running the TypeScript worldgen, world light and mesher under Node.

use crate::blocks::{Block, HOTBAR, RenderMode, all_referenced_tiles, block_color, block_def};
use crate::constants::{CHUNK_CELLS, CHUNK_SX, CHUNK_SZ, WORLD_H, block_index};
use crate::mesher::{AtlasLayout, MeshGeometry, TileRect, mesh_chunk};
use crate::noise::{fbm2, rand3, value_noise2, value_noise3};
use crate::raycast::{RayHit, raycast_voxel};
use crate::world::World;
use crate::worldgen::generate_chunk;

const SEED: f64 = 1337.0;
const OFFSET: u32 = 2_166_136_261;

#[derive(Debug, PartialEq)]
struct GeometryFixture {
    lengths: [usize; 5],
    digests: [u32; 5],
    combined: u32,
}

struct MeshFixture {
    cx: i32,
    cz: i32,
    counts: [usize; 24],
    light_digest: u32,
    opaque: Option<GeometryFixture>,
    cutout: Option<GeometryFixture>,
    blend: Option<GeometryFixture>,
    digest: u32,
}

struct RegistryFixture {
    name: &'static str,
    faces: [&'static str; 3],
    mode: RenderMode,
    flags: [bool; 6],
    light: u8,
    color: [f64; 3],
}

mod expected {
    use super::{GeometryFixture, MeshFixture, RayHit, RegistryFixture, RenderMode};
    include!("support/oracle_data.rs");
}

fn append_hash(h: u32, bytes: impl IntoIterator<Item = u8>) -> u32 {
    bytes
        .into_iter()
        .fold(h, |h, b| (h ^ u32::from(b)).wrapping_mul(16_777_619))
}

fn hash(bytes: impl IntoIterator<Item = u8>) -> u32 {
    append_hash(OFFSET, bytes)
}

fn atlas() -> AtlasLayout {
    let tiles = expected::TILES;
    let cols = (tiles.len() as f64).sqrt().ceil() as usize;
    let rows = tiles.len().div_ceil(cols);
    let rects: std::collections::HashMap<_, _> = tiles
        .iter()
        .enumerate()
        .map(|(i, &tile)| {
            let col = i % cols;
            let row = i / cols;
            (
                tile.to_owned(),
                TileRect {
                    u0: col as f64 / cols as f64 + 0.5 / (cols * 64) as f64,
                    v0: row as f64 / rows as f64 + 0.5 / (rows * 64) as f64,
                    u1: (col + 1) as f64 / cols as f64 - 0.5 / (cols * 64) as f64,
                    v1: (row + 1) as f64 / rows as f64 - 0.5 / (rows * 64) as f64,
                },
            )
        })
        .collect();
    let fallback = rects[tiles[0]];
    AtlasLayout { rects, fallback }
}

fn floats(geo: &MeshGeometry) -> [&[f32]; 4] {
    [&geo.positions, &geo.normals, &geo.uvs, &geo.colors]
}

fn append_geometry(mut h: u32, geo: &MeshGeometry) -> u32 {
    for values in floats(geo) {
        h = append_hash(h, values.iter().flat_map(|v| v.to_bits().to_le_bytes()));
    }
    append_hash(h, geo.indices.iter().flat_map(|v| v.to_le_bytes()))
}

fn geometry_fixture(geo: &MeshGeometry) -> GeometryFixture {
    let [positions, normals, uvs, colors] = floats(geo);
    let mut digests = [0; 5];
    for (i, values) in floats(geo).into_iter().enumerate() {
        digests[i] = hash(values.iter().flat_map(|v| v.to_bits().to_le_bytes()));
    }
    digests[4] = hash(geo.indices.iter().flat_map(|v| v.to_le_bytes()));
    GeometryFixture {
        lengths: [
            positions.len(),
            normals.len(),
            uvs.len(),
            colors.len(),
            geo.indices.len(),
        ],
        digests,
        combined: append_geometry(OFFSET, geo),
    }
}

fn block_counts(blocks: &[u8]) -> [usize; 24] {
    let mut out = [0; 24];
    for &id in blocks {
        if let Some(count) = out.get_mut(usize::from(id)) {
            *count += 1;
        }
    }
    out
}

fn packed_digest(world: &mut World, cx: i32, cz: i32) -> u32 {
    let mut out = vec![0; CHUNK_CELLS];
    for y in 0..WORLD_H {
        for z in 0..CHUNK_SZ {
            for x in 0..CHUNK_SX {
                out[block_index(x, y, z)] =
                    world.get_light_packed(cx * CHUNK_SX + x, y, cz * CHUNK_SZ + z);
            }
        }
    }
    hash(out)
}

fn assert_mesh(world: &mut World, fixture: &MeshFixture, atlas: &AtlasLayout) {
    let (cx, cz) = (fixture.cx, fixture.cz);
    world.light.begin_frame();
    assert!(world.warm_light_for(cx, cz, f64::INFINITY));
    let data = mesh_chunk(world, cx, cz, atlas);
    assert_eq!(
        block_counts(&world.get_chunk(cx, cz).blocks),
        fixture.counts
    );
    assert_eq!(
        packed_digest(world, cx, cz),
        fixture.light_digest,
        "light ({cx},{cz})"
    );
    let mut digest = OFFSET;
    for (mode, geo, expected) in [
        ("opaque", &data.opaque, &fixture.opaque),
        ("cutout", &data.cutout, &fixture.cutout),
        ("blend", &data.blend, &fixture.blend),
    ] {
        let actual = geo.as_ref().map(geometry_fixture);
        assert_eq!(&actual, expected, "{mode} ({cx},{cz})");
        if let Some(geo) = geo {
            digest = append_geometry(digest, geo);
        }
    }
    assert_eq!(digest, fixture.digest, "mesh ({cx},{cz})");
}

fn empty_world() -> World {
    let mut world = World::new(SEED);
    for cz in -2..=2 {
        for cx in -2..=2 {
            world.get_chunk(cx, cz).blocks.fill(Block::AIR);
        }
    }
    world
}

fn synthetic_world() -> World {
    let mut world = empty_world();
    for id in 1u8..=23 {
        let i = i32::from(id) - 1;
        world.set_block((i % 5) * 3, 60, (i / 5) * 3, id, true);
    }
    for (x, y, z, id) in [
        (1, 60, 3, Block::WATER),
        (1, 60, 0, Block::GLASS),
        (7, 66, 7, Block::STONE),
        (6, 67, 7, Block::STONE),
        (7, 67, 6, Block::STONE),
        (8, 67, 8, Block::STONE),
        (15, 70, 15, Block::LEAVES),
        (16, 70, 15, Block::GLOWSTONE),
        (9, 74, 9, 255),
    ] {
        world.set_block(x, y, z, id, true);
    }
    world
}

#[test]
fn terrain_matches_all_169_typescript_chunks_and_block_totals() {
    let mut total_counts = [0u64; 24];
    let mut blocks = vec![0; CHUNK_CELLS];
    for &(cx, cz, digest) in expected::TERRAIN {
        generate_chunk(SEED, cx, cz, &mut blocks);
        assert_eq!(hash(blocks.iter().copied()), digest, "chunk ({cx},{cz})");
        for &id in &blocks {
            total_counts[usize::from(id)] += 1;
        }
    }
    assert_eq!(total_counts, expected::TOTAL_COUNTS);
}

#[test]
fn noise_matches_f64_bits_including_negative_and_wrapping_inputs() {
    for &([seed, x, y, z], bits) in expected::NOISE {
        let actual = [
            value_noise2(seed, x, y),
            value_noise3(seed, x, y, z),
            fbm2(seed, x, y, 4, 2.0, 0.5),
            rand3(seed, x, y, z),
        ]
        .map(f64::to_bits);
        assert_eq!(actual, bits, "seed {seed}, ({x},{y},{z})");
    }
    assert!(fbm2(SEED, 1.0, 2.0, 0, 2.0, 0.5).is_nan());
}

#[test]
fn registry_properties_and_tile_insertion_order_match_typescript() {
    assert_eq!(all_referenced_tiles(), expected::TILES);
    assert_eq!(HOTBAR, expected::HOTBAR_IDS);
    for (i, expected) in expected::REGISTRY.iter().enumerate() {
        let id = (i + 1) as u8;
        let d = block_def(id).unwrap();
        assert_eq!(d.id, id);
        assert_eq!(d.name, expected.name);
        assert_eq!([d.faces.top, d.faces.side, d.faces.bottom], expected.faces);
        assert_eq!(d.render_mode, expected.mode);
        assert_eq!(
            [
                d.hides_neighbor_faces,
                d.casts_ao,
                d.collidable,
                d.fluid,
                d.sway,
                d.indestructible,
            ],
            expected.flags
        );
        assert_eq!(d.light, expected.light);
        assert_eq!(block_color(id), expected.color);
    }
}

#[test]
fn terrain_mesh_arrays_and_light_match_three_typescript_chunks() {
    let mut world = World::new(SEED);
    let atlas = atlas();
    for expected in expected::MESHES {
        assert_mesh(&mut world, expected, &atlas);
    }
}

#[test]
fn mixed_blocks_emitters_fallback_and_empty_mesh_match_typescript() {
    let mut world = synthetic_world();
    let mut atlas = atlas();
    assert_mesh(&mut world, &expected::SYNTHETIC, &atlas);
    atlas.rects.remove("dirt");
    atlas.rects.remove("leaves_transparent");
    assert_mesh(&mut world, &expected::FALLBACK, &atlas);
    let mut empty = empty_world();
    assert_mesh(&mut empty, &expected::EMPTY, &atlas);
}

#[test]
fn world_edit_order_streaming_reset_and_wrapping_match_typescript() {
    let mut world = World::new(SEED);
    for (x, y, z, id, record) in [
        (1, 80, 1, i32::from(Block::GRASS), true),
        (-1, 80, -1, i32::from(Block::GLASS), true),
        (2, 81, 2, i32::from(Block::DIRT), false),
        (3, 82, 3, i32::from(Block::STONE), true),
        (1, 80, 1, i32::from(Block::SAND), true),
        (15, 75, 15, i32::from(Block::GLOWSTONE), true),
        (15, 75, 15, i32::from(Block::AIR), true),
        (-16, 79, 1, 511, true),
        (-17, 79, 0, -1, true),
    ] {
        world.set_block(x, y, z, id, record);
    }
    assert!(world.any_emitters());
    assert_eq!(world.export_edits(), expected::EXPORTED);
    world.drop_chunk(0, 0);
    assert!(!world.has_chunk(0, 0));
    assert_eq!(
        [
            world.get_block(1, 80, 1),
            world.get_block(2, 81, 2),
            world.get_block(3, 82, 3),
        ],
        expected::RESTORED
    );
    let mut edits = world.export_edits();
    edits.extend([0, -1, 0, 23, 0, WORLD_H, 0, 23, 99]);
    world.reset(SEED + 1.0, &edits);
    assert!(!world.any_emitters());
    assert_eq!(world.export_edits(), expected::RESET_EXPORTED);
    assert_eq!(
        [world.get_block(-16, 79, 1), world.get_block(-17, 79, 0),],
        expected::WRAPPED
    );
    assert_eq!(world.set_block(1000, -1, 1000, Block::STONE, true), None);
    assert_eq!(
        world.set_block(1000, WORLD_H, 1000, Block::STONE, true),
        None
    );
    assert!(!world.has_chunk(62, 62));
}

#[test]
fn spawn_search_matches_typescript() {
    let mut world = World::new(SEED);
    for ((x, z), expected) in [(0, 0), (-1, -1), (17, -20)]
        .into_iter()
        .zip(expected::SPAWNS)
    {
        let s = world.find_spawn(x, z);
        assert_eq!([s.x, s.y, s.z], expected);
    }
}

#[test]
fn light_flood_and_interior_seam_unload_cache_rules_match_typescript() {
    let mut world = synthetic_world();
    for (phase, expected) in expected::LIGHT_SNAPSHOTS.iter().enumerate() {
        match phase {
            1 => {
                world.set_block(12, 60, 12, Block::GLOWSTONE, true);
            }
            2 => {
                world.set_block(15, 70, 15, Block::AIR, true);
            }
            3 => world.drop_chunk(1, 1),
            _ => {}
        }
        world.light.begin_frame();
        assert!(world.warm_light_for(0, 0, f64::INFINITY));
        let mut actual = [0; 9];
        for cz in -1..=1 {
            for cx in -1..=1 {
                actual[((cz + 1) * 3 + cx + 1) as usize] = packed_digest(&mut world, cx, cz);
            }
        }
        assert_eq!(&actual, expected, "phase {phase}");
    }
}

#[test]
fn light_warming_budget_is_shared_across_calls_and_reset_per_frame() {
    let mut world = World::new(SEED);
    world.light.begin_frame();
    assert!(!world.warm_light_for(0, 0, 4.0));
    assert!(!world.warm_light_for(0, 0, 4.0));
    world.light.begin_frame();
    assert!(!world.warm_light_for(0, 0, 4.0));
    world.light.begin_frame();
    assert!(world.warm_light_for(0, 0, 1.0));
    assert!(world.warm_light_for(0, 0, 0.0));
    world.set_block(8, 80, 8, Block::DIRT, true);
    world.light.begin_frame();
    assert!(world.warm_light_for(0, 0, 1.0));
    world.set_block(15, 80, 15, Block::DIRT, true);
    world.light.begin_frame();
    assert!(!world.warm_light_for(0, 0, 3.0));
    world.light.begin_frame();
    assert!(world.warm_light_for(0, 0, 1.0));
    world.light.invalidate_around(0, 0);
    world.light.begin_frame();
    assert!(!world.warm_light_for(0, 0, 8.0));
    world.light.begin_frame();
    assert!(world.warm_light_for(0, 0, 1.0));
}

#[test]
fn source_light_scratch_state_after_reset_is_preserved() {
    let mut world = World::new(SEED);
    world.set_block(8, 80, 8, Block::GLOWSTONE, true);
    let before = world.get_light_packed(8, 80, 8);
    world.reset(SEED, &[]);
    let after = world.get_light_packed(8, 80, 8);
    assert_eq!([before, after], expected::STICKY_LIGHT);
}

#[test]
fn ray_selection_fluid_skipping_boundaries_ties_and_limits_match_typescript() {
    let mut world = World::new(SEED);
    for (x, y, z, id) in [
        (1, 80, 1, Block::GLASS),
        (1, 80, 0, Block::WATER),
        (-2, 80, -1, Block::LEAVES),
        (0, 80, -1, Block::STONE),
        (0, 81, 1, Block::CACTUS),
    ] {
        world.set_block(x, y, z, id, true);
    }
    for &([ox, oy, oz, dx, dy, dz, max_dist], expected) in expected::RAYS {
        assert_eq!(
            raycast_voxel(&mut world, [ox, oy, oz], [dx, dy, dz], max_dist),
            expected
        );
    }
}
