//! Stage 9 — Refugees (spec 02 A5.9): `refugee_islands` islands excluding
//! Base Island, `refugees_per_island` each, near houses or on beaches,
//! ≥ 2 tiles from crates and helipads, on walkable free tiles.

use rand::Rng;

use crate::error::GenError;
use crate::map::{PropKind, RefugeeSpawn, Surface};
use crate::pipeline::GenCtx;

pub fn stage(ctx: &mut GenCtx, regen: bool) -> Result<(), GenError> {
    let mut rng = if regen {
        ctx.round_rng(9, 777)
    } else {
        ctx.stage_rng(9)
    };
    let want_islands =
        rng.random_range(ctx.cfg.refugee_islands.0..=ctx.cfg.refugee_islands.1) as usize;
    let n = ctx.map.islands.len().max(2);

    // Sample distinct non-base islands.
    let mut islands: Vec<usize> = (1..n).collect();
    let mut chosen: Vec<usize> = Vec::new();
    while chosen.len() < want_islands.min(islands.len()) {
        let i = rng.random_range(0..islands.len());
        chosen.push(islands.swap_remove(i));
    }
    chosen.sort_unstable();

    for idx in chosen {
        let count = rng.random_range(ctx.cfg.refugees_per_island.0..=ctx.cfg.refugees_per_island.1)
            as usize;

        // Candidate tiles: beside houses first, then beaches.
        let mut near_houses: Vec<(u32, u32)> = Vec::new();
        for p in &ctx.map.props {
            if matches!(p.kind, PropKind::House | PropKind::Hut)
                && ctx.map.island_of.raw(p.tile.0, p.tile.1) as usize == idx
            {
                for dy in -1i64..=p.footprint.1 as i64 {
                    for dx in -1i64..=p.footprint.0 as i64 {
                        let nx = p.tile.0 as i64 + dx;
                        let ny = p.tile.1 as i64 + dy;
                        if nx >= 0 && ny >= 0 {
                            near_houses.push((nx as u32, ny as u32));
                        }
                    }
                }
            }
        }
        let b = ctx.map.islands[idx].bbox;
        let mut beaches: Vec<(u32, u32)> = Vec::new();
        for y in b.y0..=b.y1 {
            for x in b.x0..=b.x1 {
                if ctx.map.island_of.raw(x, y) as usize == idx
                    && ctx.map.surface.raw(x, y) == Surface::Sand
                {
                    beaches.push((x, y));
                }
            }
        }

        let mut placed = 0;
        for pool in [near_houses, beaches] {
            for &tile in &pool {
                if placed >= count {
                    break;
                }
                if !ctx.walkable(tile.0, tile.1) {
                    continue;
                }
                let ok = ctx.map.crates.iter().all(|c| {
                    (c.tile.0 as i64 - tile.0 as i64)
                        .abs()
                        .max((c.tile.1 as i64 - tile.1 as i64).abs())
                        >= 2
                }) && ctx.map.helipads.iter().all(|p| {
                    (p.tile.0 as i64 - tile.0 as i64)
                        .abs()
                        .max((p.tile.1 as i64 - tile.1 as i64).abs())
                        >= 2
                });
                if !ok {
                    continue;
                }
                ctx.occupy(tile.0, tile.1);
                ctx.map.refugees.push(RefugeeSpawn {
                    tile,
                    island: idx as u8,
                });
                placed += 1;
            }
            if placed >= count {
                break;
            }
        }
        // An island that yields nobody keeps the round playable (invariants
        // do not pin the refugee total), but zero everywhere would be a bug
        // — the metrics stage flags totals of 0.
    }
    if ctx.map.refugees.is_empty() {
        return Err(GenError::Retry("no refugees fit anywhere".into()));
    }
    Ok(())
}
