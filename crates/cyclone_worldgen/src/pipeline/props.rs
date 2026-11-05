//! Stage 7 — Props: villages, trees, landmarks, jetties (spec 02 A5.7).
//! Applied per island with the island's own stream. Helipad tiles + a
//! 1-tile ring stay clear (already occupied); props never sit on cliffs
//! and footprints never overlap.

use rand::Rng;

use crate::error::GenError;
use crate::map::{Archetype, Prop, PropKind, Surface};
use crate::pipeline::GenCtx;

fn island_tiles(ctx: &GenCtx, idx: usize) -> Vec<(u32, u32)> {
    let mut v = Vec::new();
    let b = ctx.map.islands[idx].bbox;
    for y in b.y0..=b.y1 {
        for x in b.x0..=b.x1 {
            if ctx.map.island_of.raw(x, y) as usize == idx {
                v.push((x, y));
            }
        }
    }
    v
}

fn push_prop(ctx: &mut GenCtx, kind: PropKind, tile: (u32, u32), facing: u8) {
    let (fw, fd, hh) = kind.dims();
    ctx.map.props.push(Prop {
        kind,
        tile,
        footprint: (fw, fd),
        facing,
        height_levels: hh,
    });
    ctx.occupy_footprint(tile, (fw, fd));
}

/// `kind`'s footprint fits at (x,y) with uniform ground height.
fn footprint_fits(ctx: &GenCtx, kind: PropKind, x: u32, y: u32, level: u8) -> bool {
    let (fw, fd, _) = kind.dims();
    for j in 0..fd as u32 {
        for i in 0..fw as u32 {
            let (tx, ty) = (x + i, y + j);
            if !ctx.walkable(tx, ty) || ctx.map.height_at(tx, ty) != level {
                return false;
            }
        }
    }
    true
}

fn best_sea_dir(ctx: &GenCtx, x: u32, y: u32) -> u8 {
    const D: [(i64, i64, u8); 4] = [(0, -1, 0), (1, 0, 1), (0, 1, 2), (-1, 0, 3)];
    let mut best = (3u8, 100u8);
    for (dx, dy, dir) in D {
        let nx = x as i64 + dx;
        let ny = y as i64 + dy;
        let sd = if nx < 0 || ny < 0 || !ctx.in_bounds(nx as u32, ny as u32) {
            best = (dir, 0);
            break;
        } else {
            ctx.map.shore_dist.raw(nx as u32, ny as u32)
        };
        if sd < best.1 {
            best = (dir, sd);
        }
    }
    best.0
}

fn village(ctx: &mut GenCtx, rng: &mut impl Rng, idx: usize) {
    let cands: Vec<(u32, u32)> = island_tiles(ctx, idx)
        .into_iter()
        .filter(|&(x, y)| ctx.walkable(x, y) && (1..=6).contains(&ctx.map.shore_dist.raw(x, y)))
        .collect();
    if cands.is_empty() {
        return;
    }
    for _ in 0..30 {
        let (ax, ay) = cands[rng.random_range(0..cands.len())];
        let level = ctx.map.height_at(ax, ay);
        let want = rng.random_range(2..=6usize);
        let mut got = 0usize;
        for dy in -2i64..=2 {
            for dx in -2i64..=2 {
                if got >= want {
                    break;
                }
                let tx = ax as i64 + dx;
                let ty = ay as i64 + dy;
                if tx < 0 || ty < 0 {
                    continue;
                }
                let kind = if rng.random_bool(0.6) {
                    PropKind::House
                } else {
                    PropKind::Hut
                };
                let (fw, fd, _) = kind.dims();
                let ox = (tx as u32).saturating_sub(fw as u32 / 2);
                let oy = (ty as u32).saturating_sub(fd as u32 / 2);
                if footprint_fits(ctx, kind, ox, oy, level) {
                    let facing = best_sea_dir(ctx, ox, oy);
                    push_prop(ctx, kind, (ox, oy), facing);
                    got += 1;
                }
            }
        }
        if got >= 2 {
            jetty_for_village(ctx, rng, ax, ay);
            return;
        }
    }
}

fn jetty_for_village(ctx: &mut GenCtx, rng: &mut impl Rng, ax: u32, ay: u32) {
    if !rng.random_bool(0.6) {
        return;
    }
    for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
        let sx = ax as i64 + dx * 2;
        let sy = ay as i64 + dy * 2;
        if sx < 0 || sy < 0 {
            continue;
        }
        let (sx, sy) = (sx as u32, sy as u32);
        if !ctx.in_bounds(sx, sy) || ctx.map.surface.raw(sx, sy) != Surface::Sand {
            continue;
        }
        let (mut x, mut y) = (sx as i64, sy as i64);
        let len = rng.random_range(2..=3i64);
        let mut placed = 0;
        for _ in 0..len {
            x += dx;
            y += dy;
            if x < 0 || y < 0 || !ctx.in_bounds(x as u32, y as u32) {
                break;
            }
            let (tx, ty) = (x as u32, y as u32);
            if ctx.map.surface.raw(tx, ty) != Surface::Shallow || ctx.is_occupied(tx, ty) {
                break;
            }
            // Jetties may sit over shallow water (spec A5.7).
            ctx.map.props.push(Prop {
                kind: PropKind::Jetty,
                tile: (tx, ty),
                footprint: (1, 1),
                facing: if dx != 0 { 1 } else { 0 },
                height_levels: 0,
            });
            ctx.occupy(tx, ty);
            placed += 1;
        }
        if placed >= 2 {
            return;
        }
    }
}

