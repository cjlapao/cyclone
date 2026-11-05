//! Helpers shared by stages.

use crate::map::{PropKind, WorldMap};
use crate::noise::hash_pair;
use crate::pipeline::{GenCtx, Site};

#[inline]
pub fn dist(ax: f64, ay: f64, bx: f64, by: f64) -> f64 {
    let dx = ax - bx;
    let dy = ay - by;
    libm::sqrt(dx * dx + dy * dy)
}

#[inline]
pub fn tile_dist(ax: u32, ay: u32, bx: u32, by: u32) -> f64 {
    dist(ax as f64, ay as f64, bx as f64, by as f64)
}

/// Mark prop footprints and helipads (+1 ring) occupied on a regen pass.
pub fn mark_props_and_pads(map: &WorldMap, occupied: &mut [bool]) {
    let w = map.size.0;
    let h = map.size.1;
    for p in &map.props {
        for dy in 0..p.footprint.1 as u32 {
            for dx in 0..p.footprint.0 as u32 {
                let (x, y) = (p.tile.0 + dx, p.tile.1 + dy);
                if x < w && y < h {
                    occupied[(y * w + x) as usize] = true;
                }
            }
        }
    }
    for hp in &map.helipads {
        for dy in 0..5u32 {
            for dx in 0..5u32 {
                let x = hp.tile.0 as i64 - 1 + dx as i64;
                let y = hp.tile.1 as i64 - 1 + dy as i64;
                if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
                    occupied[(y as u32 * w + x as u32) as usize] = true;
                }
            }
        }
    }
}

/// Reconstruct sites from a finished map (for `regenerate_round`).
pub fn sites_from_islands(base: &WorldMap) -> Vec<Site> {
    base.islands
        .iter()
        .map(|i| Site {
            centre: i.centre,
            radius: libm::sqrt(i.area as f64 / std::f64::consts::PI) * 2.0,
            archetype: i.archetype,
        })
        .collect()
}

/// Top-of-world height per tile (terrain or prop tops) in world units,
/// used by crate occlusion (spec A5.8.3).
pub fn top_heights(ctx: &GenCtx) -> Vec<f32> {
    let (w, h) = ctx.map.size;
    let lh = ctx.cfg.level_height;
    let mut tops = vec![0f32; (w * h) as usize];
    for y in 0..h {
        for x in 0..w {
            tops[(y * w + x) as usize] = ctx.map.height_at(x, y) as f32 * lh;
        }
    }
    for p in &ctx.map.props {
        // Jetties don't occlude; everything else blocks at its top.
        if p.kind == PropKind::Jetty {
            continue;
        }
        let base_h = ctx.map.height_at(p.tile.0, p.tile.1) as f32 * lh;
        let top = base_h + p.height_levels as f32 * lh;
        for dy in 0..p.footprint.1 as u32 {
            for dx in 0..p.footprint.0 as u32 {
                let (x, y) = (p.tile.0 + dx, p.tile.1 + dy);
                if x < w && y < h {
                    let i = (y * w + x) as usize;
                    if top > tops[i] {
                        tops[i] = top;
                    }
                }
            }
        }
    }
    tops
}

/// Deterministic cosmetic value in [0,1) for (tile, salt) — not an RNG stream.
#[inline]
pub fn tile_tint(x: u32, y: u32, seed: u64, salt: u64) -> f32 {
    let h = hash_pair(seed, hash_pair(u64::from(x), u64::from(y) ^ salt));
    ((h >> 40) as f32) * (1.0 / (1u64 << 24) as f32)
}
