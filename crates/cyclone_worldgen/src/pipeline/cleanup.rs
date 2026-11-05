//! Stage 4 — Clean up and label (spec 02 A5.4). Flood-fill land components,
//! drop specks, fill single-tile holes, require one site per component,
//! fill `island_of`, compute island stats + names + `shore_dist`.

use std::collections::VecDeque;

use crate::error::GenError;
use crate::map::{Archetype, Island, NO_ISLAND, Rect};
use crate::names;
use crate::pipeline::GenCtx;
use crate::pipeline::common::tile_dist;

const NEIGH4: [(i64, i64); 4] = [(0, -1), (1, 0), (0, 1), (-1, 0)];

pub fn stage(ctx: &mut GenCtx) -> Result<(), GenError> {
    let (w, h) = (ctx.w(), ctx.h());
    let land = |ctx: &GenCtx, x: u32, y: u32| ctx.map.height_at(x, y) > 0;

    // ---- components (4-connected) ------------------------------------
    let mut comp = vec![u32::MAX; (w * h) as usize];
    let mut comps: Vec<Vec<(u32, u32)>> = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if land(ctx, x, y) && comp[i] == u32::MAX {
                let id = comps.len() as u32;
                let mut tiles = Vec::new();
                let mut q = VecDeque::new();
                q.push_back((x, y));
                comp[i] = id;
                while let Some((cx, cy)) = q.pop_front() {
                    tiles.push((cx, cy));
                    for (dx, dy) in NEIGH4 {
                        let nx = cx as i64 + dx;
                        let ny = cy as i64 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                            continue;
                        }
                        let j = (ny as u32 * w + nx as u32) as usize;
                        if comp[j] == u32::MAX && land(ctx, nx as u32, ny as u32) {
                            comp[j] = id;
                            q.push_back((nx as u32, ny as u32));
                        }
                    }
                }
                comps.push(tiles);
            }
        }
    }

    // ---- specks out ---------------------------------------------------
    for tiles_list in comps.iter_mut() {
        if tiles_list.len() < 8 {
            let tiles = std::mem::take(tiles_list);
            for (x, y) in tiles {
                if let Some(v) = ctx.map.height.get_mut(x, y) {
                    *v = 0;
                }
                comp[(y * w + x) as usize] = u32::MAX;
            }
        }
    }

    // Components holding an Atoll (lagoon must stay sea, spec A5.4).
    let mut lagoon_comps: Vec<u32> = Vec::new();
    for site in &ctx.sites {
        if site.archetype == Archetype::Atoll {
            for k in 0..8u32 {
                let ang = f64::from(k) * std::f64::consts::FRAC_PI_4;
                let px = ((f64::from(site.centre.0) + libm::cos(ang) * site.radius * 0.62)
                    .round()
                    .clamp(0.0, f64::from(w) - 1.0)) as u32;
                let py = ((f64::from(site.centre.1) + libm::sin(ang) * site.radius * 0.62)
                    .round()
                    .clamp(0.0, f64::from(h) - 1.0)) as u32;
                let cid = comp[(py * w + px) as usize];
                if cid != u32::MAX {
                    lagoon_comps.push(cid);
                    break;
                }
            }
        }
    }

    // ---- single-tile water holes in ------------------------------------
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            if ctx.map.height_at(x, y) != 0 {
                continue;
            }
            let nb: Vec<u8> = NEIGH4
                .iter()
                .map(|(dx, dy)| {
                    ctx.map
                        .height_at((x as i64 + dx) as u32, (y as i64 + dy) as u32)
                })
                .collect();
            if nb.iter().all(|&v| v > 0) {
                let lagoon = NEIGH4.iter().any(|(dx, dy)| {
                    let c = comp[((y as i64 + dy) as u32 * w + (x as i64 + dx) as u32) as usize];
                    lagoon_comps.contains(&c)
                });
                if lagoon {
                    continue;
                }
                let fill = *nb.iter().min().unwrap_or(&2);
                if let Some(v) = ctx.map.height.get_mut(x, y) {
                    *v = fill;
                }
                // Recompute its component lazily via island_of fill below;
                // it is adjacent to land of exactly one component in the
                // overwhelming majority — pick the majority id.
                let ids: Vec<u32> = NEIGH4
                    .iter()
                    .map(|(dx, dy)| {
                        comp[((y as i64 + dy) as u32 * w + (x as i64 + dx) as u32) as usize]
                    })
                    .filter(|&c| c != u32::MAX)
                    .collect();
                if let Some(&c) = ids.first() {
                    comp[(y * w + x) as usize] = c;
                }
            }
        }
    }

    // ---- one site per component ---------------------------------------
    // Recompute components once more cheaply for filled tiles: re-run BFS
    // only to be safe (cheap at 160×160).
    comp = vec![u32::MAX; (w * h) as usize];
    comps.clear();
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            if ctx.map.height_at(x, y) > 0 && comp[i] == u32::MAX {
                let id = comps.len() as u32;
                let mut tiles = Vec::new();
                let mut q = VecDeque::new();
                q.push_back((x, y));
                comp[i] = id;
                while let Some((cx, cy)) = q.pop_front() {
                    tiles.push((cx, cy));
                    for (dx, dy) in NEIGH4 {
                        let nx = cx as i64 + dx;
                        let ny = cy as i64 + dy;
                        if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                            continue;
                        }
                        let j = (ny as u32 * w + nx as u32) as usize;
                        if comp[j] == u32::MAX && ctx.map.height_at(nx as u32, ny as u32) > 0 {
                            comp[j] = id;
                            q.push_back((nx as u32, ny as u32));
                        }
                    }
                }
                comps.push(tiles);
            }
        }
    }
    // Drop specks created by hole-fills merging? Hole-fill never creates new
    // land > 8. Still enforce min size to be safe.
    comps.retain(|t| t.len() >= 8);

    // Which component owns which site?
    let mut site_comp = vec![usize::MAX; ctx.sites.len()];
    for (sid, site) in ctx.sites.iter().enumerate() {
        // Nearest land tile of any component to the site centre.
        let mut best = (usize::MAX, f64::MAX);
        for (ci, tiles) in comps.iter().enumerate() {
            // bbox quick reject
            for &(x, y) in tiles.iter().step_by(tiles.len().div_ceil(64).max(1)) {
                let d = tile_dist(x, y, site.centre.0, site.centre.1);
                if d < best.1 {
                    best = (ci, d);
                }
            }
        }
        if best.0 == usize::MAX {
            return Err(GenError::Retry(format!("island {sid} vanished")));
        }
        site_comp[sid] = best.0;
    }
    // Merges: two sites on one component?
    let mut seen = vec![usize::MAX; comps.len()];
    for (sid, &c) in site_comp.iter().enumerate() {
        if seen[c] != usize::MAX {
            return Err(GenError::Retry(format!(
                "islands merged (sites {} and {} on component {c})",
                seen[c], sid
            )));
        }
        seen[c] = sid;
    }
    // Stray components without a site.
    for (c, s) in seen.iter().enumerate() {
        if *s == usize::MAX && !comps[c].is_empty() {
            return Err(GenError::Retry(format!("stray land component {c}")));
        }
    }

    // ---- island_of + stats + names ------------------------------------
    ctx.map.island_of.fill(NO_ISLAND);
    ctx.map.islands.clear();
    let mut name_rng = ctx.stage_rng(4);
    let mut used_names: Vec<String> = Vec::new();
    // Islands in site order so index 0 = Base Island.
    for (sid, &c) in site_comp.iter().enumerate() {
        let tiles = &comps[c];
        let mut bbox = Rect {
            x0: u32::MAX,
            y0: u32::MAX,
            x1: 0,
            y1: 0,
        };
        let mut peak = 0u8;
        for &(x, y) in tiles {
            bbox.x0 = bbox.x0.min(x);
            bbox.y0 = bbox.y0.min(y);
            bbox.x1 = bbox.x1.max(x);
            bbox.y1 = bbox.y1.max(y);
            peak = peak.max(ctx.map.height_at(x, y));
            *ctx.map.island_of.get_mut(x, y).expect("in bounds") = sid as u8;
        }
        let name = if sid == 0 {
            names::BASE_ISLAND_NAME.to_string()
        } else {
            let mut n = names::make_name(&mut name_rng);
            let mut bump = 2;
            while used_names.contains(&n) {
                n = format!("{} {bump}", names::make_name(&mut name_rng));
                bump += 1;
            }
            n
        };
        used_names.push(name.clone());
        ctx.map.islands.push(Island {
            name,
            archetype: ctx.sites[sid].archetype,
            centre: site_centre(tiles),
            bbox,
            area: tiles.len() as u32,
            peak_level: peak,
        });
    }

    // ---- shore_dist: two-pass style BFS from coast tiles ----------------
    let mut dist = vec![255u8; (w * h) as usize];
    let mut q: VecDeque<(u32, u32)> = VecDeque::new();
    let is_land = |x: u32, y: u32| ctx.map.height_at(x, y) > 0;
    for y in 0..h {
        for x in 0..w {
            let coast = NEIGH4.iter().any(|(dx, dy)| {
                let nx = x as i64 + dx;
                let ny = y as i64 + dy;
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    return true; // world edge counts as sea
                }
                is_land(nx as u32, ny as u32) != is_land(x, y)
            });
            if coast {
                dist[(y * w + x) as usize] = 0;
                q.push_back((x, y));
            }
        }
    }
    while let Some((cx, cy)) = q.pop_front() {
        let d = dist[(cy * w + cx) as usize];
        if d >= 15 {
            continue;
        }
        for (dx, dy) in NEIGH4 {
            let nx = cx as i64 + dx;
            let ny = cy as i64 + dy;
            if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                continue;
            }
            let (nx, ny) = (nx as u32, ny as u32);
            let j = (ny * w + nx) as usize;
            if dist[j] == 255 {
                dist[j] = d + 1;
                q.push_back((nx, ny));
            }
        }
    }
    for y in 0..h {
        for x in 0..w {
            let d = dist[(y * w + x) as usize];
            *ctx.map.shore_dist.get_mut(x, y).expect("in bounds") = d.min(15);
        }
    }
    let _ = &mut name_rng;
    Ok(())
}

fn site_centre(tiles: &[(u32, u32)]) -> (u32, u32) {
    let sx: u64 = tiles.iter().map(|t| t.0 as u64).sum();
    let sy: u64 = tiles.iter().map(|t| t.1 as u64).sum();
    let n = tiles.len().max(1) as u64;
    ((sx / n) as u32, (sy / n) as u32)
}
