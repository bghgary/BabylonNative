// Generates oracle_data.rs (the expectations in ../oracle_tests.rs) by running the
// original TypeScript demo logic. From this directory:
//   npx esbuild oracle.ts --bundle --format=esm --platform=node --outfile=oracle.bundle.mjs
//   node oracle.bundle.mjs
// The bundle writes oracle_data.rs beside itself; delete the bundle afterwards.

import { writeFileSync } from "node:fs";
import { Block, allReferencedTiles, blockColor, blockDef, HOTBAR } from "../../../../../../Apps/LiteTests/LitePlayground/JavaScript/Demos/minecraft/blocks.js";
import { CHUNK_SX, CHUNK_SZ, WORLD_H, blockIndex } from "../../../../../../Apps/LiteTests/LitePlayground/JavaScript/Demos/minecraft/constants.js";
import { fbm2, rand3, valueNoise2, valueNoise3 } from "../../../../../../Apps/LiteTests/LitePlayground/JavaScript/Demos/minecraft/noise.js";
import { generateChunk } from "../../../../../../Apps/LiteTests/LitePlayground/JavaScript/Demos/minecraft/worldgen.js";
import { World } from "../../../../../../Apps/LiteTests/LitePlayground/JavaScript/Demos/minecraft/world.js";
import { meshChunk, type MeshGeometry } from "../../../../../../Apps/LiteTests/LitePlayground/JavaScript/Demos/minecraft/mesher.js";
import { raycastVoxel } from "../../../../../../Apps/LiteTests/LitePlayground/JavaScript/Demos/minecraft/raycast.js";

const SEED = 1337;
const CELLS = CHUNK_SX * CHUNK_SZ * WORLD_H;
const OFFSET = 2166136261;
const FIELDS = ["positions", "normals", "uvs", "colors", "indices"] as const;
function hashBytes(bytes: Uint8Array, h = OFFSET): number {
    for (const b of bytes) h = Math.imul(h ^ b, 16777619) >>> 0;
    return h;
}
function arrayBytes(array: Float32Array | Uint32Array): Uint8Array {
    const bytes = new Uint8Array(array.length * 4);
    const view = new DataView(bytes.buffer);
    for (let i = 0; i < array.length; i++) {
        if (array instanceof Float32Array) view.setFloat32(i * 4, array[i], true);
        else view.setUint32(i * 4, array[i], true);
    }
    return bytes;
}
function counts(blocks: Uint8Array): number[] {
    const out = Array(24).fill(0);
    for (const id of blocks) if (id < 24) out[id]++;
    return out;
}
function floatBits(n: number): bigint {
    const view = new DataView(new ArrayBuffer(8));
    view.setFloat64(0, n, true);
    return view.getBigUint64(0, true);
}
function rustFloat(n: number): string {
    const s = String(n);
    return /[.eE]/.test(s) ? s : s + ".0";
}
function arrayRust(values: (number | bigint | string)[]): string {
    return "[" + values.map(String).join(", ") + "]";
}

const totalCounts = Array(24).fill(0);
const terrain = [];
for (let cz = -6; cz <= 6; cz++) {
    for (let cx = -6; cx <= 6; cx++) {
        const blocks = new Uint8Array(CELLS);
        generateChunk(SEED, cx, cz, blocks);
        const c = counts(blocks);
        for (let id = 0; id < c.length; id++) totalCounts[id] += c[id];
        terrain.push({ cx, cz, digest: hashBytes(blocks), counts: c });
    }
}

// Same index-based cells and half-texel bounds as atlas.ts, without any textures.
const tiles = allReferencedTiles();
const cols = Math.ceil(Math.sqrt(tiles.length));
const rows = Math.ceil(tiles.length / cols);
const rects = new Map(tiles.map((tile, i) => {
    const col = i % cols;
    const row = Math.floor(i / cols);
    return [tile, {
        u0: col / cols + 0.5 / (cols * 64),
        v0: row / rows + 0.5 / (rows * 64),
        u1: (col + 1) / cols - 0.5 / (cols * 64),
        v1: (row + 1) / rows - 0.5 / (rows * 64),
    }];
}));
const atlas = { rects, fallback: rects.get(tiles[0])!, texture: null! };

