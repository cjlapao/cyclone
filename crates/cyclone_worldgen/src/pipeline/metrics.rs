//! Stage 10 — Metrics, difficulty, and the A5.10 invariants.

use crate::config::WorldGenConfig;
use crate::error::GenError;
use crate::map::{Difficulty, PropKind, Surface, WorldMap};
use crate::pipeline::GenCtx;
use crate::pipeline::common::tile_dist;

/// Euclidean MST bottleneck (longest edge of the MST) over helipads.
pub fn longest_mst_hop(map: &WorldMap) -> f64 {
    let pads = &map.helipads;
    if pads.len() < 2 {
        return 0.0;
    }
    let mut edges: Vec<(f64, usize, usize)> = Vec::new();
    for i in 0..pads.len() {
        for j in i + 1..pads.len() {
            edges.push((
                tile_dist(
                    pads[i].tile.0,
                    pads[i].tile.1,
                    pads[j].tile.0,
                    pads[j].tile.1,
                ),
                i,
                j,
            ));
        }
    }
    edges.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    let mut parent: Vec<usize> = (0..pads.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    let mut longest = 0.0f64;
    let mut used = 0;
    for (w, i, j) in edges {
        let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
        if ri != rj {
            parent[ri] = rj;
            longest = longest.max(w);
            used += 1;
            if used == pads.len() - 1 {
                break;
            }
        }
    }
    longest
}

/// Nearest-neighbour tour Base → crates → Base (tiles).
pub fn nn_route_len(map: &WorldMap) -> f64 {
    if map.crates.is_empty() {
        return 0.0;
    }
    let start = map
        .helipads
        .iter()
        .find(|p| p.island == 0)
        .map(|p| p.tile)
        .unwrap_or(map.islands[0].centre);
    let mut cur = start;
    let mut left: Vec<(u32, u32)> = map.crates.iter().map(|c| c.tile).collect();
    let mut total = 0.0f64;
    while !left.is_empty() {
        let mut bi = 0;
        let mut bd = f64::MAX;
        for (i, &(x, y)) in left.iter().enumerate() {
            let d = tile_dist(cur.0, cur.1, x, y);
            if d < bd {
                bd = d;
                bi = i;
            }
        }
        total += bd;
        cur = left.swap_remove(bi);
    }
    total += tile_dist(cur.0, cur.1, start.0, start.1);
    total
}

/// Check the A5.10 invariants. Empty vec = healthy world.
pub fn validate(map: &WorldMap, cfg: &WorldGenConfig) -> Vec<String> {
    let mut issues = Vec::new();
    let (w, h) = map.size;

    if map.islands.len() != cfg.island_count as usize {
        issues.push(format!(
            "island count {} != {}",
            map.islands.len(),
            cfg.island_count
        ));
    }

    // Sea-only margin band.
    'margin: for y in 0..h {
        for x in 0..w {
            if (x < cfg.margin || y < cfg.margin || x + cfg.margin >= w || y + cfg.margin >= h)
                && map.height_at(x, y) > 0
            {
                issues.push(format!("land at ({x},{y}) inside the sea margin"));
                break 'margin;
            }
        }
    }

    // Base Island helipad + hangar.
    if !map.helipads.iter().any(|p| p.island == 0) {
        issues.push("Base Island has no helipad".into());
    }
    if let Some(b) = map.islands.first().map(|i| i.bbox)
        && !map
            .props
            .iter()
            .any(|p| p.kind == PropKind::Hangar && b.contains(p.tile.0, p.tile.1))
    {
        issues.push("Base Island has no hangar".into());
    }

    // Fuel graph connectivity (0.8 limit) — Kruskal edge count check.
    let limit = cfg.fuel_range as f64 * 0.8;
    let mut parent: Vec<usize> = (0..map.helipads.len()).collect();
    fn find(p: &mut [usize], mut x: usize) -> usize {
        while p[x] != x {
            p[x] = p[p[x]];
            x = p[x];
        }
        x
    }
    for i in 0..map.helipads.len() {
        for j in i + 1..map.helipads.len() {
            if tile_dist(
                map.helipads[i].tile.0,
                map.helipads[i].tile.1,
                map.helipads[j].tile.0,
                map.helipads[j].tile.1,
            ) <= limit
            {
                let (ri, rj) = (find(&mut parent, i), find(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    if !map.helipads.is_empty() {
        let roots = (0..map.helipads.len())
            .map(|i| find(&mut parent, i))
            .collect::<Vec<_>>();
        if roots.windows(2).any(|win| win[0] != win[1]) {
            issues.push("fuel graph disconnected".into());
        }
    }
    // Island reach 0.4.
    let reach = cfg.fuel_range as f64 * 0.4;
    for (idx, isl) in map.islands.iter().enumerate() {
        let mut best = f64::INFINITY;
        for y in isl.bbox.y0..=isl.bbox.y1 {
            for x in isl.bbox.x0..=isl.bbox.x1 {
                if map.island_of.raw(x, y) as usize == idx {
                    best = best.min(
                        map.helipads
                            .iter()
                            .map(|p| tile_dist(x, y, p.tile.0, p.tile.1))
                            .fold(f64::INFINITY, f64::min),
                    );
                }
            }
        }
        if best > reach {
            issues.push(format!("island {} out of fuel reach", isl.name));
        }
    }

    // Crates: right count, distinct non-base islands, walkable tiles.
    if map.crates.len() != cfg.crates as usize {
        issues.push(format!("{} crates != {}", map.crates.len(), cfg.crates));
    }
    let mut crate_islands: Vec<u8> = map.crates.iter().map(|c| c.island).collect();
    crate_islands.sort_unstable();
    if crate_islands.windows(2).any(|w2| w2[0] == w2[1]) {
        issues.push("two crates share an island".into());
    }
    if crate_islands.first().copied().unwrap_or(1) == 0 {
        issues.push("a crate sits on Base Island".into());
    }
    let mut occupied_by = vec![0u8; (w * h) as usize];
    for p in &map.props {
        for j in 0..p.footprint.1 as u32 {
            for i in 0..p.footprint.0 as u32 {
                let x = p.tile.0 + i;
                let y = p.tile.1 + j;
                if x < w && y < h {
                    occupied_by[(y * w + x) as usize] += 1;
                }
            }
        }
    }
    for hp in &map.helipads {
        for j in 0..3u32 {
            for i in 0..3u32 {
                let x = hp.tile.0 - 1 + i;
                let y = hp.tile.1 - 1 + j;
                if x < w && y < h {
                    occupied_by[(y * w + x) as usize] += 1;
                }
            }
        }
    }
    for p in &map.props {
        for j in 0..p.footprint.1 as u32 {
            for i in 0..p.footprint.0 as u32 {
                let x = p.tile.0 + i;
                let y = p.tile.1 + j;
                if x < w && y < h && occupied_by[(y * w + x) as usize] > 1 {
                    issues.push(format!("overlapping footprints at ({x},{y})"));
                }
            }
        }
    }
    for c in &map.crates {
        let (x, y) = c.tile;
        if map.height_at(x, y) < 1 || map.cliff.raw(x, y) != 0 {
            issues.push(format!("crate at ({x},{y}) on invalid ground"));
        }
        if matches!(
            map.surface.raw(x, y),
            Surface::Sea | Surface::Shallow | Surface::Sand
        ) && map.surface.raw(x, y) == Surface::Sand
            && map.height_at(x, y) == 0
        {
            issues.push(format!("crate at ({x},{y}) in water"));
        }
        if occupied_by[(y * w + x) as usize] > 0 {
            issues.push(format!("crate at ({x},{y}) overlaps a prop/pad"));
        }
        if c.hidden_from.iter().all(|b| *b) {
            issues.push(format!("crate at ({x},{y}) hidden from all four headings"));
        }
    }

    // At least hidden_min crates hidden from 1–3 headings.
    let hidden = map
        .crates
        .iter()
        .filter(|c| c.hidden_from.iter().any(|b| *b))
        .count();
    let want = cfg.hidden_crates_min_for_round(map.round) as usize;
    if hidden < want {
        issues.push(format!("{hidden} hidden crates < {want}"));
    }

    // Refugees/crates not on occupied tiles and distinct tiles.
    let mut occ2 = occupied_by.clone();
    for c in &map.crates {
        occ2[(c.tile.1 * w + c.tile.0) as usize] += 1;
    }
    for r in &map.refugees {
        let i = (r.tile.1 * w + r.tile.0) as usize;
        if occ2[i] > 0 {
            issues.push(format!(
                "refugee at ({}, {}) overlaps something",
                r.tile.0, r.tile.1
            ));
        }
        occ2[i] += 1;
    }

    // Props off cliff tiles.
    for p in &map.props {
        if p.kind != PropKind::Jetty && map.cliff.raw(p.tile.0, p.tile.1) != 0 {
            issues.push(format!("prop {:?} on a cliff tile", p.kind));
        }
    }

    // Spec 02: each island is exactly one 4-connected component, and
    // coastlines of different islands keep cfg.min_gap tiles of water
    // between them (Euclidean on land cells: Chebyshev gaps used by the
    // placer are a lower bound for Euclidean, so this is not stricter
    // than generation, just measurable).
    let mut cells: Vec<Vec<(u32, u32)>> = vec![Vec::new(); map.islands.len()];
    for y in 0..h {
        for x in 0..w {
            let i = map.island_of.raw(x, y);
            if i != u8::MAX && (i as usize) < cells.len() {
                cells[i as usize].push((x, y));
            }
        }
    }
    for (i, isl) in map.islands.iter().enumerate() {
        let list = &cells[i];
        if list.len() as u32 != isl.area {
            issues.push(format!(
                "island {} area {} != {} island_of tiles",
                isl.name,
                isl.area,
                list.len()
            ));
        }
        let mut seen = vec![false; (w * h) as usize];
        let mut stack = vec![list[0]];
        seen[list[0].1 as usize * w as usize + list[0].0 as usize] = true;
        let mut reached = 1u32;
        while let Some((x, y)) = stack.pop() {
            for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
                let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                    continue;
                }
                let (nx, ny) = (nx as u32, ny as u32);
                let k = ny as usize * w as usize + nx as usize;
                if !seen[k] && map.island_of.raw(nx, ny) == i as u8 {
                    seen[k] = true;
                    reached += 1;
                    stack.push((nx, ny));
                }
            }
        }
        if reached as usize != list.len() {
            issues.push(format!(
                "island {} is not one component ({}/{} tiles reached)",
                isl.name,
                reached,
                list.len()
            ));
        }
    }
    let gap2 = (cfg.min_gap * cfg.min_gap).max(1);
    for i in 0..map.islands.len() {
        for j in i + 1..map.islands.len() {
            let (a, b) = (&map.islands[i].bbox, &map.islands[j].bbox);
            let near = a.x0.saturating_sub(cfg.min_gap) <= b.x1
                && b.x0.saturating_sub(cfg.min_gap) <= a.x1
                && a.y0.saturating_sub(cfg.min_gap) <= b.y1
                && b.y0.saturating_sub(cfg.min_gap) <= a.y1;
            if !near {
                continue;
            }
            for &(ax, ay) in &cells[i] {
                for &(bx, by) in &cells[j] {
                    let dx = ax.abs_diff(bx);
                    let dy = ay.abs_diff(by);
                    if dx * dx + dy * dy < gap2 {
                        issues.push(format!(
                            "islands {} & {} closer than min_gap {}",
                            map.islands[i].name, map.islands[j].name, cfg.min_gap
                        ));
                        break;
                    }
                }
            }
        }
    }

    issues
}

pub fn stage(ctx: &mut GenCtx) -> Result<(), GenError> {
    let map = &mut ctx.map;
    map.metrics.route_len = nn_route_len(map);
    map.metrics.longest_hop = longest_mst_hop(map);
    map.metrics.hidden_crates = map
        .crates
        .iter()
        .filter(|c| c.hidden_from.iter().any(|b| *b))
        .count() as u32;
    let land = map.height.iter().filter(|&&h| h > 0).count();
    map.metrics.land_ratio = land as f64 / map.len_tiles() as f64;
    // Area-weighted per-island band spread: one global histogram would
    // call a map flat just for containing many SMALL islands (a 30-tile
    // islet can't fill 5 bands — geometry, not flatness). Flatness is a
    // property of each island, weighted by how much land it is, so a
    // big mesa dominates and islets barely count.
    let mut num = 0.0f64;
    let mut den = 0.0f64;
    for isl in &map.islands {
        let mut bands = [0u32; 256];
        let mut n = 0u32;
        for y in isl.bbox.y0..=isl.bbox.y1 {
            for x in isl.bbox.x0..=isl.bbox.x1 {
                let hh = map.height.get(x, y).copied().unwrap_or(0);
                if hh > 0 {
                    bands[hh as usize] += 1;
                    n += 1;
                }
            }
        }
        // Under ~40 tiles the band count is geometry-limited; such
        // islets' band-share verdict still counts, weighted by their
        // (small) area — a big mesa dominates, islets barely accuse.
        if n > 0 {
            let top = bands.iter().max().copied().unwrap_or(0) as f64;
            num += n as f64 * (1.0 - top / n as f64);
            den += n as f64;
        }
    }
    map.metrics.terracing = if den > 0.0 { num / den } else { 1.0 };

    let fuel = ctx.cfg.fuel_range as f64;
    let per_crate = map.metrics.route_len / map.crates.len().max(1) as f64;
    let s_hop = map.metrics.longest_hop / fuel;
    let s_route = per_crate / fuel;
    // Weighted score over the fuel-relative metrics (spec: "simple weighted
    // score"). Bands calibrated against a 1000-seed batch sweep (median
    // score 0.76): ~25% Easy, 40% Normal, 35% Hard — the old ladder put
    // every world in Hard, so difficulty carried no signal.
    let score = 0.55 * s_hop + 0.45 * s_route;
    let (difficulty, reason) = if score < 0.723 {
        (
            Difficulty::Easy,
            "Short hops with plenty of fuel to spare".to_string(),
        )
    } else if score < 0.791 {
        (
            Difficulty::Normal,
            "Balanced distances between fuel stops".to_string(),
        )
    } else if s_hop >= s_route {
        (Difficulty::Hard, "Long hops between fuel stops".to_string())
    } else {
        (
            Difficulty::Hard,
            "Long rescue routes across the sea".to_string(),
        )
    };
    map.metrics.difficulty = difficulty;
    map.metrics.difficulty_reason = reason;

    let issues = validate(map, &ctx.cfg);
    if !issues.is_empty() {
        return Err(GenError::Retry(format!(
            "validation: {}",
            issues.join("; ")
        )));
    }
    Ok(())
}

// Small helper kept private to the crate: WorldMap has no len() itself.
trait LenTiles {
    fn len_tiles(&self) -> usize;
}
impl LenTiles for WorldMap {
    fn len_tiles(&self) -> usize {
        (self.size.0 * self.size.1) as usize
    }
}
