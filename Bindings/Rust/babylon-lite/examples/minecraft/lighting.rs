//! Port of `lighting.ts`: the day-night clock and every colour derived from it.
//! timeOfDay is in [0,1): 0 = midnight, 0.25 = sunrise, 0.5 = noon, 0.75 = sunset.

use crate::voxel_material::Rgb;

const DAY_AMBIENT: Rgb = [0.45, 0.48, 0.55];
const NIGHT_AMBIENT: Rgb = [0.12, 0.14, 0.22];
const SUNSET_AMBIENT_TINT: Rgb = [0.12, 0.05, 0.02];
const DAY_SUN: Rgb = [0.55, 0.5, 0.42];
const SUNSET_SUN: Rgb = [0.85, 0.42, 0.16];
const DAY_FOG: Rgb = [0.7, 0.82, 0.92];
const NIGHT_FOG: Rgb = [0.03, 0.04, 0.09];
const SUNSET_FOG: Rgb = [0.8, 0.52, 0.4];
const DAY_ZENITH: Rgb = [0.28, 0.5, 0.86];
const NIGHT_ZENITH: Rgb = [0.02, 0.03, 0.08];
const SUNSET_ZENITH: Rgb = [0.3, 0.3, 0.55];

fn clamp01(x: f64) -> f64 {
    x.clamp(0.0, 1.0)
}

fn smoothstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    let t = clamp01((x - edge0) / (edge1 - edge0));
    t * t * (3.0 - 2.0 * t)
}

fn lerp3(a: Rgb, b: Rgb, t: f64) -> Rgb {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn add3(a: Rgb, b: Rgb, scale: f64) -> Rgb {
    [
        a[0] + b[0] * scale,
        a[1] + b[1] * scale,
        a[2] + b[2] * scale,
    ]
}

pub struct Lighting {
    day_length: f64,
    time_of_day: f64,
    pub sun_dir: Rgb,
    pub sun_color: Rgb,
    pub ambient: Rgb,
    pub fog: Rgb,
    pub zenith: Rgb,
}

impl Lighting {
    pub fn new(day_length_sec: f64, start_time_of_day: f64) -> Self {
        let mut lighting = Self {
            day_length: day_length_sec,
            time_of_day: (start_time_of_day % 1.0 + 1.0) % 1.0,
            sun_dir: [0.0, 1.0, 0.0],
            sun_color: [0.0; 3],
            ambient: [0.0; 3],
            fog: [0.0; 3],
            zenith: [0.0; 3],
        };
        lighting.recompute();
        lighting
    }

    pub fn tick(&mut self, dt_sec: f64) {
        self.time_of_day = (self.time_of_day + dt_sec / self.day_length) % 1.0;
        self.recompute();
    }

    fn recompute(&mut self) {
        let theta = (self.time_of_day - 0.25) * std::f64::consts::PI * 2.0;
        let (sx, sy, sz) = (theta.cos(), theta.sin(), 0.3);
        let inv = 1.0 / (sx * sx + sy * sy + sz * sz).sqrt();
        self.sun_dir = [sx * inv, sy * inv, sz * inv];
        let elev = self.sun_dir[1];

        let day = smoothstep(-0.05, 0.25, elev);
        let near_horizon = clamp01(1.0 - elev.abs() / 0.25);
        let twilight = near_horizon * near_horizon * smoothstep(-0.2, -0.04, elev);

        self.ambient = add3(
            lerp3(NIGHT_AMBIENT, DAY_AMBIENT, day),
            SUNSET_AMBIENT_TINT,
            twilight,
        );

        let sun_intensity = smoothstep(-0.05, 0.15, elev);
        let warm_to_white = smoothstep(0.08, 0.35, elev);
        self.sun_color = lerp3(SUNSET_SUN, DAY_SUN, warm_to_white).map(|c| c * sun_intensity);

        let fog = lerp3(NIGHT_FOG, DAY_FOG, day);
        self.fog = lerp3(fog, SUNSET_FOG, twilight * 0.7);

        let zenith = lerp3(NIGHT_ZENITH, DAY_ZENITH, day);
        self.zenith = lerp3(zenith, SUNSET_ZENITH, twilight * 0.5);
    }

    /// 24-hour clock text, e.g. "06:45".
    pub fn clock_text(&self) -> String {
        let minutes = (self.time_of_day * 24.0 * 60.0).floor() as u32;
        format!("{:02}:{:02}", minutes / 60, minutes % 60)
    }
}
