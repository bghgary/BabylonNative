use crate::blocks::Block;
use crate::constants::{CHUNK_CELLS, CHUNK_SX, CHUNK_SZ, SEA_LEVEL, WORLD_H, block_index};
use crate::noise::{fbm2, rand3, value_noise3};

const TREE_MARGIN: i32 = 2;

fn height_at(seed: f64, wx: f64, wz: f64) -> i32 {
    let continent = fbm2(seed, wx / 220.0, wz / 220.0, 4, 2.0, 0.5);
    let hills = fbm2(seed + 7.0, wx / 70.0, wz / 70.0, 4, 2.0, 0.5);
    let detail = fbm2(seed + 19.0, wx / 24.0, wz / 24.0, 3, 2.0, 0.5);
    let mut h = f64::from(SEA_LEVEL)
        + (continent - 0.46) * 70.0
        + (hills - 0.5) * 18.0
        + (detail - 0.5) * 7.0;
    if h < f64::from(SEA_LEVEL) {
        h = f64::from(SEA_LEVEL) - (f64::from(SEA_LEVEL) - h) * 0.7;
    }
    h.floor().clamp(2.0, f64::from(WORLD_H - 12)) as i32
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Biome {
    Ocean,
    Beach,
    Desert,
    Plains,
    Forest,
    Snow,
}

fn temperature(seed: f64, wx: f64, wz: f64) -> f64 {
    fbm2(seed + 1300.0, wx / 360.0, wz / 360.0, 3, 2.0, 0.5)
}

fn moisture(seed: f64, wx: f64, wz: f64) -> f64 {
    fbm2(seed + 2600.0, wx / 300.0, wz / 300.0, 3, 2.0, 0.5)
}

#[allow(
    clippy::int_plus_one,
    reason = "Keep the original TypeScript biome threshold."
)]
fn biome_at(seed: f64, wx: f64, wz: f64, h: i32) -> Biome {
    if h <= SEA_LEVEL - 1 {
        return Biome::Ocean;
    }
    if h <= SEA_LEVEL + 1 {
        return Biome::Beach;
    }
    if h >= SEA_LEVEL + 38 {
        return Biome::Snow;
    }
    let t = temperature(seed, wx, wz);
    let m = moisture(seed, wx, wz);
    if t < 0.28 {
        return Biome::Snow;
    }
    if t > 0.55 && m < 0.42 {
        return Biome::Desert;
    }
    if m > 0.55 {
        return Biome::Forest;
    }
    Biome::Plains
}

fn surface_block(biome: Biome) -> u8 {
    match biome {
        Biome::Snow => Block::SNOW,
        Biome::Desert | Biome::Beach | Biome::Ocean => Block::SAND,
        _ => Block::GRASS,
    }
}

fn carve_cave(seed: f64, wx: f64, y: i32, wz: f64) -> bool {
    if !(5..=SEA_LEVEL + 8).contains(&y) {
        return false;
    }
    let n = value_noise3(seed + 101.0, wx / 18.0, f64::from(y) / 14.0, wz / 18.0);
    n > 0.78
}

fn ore_at(seed: f64, wx: f64, y: i32, wz: f64) -> u8 {
    let r = rand3(seed + 55.0, wx, f64::from(y), wz);
    if y < 14 && r > 0.992 {
        return Block::DIAMOND_ORE;
    }
    if y < 22 && r > 0.99 {
        return Block::GOLD_ORE;
    }
    if r > 0.978 {
        return Block::IRON_ORE;
    }
    if r > 0.955 {
        return Block::COAL_ORE;
    }
    Block::STONE
}

fn set(data: &mut [u8], x: i32, y: i32, z: i32, id: u8) {
    if !(0..CHUNK_SX).contains(&x) || !(0..CHUNK_SZ).contains(&z) || !(0..WORLD_H).contains(&y) {
        return;
    }
    data[block_index(x, y, z)] = id;
}

