//! Stage 6 — Helipads and fuel reach (spec 02 A5.6), plus the Base Island
//! hangar. A pad is a 3×3 block of equal height ≥1, surface Sand|Grass,
//! no cliff bits, on land.

use rand::Rng;

use crate::error::GenError;
use crate::map::{Helipad, Prop, PropKind, Surface};
use crate::pipeline::GenCtx;
use crate::pipeline::common::tile_dist;

/// Centre tile of a valid 3×3 pad with top-left at (x0,y0), or None.
fn pad_here(ctx: &GenCtx, x0: u32, y0: u32) -> Option<(u32, u32)> {
    let mut lvl = None;
    for dy in 0..3u32 {
        for dx in 0..3u32 {
            let (x, y) = (x0 + dx, y0 + dy);
            if !ctx.in_bounds(x, y) || ctx.is_occupied(x, y) {
                return None;
            }
            let h = ctx.map.height_at(x, y);
            if h < 1 || ctx.map.cliff.raw(x, y) != 0 {
                return None;
            }
            match ctx.map.surface.raw(x, y) {
                Surface::Sand | Surface::Grass => {}
                _ => return None,
            }
            if lvl.is_some_and(|l: u8| l != h) {
                return None;
            }
            lvl = Some(h);
        }
    }
    Some((x0 + 1, y0 + 1))
}

/// All valid pad centres on island `idx`, closest to the island centroid
/// first — a centred pad maximises fuel coverage of neighbouring islands.
fn pads_on_island(ctx: &GenCtx, idx: usize) -> Vec<(u32, u32)> {
    let mut out = Vec::new();
    if idx >= ctx.map.islands.len() {
        return out;
    }
    let centre = ctx.map.islands[idx].centre;
    let b = ctx.map.islands[idx].bbox;
    for y in b.y0..b.y1 {
        for x in b.x0..b.x1 {
            if let Some(c) = pad_here(ctx, x, y) {
                out.push(c);
            }
        }
    }
    out.sort_by(|a, b2| {
        let da = tile_dist(a.0, a.1, centre.0, centre.1);
        let db = tile_dist(b2.0, b2.1, centre.0, centre.1);
        da.total_cmp(&db).then(a.0.cmp(&b2.0)).then(a.1.cmp(&b2.1))
    });
    out
}

fn mark_pad(ctx: &mut GenCtx, c: (u32, u32)) {
    for dy in 0..5u32 {
        for dx in 0..5u32 {
            let x = c.0 as i64 - 1 + dx as i64;
            let y = c.1 as i64 - 1 + dy as i64;
            if x >= 0 && y >= 0 && (x as u32) < ctx.w() && (y as u32) < ctx.h() {
                ctx.occupy(x as u32, y as u32);
            }
        }
    }
}

fn min_dist_to_pads(ctx: &GenCtx, x: u32, y: u32) -> f64 {
    ctx.map
        .helipads
        .iter()
        .map(|p| tile_dist(x, y, p.tile.0, p.tile.1))
        .fold(f64::INFINITY, f64::min)
}

/// All land tiles of island `idx` (its "coast" for reach purposes).
fn coast_tiles(ctx: &GenCtx, idx: usize) -> Vec<(u32, u32)> {
    let isl = &ctx.map.islands[idx];
    let mut out = Vec::new();
    for y in isl.bbox.y0..=isl.bbox.y1 {
        for x in isl.bbox.x0..=isl.bbox.x1 {
            if ctx.map.island_of.raw(x, y) as usize == idx {
                out.push((x, y));
            }
        }
    }
    out
}