const nearby = [...terrain].sort((a, b) =>
    a.cx * a.cx + a.cz * a.cz - b.cx * b.cx - b.cz * b.cz || a.cz - b.cz || a.cx - b.cx);
const selected = [terrain.find(t => t.cx === 0 && t.cz === 0)!];
for (const id of [Block.WATER, Block.LEAVES]) {
    const t = nearby.find(t => t.counts[id] > 0 && !selected.includes(t));
    if (!t) throw new Error("No distinct nearby chunk with required block " + id);
    selected.push(t);
}

function packedChunk(world: World, cx: number, cz: number): Uint8Array {
    const out = new Uint8Array(CELLS);
    for (let y = 0; y < WORLD_H; y++) {
        for (let z = 0; z < CHUNK_SZ; z++) {
            for (let x = 0; x < CHUNK_SX; x++) {
                out[blockIndex(x, y, z)] = world.getLightPacked(cx * CHUNK_SX + x, y, cz * CHUNK_SZ + z);
            }
        }
    }
    return out;
}
function geometryFixture(geometry: MeshGeometry | null) {
    if (!geometry) return null;
    const bytes = FIELDS.map(field => arrayBytes(geometry[field]));
    return {
        lengths: FIELDS.map(field => geometry[field].length),
        digests: bytes.map(b => hashBytes(b)),
        combined: bytes.reduce((h, b) => hashBytes(b, h), OFFSET),
    };
}
function meshFixture(world: World, cx: number, cz: number, layout = atlas) {
    world.light.beginFrame();
    if (!world.light.warmFor(cx, cz, Infinity)) throw new Error("Unlimited warming failed");
    const data = meshChunk(world, cx, cz, layout);
    let digest = OFFSET;
    for (const mode of ["opaque", "cutout", "blend"] as const) {
        const geo = data[mode];
        if (geo) for (const field of FIELDS) digest = hashBytes(arrayBytes(geo[field]), digest);
    }
    return {
        cx, cz, counts: counts(world.getChunk(cx, cz).blocks),
        lightDigest: hashBytes(packedChunk(world, cx, cz)),
        opaque: geometryFixture(data.opaque),
        cutout: geometryFixture(data.cutout),
        blend: geometryFixture(data.blend),
        digest,
    };
}
const world = new World(SEED);
const meshes = selected.map(t => meshFixture(world, t.cx, t.cz));

function syntheticWorld(): World {
    const w = new World(SEED);
    for (let cz = -2; cz <= 2; cz++) {
        for (let cx = -2; cx <= 2; cx++) w.getChunk(cx, cz).blocks.fill(Block.AIR);
    }
    for (let id = 1; id <= 23; id++) {
        const i = id - 1;
        w.setBlock((i % 5) * 3, 60, Math.floor(i / 5) * 3, id, true);
    }
    // Mixed transparent interfaces, both AO diagonal choices, and a seam emitter.
    for (const [x, y, z, id] of [
        [1, 60, 3, Block.WATER], [1, 60, 0, Block.GLASS],
        [7, 66, 7, Block.STONE], [6, 67, 7, Block.STONE],
        [7, 67, 6, Block.STONE], [8, 67, 8, Block.STONE],
        [15, 70, 15, Block.LEAVES], [16, 70, 15, Block.GLOWSTONE],
        [9, 74, 9, 255],
    ]) w.setBlock(x, y, z, id, true);
    return w;
}
const synthetic = syntheticWorld();
const syntheticMesh = meshFixture(synthetic, 0, 0);
const fallbackAtlas = { ...atlas, rects: new Map(rects) };
fallbackAtlas.rects.delete("dirt");
fallbackAtlas.rects.delete("leaves_transparent");
const fallbackMesh = meshFixture(synthetic, 0, 0, fallbackAtlas);
const empty = new World(SEED);
for (let cz = -2; cz <= 2; cz++) {
    for (let cx = -2; cx <= 2; cx++) empty.getChunk(cx, cz).blocks.fill(Block.AIR);
}
const emptyMesh = meshFixture(empty, 0, 0);

