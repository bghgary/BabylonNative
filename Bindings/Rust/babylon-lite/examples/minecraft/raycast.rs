use crate::blocks::{Block, block_def};
use crate::world::World;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RayHit {
    /// Hit block coordinates.
    pub bx: i32,
    pub by: i32,
    pub bz: i32,
    /// Previous cell (placement target); equals the hit cell when starting inside it.
    pub px: i32,
    pub py: i32,
    pub pz: i32,
}

fn selectable(id: u8) -> bool {
    if id == Block::AIR {
        return false;
    }
    block_def(id).is_some_and(|d| !d.fluid)
}

/// Amanatides-Woo traversal; direction is not normalized here.
/// Strict axis comparisons preserve the TypeScript tie-breaking (Z, then Y).
pub fn raycast_voxel(
    world: &mut World,
    origin: [f64; 3],
    direction: [f64; 3],
    max_dist: f64,
) -> Option<RayHit> {
    let [ox, oy, oz] = origin;
    let [dx, dy, dz] = direction;
    let mut x = ox.floor() as i32;
    let mut y = oy.floor() as i32;
    let mut z = oz.floor() as i32;
    let step_x = if dx > 0.0 { 1 } else { -1 };
    let step_y = if dy > 0.0 { 1 } else { -1 };
    let step_z = if dz > 0.0 { 1 } else { -1 };
    let inv_x = if dx != 0.0 {
        1.0 / dx.abs()
    } else {
        f64::INFINITY
    };
    let inv_y = if dy != 0.0 {
        1.0 / dy.abs()
    } else {
        f64::INFINITY
    };
    let inv_z = if dz != 0.0 {
        1.0 / dz.abs()
    } else {
        f64::INFINITY
    };
    let mut t_max_x = if dx != 0.0 {
        (if dx > 0.0 {
            f64::from(x) + 1.0 - ox
        } else {
            ox - f64::from(x)
        }) * inv_x
    } else {
        f64::INFINITY
    };
    let mut t_max_y = if dy != 0.0 {
        (if dy > 0.0 {
            f64::from(y) + 1.0 - oy
        } else {
            oy - f64::from(y)
        }) * inv_y
    } else {
        f64::INFINITY
    };
    let mut t_max_z = if dz != 0.0 {
        (if dz > 0.0 {
            f64::from(z) + 1.0 - oz
        } else {
            oz - f64::from(z)
        }) * inv_z
    } else {
        f64::INFINITY
    };
    let mut px = x;
    let mut py = y;
    let mut pz = z;
    let mut t = 0.0;
    while t <= max_dist {
        if selectable(world.get_block(x, y, z)) {
            return Some(RayHit {
                bx: x,
                by: y,
                bz: z,
                px,
                py,
                pz,
            });
        }
        px = x;
        py = y;
        pz = z;
        if t_max_x < t_max_y && t_max_x < t_max_z {
            x += step_x;
            t = t_max_x;
            t_max_x += inv_x;
        } else if t_max_y < t_max_z {
            y += step_y;
            t = t_max_y;
            t_max_y += inv_y;
        } else {
            z += step_z;
            t = t_max_z;
            t_max_z += inv_z;
        }
    }
    None
}
