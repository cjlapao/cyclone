//! Deterministic island layout and per-round spawns from a seed
//! (pure logic, unit-testable). Types and queries live in `world.rs`.

use crate::iso::View;
use crate::tuning::*;
use crate::world::*;

const GRID: f32 = 4.0; // 4×4 slots for 14 islands
const CELL_TILES: f32 = WORLD_TILES as f32 / GRID;

// ---------------------------------------------------------------- islands
pub fn gen_world(seed: u64) -> World {
    let mut rng = Rng(seed);
    let mut world = World {
        seed,
        h: vec![0; (WORLD_TILES * WORLD_TILES) as usize],
        islands: Vec::new(),
        objects: Vec::new(),
    };

    // All 16 grid cells; the base island gets a centre cell, the other 13
    // slots come from a seeded shuffle of the rest.
    let mut cells: Vec<(u32, u32)> = Vec::new();
    for cy in 0..GRID as u32 {
        for cx in 0..GRID as u32 {
            cells.push((cx, cy));
        }
    }
    rng.shuffle(&mut cells);
    let centre = cells
        .iter()
        .position(|c| (c.0 == 1 || c.0 == 2) && (c.1 == 1 || c.1 == 2))
        .unwrap();
    let mut slot_cells = vec![cells.remove(centre)];
    slot_cells.extend_from_slice(&cells[..NUM_ISLANDS as usize - 1]);

    for (i, (gx, gy)) in slot_cells.iter().enumerate() {
        let is_base = i == 0;
        let jit = if is_base { 0.2 } else { 0.35 };
        let cx = (*gx as f32 + 0.5 + rng.range(-jit, jit)) * CELL_TILES * TILE;
        let cy = (*gy as f32 + 0.5 + rng.range(-jit, jit)) * CELL_TILES * TILE;
        let radius = if is_base {
            rng.range(5.5, 6.8) * TILE
        } else {
            rng.range(4.0, 7.5) * TILE
        };
        let peak = if is_base { 3.2 } else { rng.range(2.6, 6.0) };

        // stamp this island's heights (overlap keeps the higher ground)
        let rt = (radius / TILE).ceil() as i32 + 1;
        let tcx = (cx / TILE) as i32;
        let tcy = (cy / TILE) as i32;
        for dy in -rt..=rt {
            for dx in -rt..=rt {
                let tx = tcx + dx;
                let ty = tcy + dy;
                if tx < 1 || ty < 1 || tx >= WORLD_TILES as i32 - 1 || ty >= WORLD_TILES as i32 - 1
                {
                    continue;
                }
                let wx = (tx as f32 + 0.5) * TILE;
                let wy = (ty as f32 + 0.5) * TILE;
                let d = ((wx - cx).powi(2) + (wy - cy).powi(2)).sqrt();
                let coast = (noise2(tx >> 1, ty >> 1, seed) - 0.5) * 0.30
                    + (noise2(tx, ty, seed) - 0.5) * 0.10;
                let e = 1.0 - d / radius + coast;
                if e > 0.14 {
                    let h = (e * peak).round().clamp(1.0, MAX_LAND_HEIGHT as f32) as u8;
                    let idx = (ty as u32 * WORLD_TILES + tx as u32) as usize;
                    world.h[idx] = world.h[idx].max(h);
                }
            }
        }
        world.islands.push(Island {
            cx,
            cy,
            radius,
            is_base,
            helipad: None,
            tiles: Vec::new(),
            obj_count: 0,
        });
    }

    // Terrain smoothing: plateaus need to exist for flat landings, and
    // neighbours may never differ by more than 1 (gentle slopes, cliffs are
    // drawn at height >= 5 against lower neighbours only where erosion left them).
    const NB8: [(i32, i32); 8] = [
        (-1, -1),
        (-1, 0),
        (-1, 1),
        (0, -1),
        (0, 1),
        (1, -1),
        (1, 0),
        (1, 1),
    ];
    for _ in 0..9 {
        let src = world.h.clone();
        for ty in 1..WORLD_TILES - 1 {
            for tx in 1..WORLD_TILES - 1 {
                let i = (ty * WORLD_TILES + tx) as usize;
                if src[i] == 0 {
                    continue;
                }
                let mut sum = src[i] as u32;
                let mut n = 1u32;
                for (dx, dy) in NB8 {
                    let j =
                        ((ty as i32 + dy) as u32 * WORLD_TILES + (tx as i32 + dx) as u32) as usize;
                    if src[j] > 0 {
                        sum += src[j] as u32;
                        n += 1;
                    }
                }
                world.h[i] = ((sum as f32 / n as f32).round().max(1.0)) as u8;
            }
        }
    }
    // slope limit: one step max between touching land tiles
    for _ in 0..10 {
        let mut changed = false;
        for ty in 0..WORLD_TILES {
            for tx in 0..WORLD_TILES {
                let i = (ty * WORLD_TILES + tx) as usize;
                let h = world.h[i];
                if h == 0 {
                    continue;
                }
                for (dx, dy) in [(1i32, 0i32), (0, 1)] {
                    let (nx, ny) = (tx as i32 + dx, ty as i32 + dy);
                    if nx >= WORLD_TILES as i32 || ny >= WORLD_TILES as i32 {
                        continue;
                    }
                    let j = (ny as u32 * WORLD_TILES + nx as u32) as usize;
                    if world.h[j] > 0 && world.h[j] as i32 - h as i32 > 1 {
                        world.h[i] = world.h[j] - 1;
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    // attribute each land tile to the nearest island centre
    for t in 0..(WORLD_TILES * WORLD_TILES) {
        if world.h[t as usize] == 0 {
            continue;
        }
        let wx = (t % WORLD_TILES) as f32 * TILE + TILE * 0.5;
        let wy = (t / WORLD_TILES) as f32 * TILE + TILE * 0.5;
        let mut best = 0usize;
        let mut bd = f32::MAX;
        for (i, isl) in world.islands.iter().enumerate() {
            let d = (wx - isl.cx).powi(2) + (wy - isl.cy).powi(2);
            if d < bd {
                bd = d;
                best = i;
            }
        }
        world.islands[best]
            .tiles
            .push((t % WORLD_TILES, t / WORLD_TILES));
    }

    // --- buildings & trees --------------------------------------------------
    for i in 0..world.islands.len() {
        let mut rng = Rng(seed ^ (i as u64).wrapping_mul(0x51A7F3DB));
        let tiles: Vec<(u32, u32)> = world.islands[i].tiles.clone();
        if tiles.is_empty() {
            continue;
        }
        let n_build = if world.islands[i].is_base {
            2
        } else {
            1 + rng.below(3)
        };
        let n_tree = 3 + rng.below(6);
        let mut placed: Vec<(f32, f32)> = Vec::new();

        // Base Island: claim a flat 3×3 for the home helipad first.
        if world.islands[i].is_base {
            let mut rngp = Rng(seed ^ (i as u64).wrapping_mul(0x2545F491));
            for _ in 0..400 {
                let (tx, ty) = tiles[rngp.below(tiles.len())];
                let h = world.tile_h(tx, ty);
                if h >= 1 && flat_3x3(&world, tx, ty, h) {
                    world.islands[i].helipad = Some((tx, ty));
                    placed.push(((tx as f32 + 0.5) * TILE, (ty as f32 + 0.5) * TILE));
                    break;
                }
            }
        }

        let try_place = |world: &World,
                         tiles: &[(u32, u32)],
                         rng: &mut Rng,
                         placed: &[(f32, f32)],
                         max_h: u8,
                         gap: f32|
         -> Option<(u32, u32)> {
            for _ in 0..60 {
                let (tx, ty) = tiles[rng.below(tiles.len())];
                let h = world.tile_h(tx, ty);
                if h < 1 || h > max_h || !flat_3x3(world, tx, ty, h) {
                    continue;
                }
                let wx = (tx as f32 + 0.5) * TILE;
                let wy = (ty as f32 + 0.5) * TILE;
                if placed
                    .iter()
                    .any(|(px, py)| (px - wx).abs() + (py - wy).abs() < gap * TILE)
                {
                    continue;
                }
                return Some((tx, ty));
            }
            None
        };

        for b in 0..n_build {
            let kind = if world.islands[i].is_base && b == 1 {
                ObjKind::Tower
            } else if rng.f32() < 0.7 {
                ObjKind::Hut
            } else {
                ObjKind::Tower
            };
            if let Some((tx, ty)) = try_place(&world, &tiles, &mut rng, &placed, 4, 2.2) {
                let wx = (tx as f32 + 0.5) * TILE;
                let wy = (ty as f32 + 0.5) * TILE;
                placed.push((wx, wy));
                world.objects.push(Obj {
                    x: wx,
                    y: wy,
                    base_h: world.tile_h(tx, ty),
                    kind,
                });
            }
        }

        // lighthouse near the rim — a tall occluder
        let rim: Vec<(u32, u32)> = world.islands[i]
            .tiles
            .iter()
            .copied()
            .filter(|(tx, ty)| {
                let wx = (*tx as f32 + 0.5) * TILE;
                let wy = (*ty as f32 + 0.5) * TILE;
                let d = ((wx - world.islands[i].cx).powi(2) + (wy - world.islands[i].cy).powi(2))
                    .sqrt();
                d > world.islands[i].radius * 0.55 && d < world.islands[i].radius * 0.95
            })
            .collect();
        if !rim.is_empty() && rng.f32() < 0.75 {
            for _ in 0..30 {
                let (tx, ty) = rim[rng.below(rim.len())];
                let wx = (tx as f32 + 0.5) * TILE;
                let wy = (ty as f32 + 0.5) * TILE;
                if !placed
                    .iter()
                    .any(|(px, py)| (px - wx).abs() + (py - wy).abs() < 3.0 * TILE)
                {
                    placed.push((wx, wy));
                    world.objects.push(Obj {
                        x: wx,
                        y: wy,
                        base_h: world.tile_h(tx, ty),
                        kind: ObjKind::Lighthouse,
                    });
                    break;
                }
            }
        }

        for _ in 0..n_tree {
            if let Some((tx, ty)) = try_place(&world, &tiles, &mut rng, &placed, 5, 2.0) {
                let wx = (tx as f32 + 0.5) * TILE;
                let wy = (ty as f32 + 0.5) * TILE;
                placed.push((wx, wy));
                world.objects.push(Obj {
                    x: wx,
                    y: wy,
                    base_h: world.tile_h(tx, ty),
                    kind: ObjKind::Tree,
                });
            }
        }
    }

    // count objects per island (nearest centre within radius*1.25)
    for o in &world.objects {
        let mut best = 0usize;
        let mut bd = f32::MAX;
        for (i, isl) in world.islands.iter().enumerate() {
            let d = ((o.x - isl.cx).powi(2) + (o.y - isl.cy).powi(2)).sqrt();
            if d < bd {
                bd = d;
                best = i;
            }
        }
        if bd <= world.islands[best].radius * 1.25 {
            world.islands[best].obj_count += 1;
        }
    }

    // 3 extra helipads on random non-base islands with room
    let mut extra: Vec<usize> = (1..world.islands.len()).collect();
    Rng(seed ^ 0xABCD).shuffle(&mut extra);
    let mut added = 0;
    for &idx in extra.iter() {
        if added >= 3 {
            break;
        }
        if world.islands[idx].helipad.is_some() || world.islands[idx].tiles.len() < 25 {
            continue;
        }
        let mut rng2 = Rng(seed ^ (idx as u64).wrapping_mul(0x77F1));
        let pad_tiles = world.islands[idx].tiles.clone();
        for _ in 0..80 {
            let (tx, ty) = pad_tiles[rng2.below(pad_tiles.len())];
            let x = (tx as f32 + 0.5) * TILE;
            let y = (ty as f32 + 0.5) * TILE;
            if world.landing_ok(x, y) {
                world.islands[idx].helipad = Some((tx, ty));
                added += 1;
                break;
            }
        }
    }

    world
}

// ---------------------------------------------------------------- spawns
/// Place crates and refugees for a round. `round` bumps the hidden count.
pub fn place_round(world: &World, seed: u64, round: u32) -> (Vec<Crate>, Vec<Refugee>) {
    let mut rng = Rng(seed ^ (round as u64).wrapping_mul(0x9E3779B1));

    let mut isl: Vec<usize> = (0..world.islands.len())
        .filter(|&i| !world.islands[i].is_base && world.islands[i].tiles.len() >= 12)
        .collect();
    rng.shuffle(&mut isl);
    // object islands first — hidden crates need occluders to stand behind
    isl.sort_by_key(|&i| if world.islands[i].obj_count > 0 { 0 } else { 1 });

    let need_hidden = (MIN_HIDDEN_CRATES + round / 2).min(4) as usize;
    let mut crates = Vec::new();

    for (ci, &is) in isl.iter().take(NUM_CRATES as usize).enumerate() {
        let mut tiles: Vec<(u32, u32)> = world.islands[is].tiles.clone();
        rng.shuffle(&mut tiles);
        let mut spot = None;
        if ci < need_hidden {
            for &(tx, ty) in &tiles {
                let x = (tx as f32 + 0.5) * TILE;
                let y = (ty as f32 + 0.5) * TILE;
                let h = world.tile_h(tx, ty);
                if h == 0 || near_object(world, x, y, 1.0) {
                    continue;
                }
                if blocking_obj(world, x, y, h, View::North).is_some()
                    && blocking_obj(world, x, y, h, View::South).is_none()
                {
                    spot = Some((x, y, Some(View::North)));
                    break;
                }
                if blocking_obj(world, x, y, h, View::South).is_some()
                    && blocking_obj(world, x, y, h, View::North).is_none()
                {
                    spot = Some((x, y, Some(View::South)));
                    break;
                }
            }
        }
        if spot.is_none() {
            for &(tx, ty) in &tiles {
                let x = (tx as f32 + 0.5) * TILE;
                let y = (ty as f32 + 0.5) * TILE;
                if world.tile_h(tx, ty) == 0 || near_object(world, x, y, 1.0) {
                    continue;
                }
                spot = Some((x, y, None));
                break;
            }
        }
        if let Some((x, y, hidden)) = spot.or(tiles
            .first()
            .map(|&(tx, ty)| ((tx as f32 + 0.5) * TILE, (ty as f32 + 0.5) * TILE, None)))
        {
            crates.push(Crate {
                x,
                y,
                island: is,
                hidden_by: hidden,
            });
        }
    }

    let mut rng2 = Rng(seed ^ (round as u64).wrapping_mul(0xC2B2AE3D));
    let mut isl2: Vec<usize> = (0..world.islands.len())
        .filter(|&i| !world.islands[i].is_base && world.islands[i].tiles.len() >= 12)
        .collect();
    rng2.shuffle(&mut isl2);
    let mut refs = Vec::new();
    for &is in isl2.iter().take(9) {
        let n = 1 + rng2.below(3);
        for _ in 0..n {
            let mut tiles: Vec<(u32, u32)> = world.islands[is].tiles.clone();
            rng2.shuffle(&mut tiles);
            for &(tx, ty) in tiles.iter().take(14) {
                let x = (tx as f32 + 0.5) * TILE;
                let y = (ty as f32 + 0.5) * TILE;
                if world.tile_h(tx, ty) == 0 || near_object(world, x, y, 0.8) {
                    continue;
                }
                refs.push(Refugee { x, y });
                break;
            }
        }
    }
    (crates, refs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_same_seed() {
        let a = gen_world(42);
        let b = gen_world(42);
        assert_eq!(a.h, b.h);
        assert_eq!(a.objects.len(), b.objects.len());
        for (x, y) in a.objects.iter().zip(b.objects.iter()) {
            assert_eq!(x.x.to_bits(), y.x.to_bits());
            assert_eq!(x.kind, y.kind);
        }
        let (ca, ra) = place_round(&a, 42, 1);
        let (cb, rb) = place_round(&b, 42, 1);
        assert_eq!(ca.len(), cb.len());
        assert_eq!(ra.len(), rb.len());
        for (x, y) in ca.iter().zip(cb.iter()) {
            assert_eq!(x.x.to_bits(), y.x.to_bits());
            assert_eq!(x.hidden_by, y.hidden_by);
        }
    }

    #[test]
    fn different_seeds_differ() {
        assert_ne!(gen_world(1).h, gen_world(2).h);
    }

    #[test]
    fn world_shape() {
        let w = gen_world(1984);
        assert_eq!(w.islands.len() as u32, NUM_ISLANDS);
        assert!(w.islands[0].is_base);
        let c = WORLD_UNITS * 0.5;
        assert!(
            ((w.islands[0].cx - c).powi(2) + (w.islands[0].cy - c).powi(2)).sqrt()
                < CELL_TILES * TILE,
            "base island near centre"
        );
        assert!(w.h.iter().all(|&h| h <= MAX_LAND_HEIGHT));
        let land: usize = w.h.iter().filter(|&&h| h > 0).count();
        assert!(land > 800, "plenty of land, got {land}");
        assert!(w.islands.iter().filter(|i| i.helipad.is_some()).count() >= 4);
    }

    #[test]
    fn all_islands_have_land() {
        let w = gen_world(7);
        for (i, isl) in w.islands.iter().enumerate() {
            assert!(!isl.tiles.is_empty(), "island {i} has no land");
        }
    }
}