const noiseInputs = [
    [1337, -0.25, 0.5, -17.75],
    [-1, -16.125, -33.75, 14.1],
    [4294968633.75, 4294967295.75, -4294967296.25, 0.25],
    [-1099511627776.5, 1000000000000.125, -1000000000000.75, -3.5],
    [0, 0, 0, 0],
];
const noise = noiseInputs.map(([seed, x, y, z]) => ({
    input: [seed, x, y, z],
    bits: [
        valueNoise2(seed, x, y), valueNoise3(seed, x, y, z),
        fbm2(seed, x, y, 4, 2, 0.5), rand3(seed, x, y, z),
    ].map(v => floatBits(v).toString()),
}));

const editWorld = new World(SEED);
for (const [x, y, z, id, record] of [
    [1, 80, 1, Block.GRASS, true], [-1, 80, -1, Block.GLASS, true],
    [2, 81, 2, Block.DIRT, false], [3, 82, 3, Block.STONE, true],
    [1, 80, 1, Block.SAND, true], [15, 75, 15, Block.GLOWSTONE, true],
    [15, 75, 15, Block.AIR, true], [-16, 79, 1, 511, true],
    [-17, 79, 0, -1, true],
] as const) editWorld.setBlock(x, y, z, id, record);
const exported = editWorld.exportEdits();
editWorld.dropChunk(0, 0);
const restored = [editWorld.getBlock(1, 80, 1), editWorld.getBlock(2, 81, 2), editWorld.getBlock(3, 82, 3)];
editWorld.reset(SEED + 1, [...exported, 0, -1, 0, 23, 0, WORLD_H, 0, 23, 99]);
const resetExported = editWorld.exportEdits();
const wrapped = [editWorld.getBlock(-16, 79, 1), editWorld.getBlock(-17, 79, 0)];
const spawnWorld = new World(SEED);
const spawns = [[0, 0], [-1, -1], [17, -20]].map(([x, z]) => spawnWorld.findSpawn(x, z));
const stickyWorld = new World(SEED);
stickyWorld.setBlock(8, 80, 8, Block.GLOWSTONE, true);
const stickyBefore = stickyWorld.getLightPacked(8, 80, 8);
stickyWorld.reset(SEED, []);
const stickyAfter = stickyWorld.getLightPacked(8, 80, 8);
const edits = { exported, restored, resetExported, wrapped, spawns, stickyBefore, stickyAfter };

const lightWorld = syntheticWorld();
const lightSnapshots: number[][] = [];
function snapshot() {
    lightWorld.light.beginFrame();
    lightWorld.light.warmFor(0, 0, Infinity);
    const hashes = [];
    for (let cz = -1; cz <= 1; cz++) {
        for (let cx = -1; cx <= 1; cx++) hashes.push(hashBytes(packedChunk(lightWorld, cx, cz)));
    }
    lightSnapshots.push(hashes);
}
snapshot();
lightWorld.setBlock(12, 60, 12, Block.GLOWSTONE, true); // interior: only self invalidated
snapshot();
lightWorld.setBlock(15, 70, 15, Block.AIR, true); // corner: four caches invalidated
snapshot();
lightWorld.dropChunk(1, 1); // deterministic terrain replay, only own cache invalidated
snapshot();

const rayWorld = new World(SEED);
for (const [x, y, z, id] of [
    [1, 80, 1, Block.GLASS], [1, 80, 0, Block.WATER],
    [-2, 80, -1, Block.LEAVES], [0, 80, -1, Block.STONE],
    [0, 81, 1, Block.CACTUS],
]) rayWorld.setBlock(x, y, z, id, true);
const rays = [
    [0.5, 80.5, 0.5, 1, 0, 1, 8],
    [1.5, 80.5, 1.5, 0, 0, 0, 8],
    [-0.5, 80.5, -0.5, -1, 0, 0, 8],
    [0, 80.5, -0.5, -1, 0, 0, 0],
    [0.5, 80.5, 0.5, 0, 0, 0, 8],
    [0.5, 80.5, 0.5, 1, 0, 1, 0.4],
    [0.5, 80.5, 0.5, 0, 1, 1, 8],
    [0.5, 80.5, 0.5, 1, 0, 0, 0.5],
].map(input => ({
    input,
    hit: raycastVoxel(rayWorld, input[0], input[1], input[2], input[3], input[4], input[5], input[6]),
}));