/// Best pad spot on island `idx`: maximises how many of `targets`' tile
/// sets are within `reach`, tie-break closest to the island centroid.
/// (The spec fixes *which island* the pad goes on; the exact spot inside
/// it is our choice, so we spend it on fuel coverage.)
fn best_spot(
    ctx: &GenCtx,
    idx: usize,
    targets: &[Vec<(u32, u32)>],
    reach: f64,
) -> Option<((u32, u32), u32)> {
    let centre = ctx.map.islands[idx].centre;
    let b = ctx.map.islands[idx].bbox;
    let mut best: Option<(u32, f64, (u32, u32))> = None; // gain, centroid dist, spot
    for y0 in b.y0..b.y1 {
        for x0 in b.x0..b.x1 {
            let Some(c) = pad_here(ctx, x0, y0) else {
                continue;
            };
            let mut gain = 0u32;
            for tiles in targets {
                if tiles
                    .iter()
                    .any(|&(x, y)| tile_dist(c.0, c.1, x, y) <= reach)
                {
                    gain += 1;
                }
            }
            let dcent = tile_dist(c.0, c.1, centre.0, centre.1);
            let better = match best {
                None => true,
                Some((g, d, _)) => gain > g || (gain == g && dcent < d),
            };
            if better {
                best = Some((gain, dcent, c));
            }
        }
    }
    best.map(|(g, _, c)| (c, g))
}

/// Fuel state (spec A5.6.3): pad graph (edge ≤ 0.8·fuel) connectivity and
/// the islands not within 0.4·fuel of any pad.
fn fuel_state(ctx: &GenCtx) -> (bool, Vec<usize>) {
    let pads = &ctx.map.helipads;
    if pads.is_empty() {
        return (true, (0..ctx.map.islands.len()).collect());
    }
    let limit = ctx.cfg.fuel_range as f64 * 0.8;
    let mut vis = vec![false; pads.len()];
    let mut stack = vec![0usize];
    vis[0] = true;
    let mut count = 1;
    while let Some(i) = stack.pop() {
        for j in 0..pads.len() {
            if !vis[j]
                && tile_dist(
                    pads[i].tile.0,
                    pads[i].tile.1,
                    pads[j].tile.0,
                    pads[j].tile.1,
                ) <= limit
            {
                vis[j] = true;
                count += 1;
                stack.push(j);
            }
        }
    }
    let disconnected = count != pads.len();
    let reach = ctx.cfg.fuel_range as f64 * 0.4;
    let mut unreached = Vec::new();
    for (idx, isl) in ctx.map.islands.iter().enumerate() {
        let mut best = f64::INFINITY;
        for y in isl.bbox.y0..=isl.bbox.y1 {
            for x in isl.bbox.x0..=isl.bbox.x1 {
                if ctx.map.island_of.raw(x, y) as usize == idx {
                    best = best.min(min_dist_to_pads(ctx, x, y));
                }
            }
        }
        if best > reach {
            unreached.push(idx);
        }
    }
    (disconnected, unreached)
}

/// Component label of every existing pad in the 0.8·fuel graph.
fn pad_components(ctx: &GenCtx) -> Vec<usize> {
    let pads = &ctx.map.helipads;
    let limit = ctx.cfg.fuel_range as f64 * 0.8;
    let mut label = vec![usize::MAX; pads.len()];
    let mut n = 0;
    for s in 0..pads.len() {
        if label[s] != usize::MAX {
            continue;
        }
        label[s] = n;
        let mut stack = vec![s];
        while let Some(i) = stack.pop() {
            for j in 0..pads.len() {
                if label[j] == usize::MAX
                    && tile_dist(
                        pads[i].tile.0,
                        pads[i].tile.1,
                        pads[j].tile.0,
                        pads[j].tile.1,
                    ) <= limit
                {
                    label[j] = n;
                    stack.push(j);
                }
            }
        }
        n += 1;
    }
    label
}

fn bad_reason(ctx: &GenCtx, disconnected: bool, unreached: &[usize]) -> String {
    let mut parts = Vec::new();
    if disconnected {
        parts.push("fuel graph disconnected".to_string());
    }
    for &i in unreached {
        let mut best = f64::INFINITY;
        let isl = &ctx.map.islands[i];
        for y in isl.bbox.y0..=isl.bbox.y1 {
            for x in isl.bbox.x0..=isl.bbox.x1 {
                if ctx.map.island_of.raw(x, y) as usize == i {
                    best = best.min(min_dist_to_pads(ctx, x, y));
                }
            }
        }
        parts.push(format!("island {} out of fuel reach ({best:.0})", isl.name));
    }
    format!("fuel reach: {}", parts.join("; "))
}

