/// JavaScript ToUint32: unlike Rust's float cast, out-of-range numbers wrap.
fn to_u32(n: f64) -> u32 {
    if !n.is_finite() || n == 0.0 {
        return 0;
    }
    let r = n.trunc() % 4_294_967_296.0;
    let r = if r < 0.0 { r + 4_294_967_296.0 } else { r };
    r as u32
}

fn hash2(seed: f64, ix: f64, iy: f64) -> f64 {
    let mut h =
        to_u32(seed) ^ to_u32(ix).wrapping_mul(374_761_393) ^ to_u32(iy).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    f64::from(h) / 4_294_967_296.0
}

fn hash3(seed: f64, ix: f64, iy: f64, iz: f64) -> f64 {
    let mut h = to_u32(seed)
        ^ to_u32(ix).wrapping_mul(374_761_393)
        ^ to_u32(iy).wrapping_mul(668_265_263)
        ^ to_u32(iz).wrapping_mul(2_147_483_647);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 15;
    f64::from(h) / 4_294_967_296.0
}

fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

pub fn value_noise2(seed: f64, x: f64, y: f64) -> f64 {
    let x0 = x.floor();
    let y0 = y.floor();
    let fx = smooth(x - x0);
    let fy = smooth(y - y0);
    let v00 = hash2(seed, x0, y0);
    let v10 = hash2(seed, x0 + 1.0, y0);
    let v01 = hash2(seed, x0, y0 + 1.0);
    let v11 = hash2(seed, x0 + 1.0, y0 + 1.0);
    lerp(lerp(v00, v10, fx), lerp(v01, v11, fx), fy)
}

pub fn value_noise3(seed: f64, x: f64, y: f64, z: f64) -> f64 {
    let x0 = x.floor();
    let y0 = y.floor();
    let z0 = z.floor();
    let fx = smooth(x - x0);
    let fy = smooth(y - y0);
    let fz = smooth(z - z0);
    let c000 = hash3(seed, x0, y0, z0);
    let c100 = hash3(seed, x0 + 1.0, y0, z0);
    let c010 = hash3(seed, x0, y0 + 1.0, z0);
    let c110 = hash3(seed, x0 + 1.0, y0 + 1.0, z0);
    let c001 = hash3(seed, x0, y0, z0 + 1.0);
    let c101 = hash3(seed, x0 + 1.0, y0, z0 + 1.0);
    let c011 = hash3(seed, x0, y0 + 1.0, z0 + 1.0);
    let c111 = hash3(seed, x0 + 1.0, y0 + 1.0, z0 + 1.0);
    let x00 = lerp(c000, c100, fx);
    let x10 = lerp(c010, c110, fx);
    let x01 = lerp(c001, c101, fx);
    let x11 = lerp(c011, c111, fx);
    lerp(lerp(x00, x10, fy), lerp(x01, x11, fy), fz)
}

/// Fractional Brownian motion. TypeScript defaults: lacunarity = 2, gain = 0.5.
pub fn fbm2(seed: f64, x: f64, y: f64, octaves: u32, lacunarity: f64, gain: f64) -> f64 {
    let mut amp = 1.0;
    let mut freq = 1.0;
    let mut sum = 0.0;
    let mut norm = 0.0;
    for o in 0..octaves {
        sum += amp * value_noise2(seed + f64::from(o) * 1013.0, x * freq, y * freq);
        norm += amp;
        amp *= gain;
        freq *= lacunarity;
    }
    sum / norm
}

pub fn rand3(seed: f64, x: f64, y: f64, z: f64) -> f64 {
    hash3(seed, x, y, z)
}