function geoRust(geo: ReturnType<typeof geometryFixture>): string {
    return geo ? `Some(GeometryFixture { lengths: ${arrayRust(geo.lengths)}, digests: ${arrayRust(geo.digests)}, combined: ${geo.combined} })` : "None";
}
function meshRust(m: ReturnType<typeof meshFixture>): string {
    return `MeshFixture { cx: ${m.cx}, cz: ${m.cz}, counts: ${arrayRust(m.counts)}, light_digest: ${m.lightDigest}, opaque: ${geoRust(m.opaque)}, cutout: ${geoRust(m.cutout)}, blend: ${geoRust(m.blend)}, digest: ${m.digest} }`;
}
const registry = Array.from({ length: 23 }, (_, i) => {
    const d = blockDef(i + 1)!;
    return { ...d, color: blockColor(i + 1) };
});
let rust = "// Generated by tools/oracle.ts from the original TypeScript; do not hand-edit.\n";
rust += `pub const TERRAIN: &[(i32, i32, u32)] = &[\n${terrain.map(t => `    (${t.cx}, ${t.cz}, ${t.digest}),`).join("\n")}\n];\n`;
rust += `pub const TOTAL_COUNTS: [u64; 24] = ${arrayRust(totalCounts)};\n`;
rust += `pub const TILES: &[&str] = &${JSON.stringify(tiles)};\n`;
rust += `pub const HOTBAR_IDS: [u8; 10] = ${arrayRust(HOTBAR)};\n`;
rust += `pub const MESHES: &[MeshFixture] = &[\n${meshes.map(m => meshRust(m) + ",").join("\n")}\n];\n`;
rust += `pub const SYNTHETIC: MeshFixture = ${meshRust(syntheticMesh)};\n`;
rust += `pub const FALLBACK: MeshFixture = ${meshRust(fallbackMesh)};\n`;
rust += `pub const EMPTY: MeshFixture = ${meshRust(emptyMesh)};\n`;
rust += `pub const NOISE: &[([f64; 4], [u64; 4])] = &[\n${noise.map(n => `(${arrayRust(n.input.map(rustFloat))}, ${arrayRust(n.bits)}),`).join("\n")}\n];\n`;
rust += `pub const EXPORTED: &[i32] = &${arrayRust(exported)};\n`;
rust += `pub const RESTORED: [u8; 3] = ${arrayRust(restored)};\n`;
rust += `pub const RESET_EXPORTED: &[i32] = &${arrayRust(resetExported)};\n`;
rust += `pub const WRAPPED: [u8; 2] = ${arrayRust(wrapped)};\n`;
rust += `pub const SPAWNS: [[f64; 3]; 3] = [${spawns.map(s => arrayRust([s.x, s.y, s.z].map(rustFloat))).join(", ")}];\n`;
rust += `pub const STICKY_LIGHT: [u8; 2] = ${arrayRust([stickyBefore, stickyAfter])};\n`;
rust += `pub const LIGHT_SNAPSHOTS: [[u32; 9]; 4] = [${lightSnapshots.map(arrayRust).join(", ")}];\n`;
rust += `pub const RAYS: &[([f64; 7], Option<RayHit>)] = &[\n${rays.map(r =>
    `(${arrayRust(r.input.map(rustFloat))}, ${r.hit ? `Some(RayHit { bx: ${r.hit.bx}, by: ${r.hit.by}, bz: ${r.hit.bz}, px: ${r.hit.px}, py: ${r.hit.py}, pz: ${r.hit.pz} })` : "None"}),`).join("\n")}\n];\n`;
rust += `pub const REGISTRY: &[RegistryFixture] = &[\n${registry.map(d =>
    `RegistryFixture { name: ${JSON.stringify(d.name)}, faces: ${JSON.stringify([d.faces.top, d.faces.side, d.faces.bottom])}, mode: RenderMode::${d.renderMode[0].toUpperCase() + d.renderMode.slice(1)}, flags: ${arrayRust([d.hidesNeighborFaces, d.castsAO, d.collidable, d.fluid, d.sway, d.indestructible].map(String))}, light: ${d.light}, color: ${arrayRust(d.color.map(rustFloat))} },`).join("\n")}\n];\n`;
writeFileSync(new URL("./oracle_data.rs", import.meta.url), rust);
const result = { seed: SEED, terrain, totalCounts, tiles, meshes, syntheticMesh, fallbackMesh, emptyMesh, noise, edits, lightSnapshots, rays, registry };

console.log(JSON.stringify(result, null, 2));
