//! Stage 8 — Crates with per-heading occlusion (spec 02 A5.8).
//!
//! Heading convention shared with the renderer: heading h's camera sits at
//! yaw 45°+90°·h, i.e. at grid offset CAM_DIRS[h]; we march crate→camera
//! along that offset, rising by `step × tan(pitch)` per step.

use rand::Rng;

use crate::error::GenError;
use crate::map::CrateSpawn;
use crate::pipeline::GenCtx;
use crate::pipeline::common::{tile_dist, top_heights};

const S: f64 = std::f64::consts::FRAC_1_SQRT_2; // 1/√2

/// Grid offsets from a tile toward each camera heading, unit length.
pub const CAM_DIRS: [(f64, f64); 4] = [(S, S), (S, -S), (-S, -S), (-S, S)];

/// Is (x,y) hidden from heading `h` by terrain or a prop top?
pub fn occluded(ctx: &GenCtx, tops: &[f32], x: u32, y: u32, h: usize) -> bool {
    let (w, hgt) = (ctx.w(), ctx.h());
    let lh = ctx.cfg.level_height as f64;
    // tan(35.264°) == 1/√2 exactly — the true-isometric magic angle.
    let rise_per_unit = S;
    let (dx, dy) = CAM_DIRS[h];
    let mut px = x as f64 + 0.5;
    let mut py = y as f64 + 0.5;
    let mut z = ctx.map.height_at(x, y) as f64 * lh + 0.15;
    let step = 0.25f64;
    for _ in 0..80 {
        px += dx * step;
        py += dy * step;
        z += rise_per_unit * step;
        let tx = px.floor();
        let ty = py.floor();
        if tx < 0.0 || ty < 0.0 || tx >= w as f64 || ty >= hgt as f64 {
            break;
        }
        let top = tops[(ty as u32 * w + tx as u32) as usize] as f64;
        if top > z + 0.02 {
            return true;
        }
    }
    false
}

fn hidden_from(ctx: &GenCtx, tops: &[f32], x: u32, y: u32) -> [bool; 4] {
    let mut out = [false; 4];
    for (h, slot) in out.iter_mut().enumerate() {
        *slot = occluded(ctx, tops, x, y, h);
    }
    out
}

fn walkable_tiles_on(ctx: &GenCtx, idx: usize) -> Vec<(u32, u32)> {
    let mut v = Vec::new();
    let b = ctx.map.islands[idx].bbox;
    for y in b.y0..=b.y1 {
        for x in b.x0..=b.x1 {
            if ctx.map.island_of.raw(x, y) as usize == idx && ctx.walkable(x, y) {
                v.push((x, y));
            }
        }
    }
    v
}

#[derive(Clone)]
struct IslandSpots {
    idx: usize,
    open: Vec<(u32, u32)>,
    /// Tiles hidden from exactly 1..=3 headings, with the count.
    hidden: Vec<((u32, u32), u8)>,
}

/// Pick weight for an island: distance from base + spread from other crate
/// islands + (round ≥ 2) a preference for small islands.
fn weight(ctx: &GenCtx, base: (u32, u32), used: &[usize], round: u32, idx: usize) -> u64 {
    let c = ctx.map.islands[idx].centre;
    let d_base = tile_dist(base.0, base.1, c.0, c.1) as u64;
    let d_spread = used
        .iter()
        .map(|&u| {
            let uc = ctx.map.islands[u].centre;
            tile_dist(c.0, c.1, uc.0, uc.1) as u64
        })
        .min()
        .unwrap_or(400);
    let small = if round >= 2 {
        let area = ctx.map.islands[idx].area;
        ((600 - area.min(600)) / 50) as u64
    } else {
        0
    };
    1 + d_base + d_spread + small * 10
}

fn take(
    ctx: &GenCtx,
    base: (u32, u32),
    used: &[usize],
    round: u32,
    spots: &[IslandSpots],
    rng: &mut impl Rng,
    want_hidden: bool,
) -> Option<usize> {
    let eligible = |s: &IslandSpots| {
        !used.contains(&s.idx)
            && if want_hidden {
                !s.hidden.is_empty()
            } else {
                !s.open.is_empty()
            }
    };
    let wsum: u64 = spots
        .iter()
        .filter(|s| eligible(s))
        .map(|s| weight(ctx, base, used, round, s.idx))
        .sum();
    if wsum == 0 {
        return None;
    }
    let target = rng.random_range(0..wsum);
    let mut acc = 0u64;
    for s in spots {
        if !eligible(s) {
            continue;
        }
        acc += weight(ctx, base, used, round, s.idx);
        if acc > target {
            return Some(s.idx);
        }
    }
    None
}