fn stamp_tree(seed: f64, data: &mut [u8], base: [f64; 2], root: [f64; 2], base_y: i32) {
    let [bx, bz] = root;
    let trunk_h = 4 + (rand3(seed + 3.0, bx, 0.0, bz) * 3.0).floor() as i32;
    let top_y = base_y + trunk_h;
    for dy in -2..=1 {
        let ly = top_y + dy;
        let radius = if dy >= 1 { 1 } else { 2 };
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx * dx + dz * dz > radius * radius + 1 {
                    continue;
                }
                let lx = (bx + f64::from(dx) - base[0]) as i32;
                let lz = (bz + f64::from(dz) - base[1]) as i32;
                if !(0..CHUNK_SX).contains(&lx)
                    || !(0..CHUNK_SZ).contains(&lz)
                    || !(0..WORLD_H).contains(&ly)
                {
                    continue;
                }
                let idx = block_index(lx, ly, lz);
                if data[idx] == Block::AIR {
                    data[idx] = Block::LEAVES;
                }
            }
        }
    }
    for i in 0..trunk_h {
        set(
            data,
            (bx - base[0]) as i32,
            base_y + i,
            (bz - base[1]) as i32,
            Block::LOG,
        );
    }
}

fn stamp_cactus(seed: f64, data: &mut [u8], base: [f64; 2], root: [f64; 2], base_y: i32) {
    let [bx, bz] = root;
    let tall = 1 + (rand3(seed + 9.0, bx, 1.0, bz) * 3.0).floor() as i32;
    for i in 0..tall {
        set(
            data,
            (bx - base[0]) as i32,
            base_y + i,
            (bz - base[1]) as i32,
            Block::CACTUS,
        );
    }
}

/// Fill a chunk in the TypeScript x/z/y and vegetation-stamping order.
///
/// `data` must contain at least `CHUNK_CELLS` bytes. The whole slice is cleared,
/// matching `Uint8Array.fill(AIR)`.
pub fn generate_chunk(seed: f64, cx: i32, cz: i32, data: &mut [u8]) {
    assert!(data.len() >= CHUNK_CELLS);
    data.fill(Block::AIR);
    let base_x = f64::from(cx) * f64::from(CHUNK_SX);
    let base_z = f64::from(cz) * f64::from(CHUNK_SZ);
    for x in 0..CHUNK_SX {
        for z in 0..CHUNK_SZ {
            let wx = base_x + f64::from(x);
            let wz = base_z + f64::from(z);
            let h = height_at(seed, wx, wz);
            let biome = biome_at(seed, wx, wz, h);
            let surf = surface_block(biome);
            for y in 0..=h {
                if y == 0 {
                    data[block_index(x, y, z)] = Block::BEDROCK;
                    continue;
                }
                if carve_cave(seed, wx, y, wz) {
                    continue;
                }
                let id = if y == h {
                    surf
                } else if y >= h - 3 {
                    if surf == Block::SAND {
                        Block::SAND
                    } else {
                        Block::DIRT
                    }
                } else {
                    ore_at(seed, wx, y, wz)
                };
                data[block_index(x, y, z)] = id;
            }
            if h < SEA_LEVEL {
                for y in (1..SEA_LEVEL).rev() {
                    if data[block_index(x, y, z)] != Block::AIR {
                        break;
                    }
                    data[block_index(x, y, z)] = Block::WATER;
                }
            }
        }
    }
    for x in -TREE_MARGIN..CHUNK_SX + TREE_MARGIN {
        for z in -TREE_MARGIN..CHUNK_SZ + TREE_MARGIN {
            let wx = base_x + f64::from(x);
            let wz = base_z + f64::from(z);
            let h = height_at(seed, wx, wz);
            if h < SEA_LEVEL + 1 {
                continue;
            }
            if carve_cave(seed, wx, h, wz) {
                continue;
            }
            let biome = biome_at(seed, wx, wz, h);
            let r = rand3(seed + 777.0, wx, 5.0, wz);
            match biome {
                Biome::Forest if r > 0.985 => {
                    stamp_tree(seed, data, [base_x, base_z], [wx, wz], h + 1);
                }
                Biome::Plains if r > 0.995 => {
                    stamp_tree(seed, data, [base_x, base_z], [wx, wz], h + 1);
                }
                Biome::Snow if r > 0.994 => {
                    stamp_tree(seed, data, [base_x, base_z], [wx, wz], h + 1);
                }
                Biome::Desert if rand3(seed + 888.0, wx, 7.0, wz) > 0.99 => {
                    stamp_cactus(seed, data, [base_x, base_z], [wx, wz], h + 1);
                }
                _ => {}
            }
        }
    }
}
