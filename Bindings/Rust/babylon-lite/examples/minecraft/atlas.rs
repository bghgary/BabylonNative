//! Port of `atlas.ts`: composes block-face PNGs into one texture with 64 px
//! cells and half-texel-inset UV rects.

use crate::mesher::{AtlasLayout, TileRect};
use babylon_lite::{Engine, PixelsTexture2DOptions, Result, Texture2D};
use std::collections::HashMap;
use std::path::Path;

const TILE_PX: usize = 64;

pub struct BlockAtlas {
    pub texture: Texture2D,
    pub layout: AtlasLayout,
}

/// Decodes a PNG to straight-alpha RGBA8.
fn load_png(path: &Path) -> Option<(Vec<u8>, usize, usize)> {
    let mut decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(path).ok()?));
    decoder.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
    let mut reader = decoder.read_info().ok()?;
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buffer).ok()?;
    let (w, h) = (info.width as usize, info.height as usize);
    let rgba = match info.color_type {
        png::ColorType::Rgba => buffer[..w * h * 4].to_vec(),
        png::ColorType::Rgb => buffer[..w * h * 3]
            .chunks(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        png::ColorType::GrayscaleAlpha => buffer[..w * h * 2]
            .chunks(2)
            .flat_map(|p| [p[0], p[0], p[0], p[1]])
            .collect(),
        png::ColorType::Grayscale => buffer[..w * h]
            .iter()
            .flat_map(|&g| [g, g, g, 255])
            .collect(),
        png::ColorType::Indexed => return None,
    };
    Some((rgba, w, h))
}

/// Resamples `src` into a TILE_PX cell of `atlas` the way canvas `drawImage`
/// does: filtering in premultiplied alpha. Every pack tile is an integer
/// multiple of TILE_PX, which makes bilinear filtering a box average.
fn draw_tile(
    atlas: &mut [u8],
    atlas_w: usize,
    ox: usize,
    oy: usize,
    src: &[u8],
    sw: usize,
    sh: usize,
) {
    let (fx, fy) = ((sw / TILE_PX).max(1), (sh / TILE_PX).max(1));
    for y in 0..TILE_PX {
        for x in 0..TILE_PX {
            let mut sum = [0u32; 4];
            for dy in 0..fy {
                for dx in 0..fx {
                    let sx = (x * sw / TILE_PX + dx).min(sw - 1);
                    let sy = (y * sh / TILE_PX + dy).min(sh - 1);
                    let p = &src[(sy * sw + sx) * 4..][..4];
                    let a = u32::from(p[3]);
                    for c in 0..3 {
                        sum[c] += u32::from(p[c]) * a;
                    }
                    sum[3] += a;
                }
            }
            let n = (fx * fy) as u32;
            let out = &mut atlas[((oy + y) * atlas_w + ox + x) * 4..][..4];
            let alpha = sum[3];
            for c in 0..3 {
                out[c] = (sum[c] + alpha / 2).checked_div(alpha).unwrap_or(0) as u8;
            }
            out[3] = ((alpha + n / 2) / n) as u8;
        }
    }
}

pub fn build_block_atlas(
    engine: &Engine,
    pack_dir: &Path,
    tile_names: &[&str],
) -> Result<BlockAtlas> {
    let mut names: Vec<&str> = Vec::new();
    for name in tile_names {
        if !names.contains(name) {
            names.push(name);
        }
    }
    let cols = (names.len() as f64).sqrt().ceil() as usize;
    let rows = names.len().div_ceil(cols);
    let (atlas_w, atlas_h) = (cols * TILE_PX, rows * TILE_PX);
    let mut pixels = vec![0u8; atlas_w * atlas_h * 4];
    let half_u = 0.5 / atlas_w as f64;
    let half_v = 0.5 / atlas_h as f64;
    let mut rects = HashMap::new();
    let mut first = None;

    for (i, name) in names.iter().enumerate() {
        let (ox, oy) = ((i % cols) * TILE_PX, (i / cols) * TILE_PX);
        match load_png(&pack_dir.join(format!("{name}.png"))) {
            Some((rgba, w, h)) => draw_tile(&mut pixels, atlas_w, ox, oy, &rgba, w, h),
            None => {
                // Magenta placeholder so a missing texture is obvious.
                for y in 0..TILE_PX {
                    for x in 0..TILE_PX {
                        pixels[((oy + y) * atlas_w + ox + x) * 4..][..4]
                            .copy_from_slice(&[255, 0, 255, 255]);
                    }
                }
            }
        }
        let rect = TileRect {
            u0: ox as f64 / atlas_w as f64 + half_u,
            v0: oy as f64 / atlas_h as f64 + half_v,
            u1: (ox + TILE_PX) as f64 / atlas_w as f64 - half_u,
            v1: (oy + TILE_PX) as f64 / atlas_h as f64 - half_v,
        };
        first.get_or_insert(rect);
        rects.insert((*name).to_owned(), rect);
    }

    let texture = engine.create_texture_2d_from_pixels(
        &pixels,
        atlas_w as u32,
        atlas_h as u32,
        &PixelsTexture2DOptions {
            srgb: true,
            ..Default::default()
        },
    )?;
    let fallback = first.unwrap_or(TileRect {
        u0: 0.0,
        v0: 0.0,
        u1: 1.0,
        v1: 1.0,
    });
    Ok(BlockAtlas {
        texture,
        layout: AtlasLayout { rects, fallback },
    })
}