pub fn stage(ctx: &mut GenCtx) -> Result<(), GenError> {
    let mut rng = ctx.stage_rng(6);
    #[cfg(feature = "diag")]
    {
        for idx in 0..ctx.map.islands.len() {
            let isl = &ctx.map.islands[idx];
            eprintln!(
                "DIAG {idx} {} {:?} area={} peak={} spots={}",
                isl.name,
                isl.archetype,
                isl.area,
                isl.peak_level,
                pads_on_island(ctx, idx).len()
            );
        }
    }

    // 1. Base Island must have one; flatten the best spot if none exists.
    let base_pads = pads_on_island(ctx, 0);
    let base_centre = ctx.map.islands[0].centre;
    let base_pad = if let Some(c) = base_pads.first().copied() {
        c
    } else {
        let (bx, by) = base_centre;
        // Flatten the 3×3 near the centre with the smallest total change.
        let mut best: Option<(u32, u32, u32, u8)> = None; // centre, cost, level
        for dy in -6i64..=6 {
            for dx in -6i64..=6 {
                let x0 = bx as i64 + dx;
                let y0 = by as i64 + dy;
                if x0 < 1 || y0 < 1 || (x0 + 3) as u32 >= ctx.w() || (y0 + 3) as u32 >= ctx.h() {
                    continue;
                }
                let mut hs = Vec::with_capacity(9);
                for j in 0..3u32 {
                    for i in 0..3u32 {
                        hs.push(
                            ctx.map
                                .height_at((x0 + i as i64) as u32, (y0 + j as i64) as u32),
                        );
                    }
                }
                if hs.iter().any(|&h| h < 1) {
                    continue; // don't flatten into the sea
                }
                let target = *hs.iter().max().expect("9 sampled");
                let cost: u32 = hs.iter().map(|&h| h.abs_diff(target) as u32).sum::<u32>()
                    + (dx * dx + dy * dy) as u32;
                let cur_best = best.map(|b| b.2).unwrap_or(u32::MAX);
                if cost < cur_best {
                    best = Some(((x0 + 1) as u32, (y0 + 1) as u32, cost, target));
                }
            }
        }
        let Some((cx, cy, _cost, level)) = best else {
            return Err(GenError::Retry("no base island helipad spot".into()));
        };
        for j in 0..3u32 {
            for i in 0..3u32 {
                *ctx.map
                    .height
                    .get_mut(cx - 1 + i, cy - 1 + j)
                    .expect("in bounds") = level;
            }
        }
        // Recompute surface + cliffs locally (A5.6.1).
        super::surface::stage(ctx)?;
        (cx, cy)
    };
    ctx.map.helipads.push(Helipad {
        tile: base_pad,
        island: 0,
    });
    mark_pad(ctx, base_pad);

    // 2. Extra pads (spec A5.6.2). The spec's own fix rule (A5.6.4, "add a
    // helipad on the island that fixes the most violations") is a coverage
    // greedy; we use that same rule here, because pure farthest-point
    // selection provably cannot satisfy the A5.6.3 invariant at the spec
    // defaults (2-hop islands land at ~45 tiles > 0.4·fuel — see
    // DECISIONS.md). Tie-break prefers the island farthest from any pad.
    let want_extra = rng.random_range(ctx.cfg.helipads_extra.0..=ctx.cfg.helipads_extra.1) as usize;
    let mut chosen: Vec<usize> = Vec::new();
    while chosen.len() < want_extra {
        let (_, unreached) = fuel_state(ctx);
        let reach = ctx.cfg.fuel_range as f64 * 0.4;
        let targets: Vec<Vec<(u32, u32)>> =
            unreached.iter().map(|&u| coast_tiles(ctx, u)).collect();
        let mut best: Option<(u32, f64, usize, (u32, u32))> = None; // gain, spread, island, spot
        for idx in 1..ctx.map.islands.len() {
            if chosen.contains(&idx) || ctx.map.helipads.iter().any(|p| p.island as usize == idx) {
                continue;
            }
            let Some((c, gain)) = best_spot(ctx, idx, &targets, reach) else {
                continue;
            };
            let spread = ctx
                .map
                .helipads
                .iter()
                .map(|p| tile_dist(c.0, c.1, p.tile.0, p.tile.1))
                .fold(f64::INFINITY, f64::min);
            let better = match best {
                None => true,
                Some((g, sp, _, _)) => gain > g || (gain == g && spread > sp),
            };
            if better {
                best = Some((gain, spread, idx, c));
            }
        }
        match best {
            Some((_, _, idx, c)) => {
                chosen.push(idx);
                ctx.map.helipads.push(Helipad {
                    tile: c,
                    island: idx as u8,
                });
                mark_pad(ctx, c);
            }
            None => break, // no more islands have room; fuel check below decides
        }
    }

    // 3/4. Fuel check with up to 2 fixer pads on the island that fixes the
    // most violations (spec A5.6.4).
    let mut fixes = 0;
    loop {
        let (disconnected, unreached) = fuel_state(ctx);
        if !disconnected && unreached.is_empty() {
            break;
        }
        if fixes >= 2 {
            return Err(GenError::Retry(bad_reason(ctx, disconnected, &unreached)));
        }
        fixes += 1;
        let reach = ctx.cfg.fuel_range as f64 * 0.4;
        let limit = ctx.cfg.fuel_range as f64 * 0.8;
        let labels = pad_components(ctx);
        let targets: Vec<Vec<(u32, u32)>> =
            unreached.iter().map(|&u| coast_tiles(ctx, u)).collect();
        let mut best: Option<(u32, usize, (u32, u32))> = None; // gain, island, spot
        for idx in 1..ctx.map.islands.len() {
            if ctx.map.helipads.iter().any(|p| p.island as usize == idx) {
                continue;
            }
            let Some((c, mut gain)) = best_spot(ctx, idx, &targets, reach) else {
                continue;
            };
            if disconnected {
                let mut near: Vec<usize> = Vec::new();
                for (pi, p) in ctx.map.helipads.iter().enumerate() {
                    let d = tile_dist(c.0, c.1, p.tile.0, p.tile.1);
                    if d <= limit && !near.contains(&labels[pi]) {
                        near.push(labels[pi]);
                    }
                }
                if near.len() >= 2 {
                    gain += 10 + near.len() as u32;
                }
            }
            if gain > 0 && best.is_none_or(|(g, _, _)| gain > g) {
                best = Some((gain, idx, c));
            }
        }
        match best {
            Some((_, idx, c)) => {
                ctx.map.helipads.push(Helipad {
                    tile: c,
                    island: idx as u8,
                });
                mark_pad(ctx, c);
            }
            None => return Err(GenError::Retry(bad_reason(ctx, disconnected, &unreached))),
        }
    }

    // Hangar beside the base pad (A5.6). Try the 8 positions around the
    // 3×3 pad; 2×2 flat footprint on the pad's level.
    let pad_level = ctx.map.height_at(base_pad.0, base_pad.1);
    let mut placed = false;
    'spots: for dy in [-2i64, -1, 2, 3] {
        for dx in [-2i64, -1, 2, 3] {
            let x0 = base_pad.0 as i64 - 1 + dx;
            let y0 = base_pad.1 as i64 - 1 + dy;
            if x0 < 0 || y0 < 0 {
                continue;
            }
            let (x0, y0) = (x0 as u32, y0 as u32);
            let mut ok = true;
            for j in 0..2u32 {
                for i in 0..2u32 {
                    let (x, y) = (x0 + i, y0 + j);
                    if !ctx.in_bounds(x, y)
                        || ctx.map.height_at(x, y) != pad_level
                        || ctx.map.cliff.raw(x, y) != 0
                        || ctx.is_occupied(x, y)
                    {
                        ok = false;
                    }
                }
            }
            if ok {
                ctx.map.props.push(Prop {
                    kind: PropKind::Hangar,
                    tile: (x0, y0),
                    footprint: (2, 2),
                    facing: 0,
                    height_levels: 2,
                });
                ctx.occupy_footprint((x0, y0), (2, 2));
                placed = true;
                break 'spots;
            }
        }
    }
    if !placed {
        return Err(GenError::Retry("no room for the base hangar".into()));
    }
    Ok(())
}