pub fn stage(ctx: &mut GenCtx) -> Result<(), GenError> {
    for idx in 0..ctx.map.islands.len() {
        let mut rng = ctx.island_rng(7, idx as u8);
        let arch = ctx.map.islands[idx].archetype;

        // Villages: 0–2 clusters on Flat/Hilly islands.
        if matches!(arch, Archetype::Flat | Archetype::Hilly) {
            let n = rng.random_range(0..=3usize).min(2);
            for _ in 0..n {
                village(ctx, &mut rng, idx);
            }
        }

        // Trees: min 2-tile spacing.
        let tiles = island_tiles(ctx, idx);
        if tiles.is_empty() {
            continue;
        }
        let budget = (tiles.len() / 14).clamp(4, 24);
        let mut trees: Vec<(u32, u32)> = Vec::new();
        let mut tries = 0;
        while trees.len() < budget && tries < budget * 6 {
            tries += 1;
            let (x, y) = tiles[rng.random_range(0..tiles.len())];
            if !ctx.walkable(x, y) {
                continue;
            }
            if trees.iter().any(|&(tx, ty)| {
                (tx as i64 - x as i64)
                    .abs()
                    .max((ty as i64 - y as i64).abs())
                    < 2
            }) {
                continue;
            }
            let h = ctx.map.height_at(x, y);
            let kind = match ctx.map.surface.raw(x, y) {
                Surface::Sand | Surface::Grass if h <= 3 => Some(PropKind::PalmTree),
                Surface::Grass | Surface::Rock if h >= 4 => Some(PropKind::PineTree),
                _ => None,
            };
            if let Some(k) = kind {
                push_prop(ctx, k, (x, y), rng.random_range(0..4u8));
                trees.push((x, y));
            }
        }

        // Scenic rocks on high, cliffy ground (DECISION: extra flavour;
        // A5.7 does not place Rock props explicitly).
        let rocks = rng.random_range(0..=3usize);
        let mut placed = 0;
        for &(x, y) in &tiles {
            if placed >= rocks {
                break;
            }
            if ctx.walkable(x, y)
                && ctx.map.height_at(x, y) >= 4
                && ctx.map.cliff.raw(x, y) != 0
                && rng.random_bool(0.15)
            {
                push_prop(ctx, PropKind::Rock, (x, y), rng.random_range(0..4u8));
                placed += 1;
            }
        }
    }

    landmarks(ctx);
    Ok(())
}

fn landmarks(ctx: &mut GenCtx) {
    // One Lighthouse on the coast of the first Plateau/Peak island.
    let mut used_for_light = usize::MAX;
    for idx in 1..ctx.map.islands.len() {
        let arch = ctx.map.islands[idx].archetype;
        if !matches!(arch, Archetype::Plateau | Archetype::Peak) {
            continue;
        }
        let tiles = island_tiles(ctx, idx);
        let spot = tiles.iter().copied().find(|&(x, y)| {
            ctx.map.height_at(x, y) >= 2
                && ctx.walkable(x, y)
                && [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dy)| {
                        let nx = x as i64 + dx;
                        let ny = y as i64 + dy;
                        nx < 0
                            || ny < 0
                            || !ctx.in_bounds(nx as u32, ny as u32)
                            || ctx.map.height_at(nx as u32, ny as u32) == 0
                    })
        });
        if let Some((x, y)) = spot {
            push_prop(ctx, PropKind::Lighthouse, (x, y), best_sea_dir(ctx, x, y));
            used_for_light = idx;
            break;
        }
    }
    // One RadioTower on the highest flat spot of another island.
    let mut best: Option<(u8, u32, u32)> = None;
    for idx in 1..ctx.map.islands.len() {
        if idx == used_for_light || ctx.map.islands[idx].archetype == Archetype::Atoll {
            continue;
        }
        let tiles = island_tiles(ctx, idx);
        for (x, y) in tiles {
            let h = ctx.map.height_at(x, y);
            if h < 3 || h <= best.map(|b| b.0).unwrap_or(0) || !ctx.walkable(x, y) {
                continue;
            }
            let tx = x.saturating_sub(1);
            let ty = y.saturating_sub(1);
            let flat = (0..3u32).all(|dy| {
                (0..3u32).all(|dx| {
                    let (ax, ay) = (tx + dx, ty + dy);
                    ctx.in_bounds(ax, ay)
                        && ctx.map.height_at(ax, ay) == h
                        && ctx.map.cliff.raw(ax, ay) == 0
                })
            });
            if flat && ctx.walkable(tx, ty) && ctx.walkable(tx + 1, ty + 1) {
                best = Some((h, tx, ty));
            }
        }
    }
    if let Some((_, x, y)) = best
        && ctx.walkable(x, y)
    {
        push_prop(ctx, PropKind::RadioTower, (x, y), 0);
    }
    // Landmarks are best-effort: maps with no valid spot still play.
}