pub fn stage(ctx: &mut GenCtx, regen: bool) -> Result<(), GenError> {
    let tops = top_heights(ctx);
    let base = ctx.map.islands[0].centre;

    // Classify candidate tiles per island (stride-sampled to keep it snappy).
    let mut spots: Vec<IslandSpots> = Vec::new();
    for idx in 1..ctx.map.islands.len() {
        let tiles = walkable_tiles_on(ctx, idx);
        let mut open = Vec::new();
        let mut hidden = Vec::new();
        let stride = (tiles.len() / 80).max(1);
        for (k, &(x, y)) in tiles.iter().enumerate() {
            if k % stride != 0 {
                continue;
            }
            let hf = hidden_from(ctx, &tops, x, y);
            let trues = hf.iter().filter(|b| **b).count();
            match trues {
                0 => open.push((x, y)),
                1..=3 => hidden.push(((x, y), trues as u8)),
                _ => {}
            }
        }
        spots.push(IslandSpots { idx, open, hidden });
    }

    let want_hidden = ctx.cfg.hidden_crates_min_for_round(ctx.round) as usize;
    let want_open = (ctx.cfg.crates as usize).saturating_sub(want_hidden);

    let mut used: Vec<usize> = Vec::new();
    let mut rng = if regen {
        ctx.round_rng(8, 777)
    } else {
        ctx.stage_rng(8)
    };

    let mut chosen: Vec<CrateSpawn> = Vec::new();
    for (count, want_hidden) in [(want_hidden, true), (want_open, false)] {
        for _ in 0..count {
            let idx = match take(ctx, base, &used, ctx.round, &spots, &mut rng, want_hidden) {
                Some(i) => i,
                None => return Err(GenError::Retry("no hiding spots".into())),
            };
            let s = &spots.iter().find(|s| s.idx == idx).expect("picked");
            let (tile, hf) = if want_hidden {
                // Prefer tiles behind the tallest things (hidden from most
                // headings); ties broken by the island stream.
                let max_score = s.hidden.iter().map(|&(_, sc)| sc).max().unwrap_or(1);
                let top: Vec<_> = s
                    .hidden
                    .iter()
                    .copied()
                    .filter(|&(_, sc)| sc == max_score)
                    .collect();
                let ((x, y), _) = top[rng.random_range(0..top.len())];
                let hf = hidden_from(ctx, &tops, x, y);
                ((x, y), hf)
            } else {
                let (x, y) = s.open[rng.random_range(0..s.open.len())];
                ((x, y), [false; 4])
            };
            used.push(idx);
            ctx.occupy(tile.0, tile.1);
            chosen.push(CrateSpawn {
                tile,
                island: idx as u8,
                hidden_from: hf,
            });
        }
    }

    chosen.sort_by_key(|c| c.island);
    ctx.map.crates = chosen;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::WorldGenConfig;
    use crate::grid::Grid;
    use crate::map::Surface;
    use crate::pipeline::{ProgressNoop, blank_map};
    use crate::seed::Seed;

    #[test]
    fn wall_hides_crate_only_from_the_west_headings() {
        let cfg = WorldGenConfig {
            size: (32, 32),
            island_count: 3,
            ..WorldGenConfig::default()
        };
        let mut map = blank_map(Seed(1), 1, &cfg);
        map.height = Grid::new(32, 32, 1);
        map.surface = Grid::new(32, 32, Surface::Grass);
        for y in 13..=19u32 {
            for x in 14..=16u32 {
                *map.height.get_mut(x, y).unwrap() = 8;
                *map.surface.get_mut(x, y).unwrap() = Surface::Rock;
            }
        }
        let ctx = GenCtx {
            seed: Seed(1),
            round: 1,
            attempt: 0,
            cfg,
            map,
            sites: Vec::new(),
            occupied: vec![false; 32 * 32],
            progress: &ProgressNoop,
        };
        let tops = top_heights(&ctx);
        let hf = hidden_from(&ctx, &tops, 17, 16);
        // Headings 2 and 3 march toward −x and walk into the wall.
        assert!(hf[2] && hf[3], "west wall must hide the crate: {hf:?}");
        assert!(!hf[0] && !hf[1], "east views must stay clear: {hf:?}");
    }
}
