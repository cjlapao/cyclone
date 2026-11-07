//! Continent surface mesh: the heightfield's TOP LAYER, chunked.
//!
//! The voxel-Earth pivot (round 154) is a heightfield, so "render the
//! top layer following the terrain" is native: one top quad per mesh
//! cell, plus skirt walls toward lower neighbours (sealed-by-construction
//! again — the sliver-lesson from terrain.rs applies at 1 km scale).
//! Mesh cells are 8 voxels (2 m) at 25 cm voxel data: terraces at full
//! voxel resolution would be 16 M cells the GPU never resolves at the
//! 3000-unit far plane; 2 m steps keep every ridge line readable while
//! a future near-field clipmap can spend the 25 cm detail where the
//! camera can actually see it.

use bevy::prelude::*;
use cyclone_worldgen::{Continent, ContinentConfig, elevation_m, generate_continent};

use crate::QuadBuilder;
use crate::terrain::tile_tint;

/// Voxels per mesh cell edge. 8 × 25 cm = 2 m.
pub const CELL_VOX: u32 = 8;
/// Mesh cells per chunk edge (64 × 2 m = 128 m chunks).
const CELLS_PER_CHUNK: u32 = 64;
/// Sea plane sits at 0; land skirts run to this depth (m).
const SEA_FLOOR_M: f32 = -6.0;

/// Elevation ramp: the satellite palette (sand → green → ochre → rock →
/// snow) as linear-space vertex colours.
fn earth_rgb(h_m: f32, tint: f32) -> Vec4 {
    let s = |v: f32| {
        let x = (v * tint).clamp(0.0, 1.0);
        // sRGB → linear (same curve as crate::srgb_to_vec4, inlined per
        // channel to avoid a Vec4 round-trip per vertex).
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    let (r, g, b) = if h_m < 2.0 {
        return Vec4::splat(0.0); // unused: ocean columns are skipped
    } else if h_m < 14.0 {
        // Breakpoints ride the CURRENT relief band (300 m ceiling,
        // round-176 rebalance): the old 260/600/900 cuts lived above a
        // world that no longer exists, so every slope read as one flat
        // ochre slab. Bands from the census: flat ~<50, high 120+,
        // mountain 210+.
        (0.855f32, 0.769, 0.545) // sand
    } else if h_m < 95.0 {
        let t = (h_m - 14.0) / 81.0;
        (0.30 - 0.10 * t, 0.62 - 0.20 * t, 0.26) // green deepening
    } else if h_m < 175.0 {
        (0.55, 0.47, 0.28) // hills ochre
    } else if h_m < 245.0 {
        (0.46, 0.45, 0.46) // rock
    } else {
        (0.92, 0.93, 0.94) // snow
    };
    Vec4::new(s(r), s(g), s(b), 1.0)
}

/// River water: the sea's own hue, linearised the same way —
/// one ocean palette, one word for "water" in every shader path.
fn water_rgb() -> Vec4 {
    let s = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    Vec4::new(s(0.05), s(0.28), s(0.47), 1.0)
}

/// Wall colour: palette-darkened rock/soil of the column it clads.
fn wall_rgb(h_m: f32, tint: f32) -> Vec4 {
    let c = earth_rgb(h_m.max(1.0), tint);
    Vec4::new(c.x * 0.62, c.y * 0.62, c.z * 0.62, 1.0)
}

/// One chunk: local vertices, world anchor at the chunk corner.
pub struct ContinentChunk {
    pub anchor: Vec3,
    pub builder: QuadBuilder,
}

/// Build every chunk of the surface. Pure function of the Continent —
/// no world, no assets, headless-testable, deterministic by input.
pub fn build_continent_chunks(c: &Continent) -> Vec<ContinentChunk> {
    let cells = c.config.size / CELL_VOX;
    let vm = c.config.voxel as f64;
    let cell_m = (CELL_VOX as f64 * vm) as f32;
    let h_at = |cx: u32, cy: u32| -> f32 {
        (c.heights[(cy * c.config.size + cx) as usize] as f64 * vm) as f32
    };
    // River water surface at a column (0 = dry). The surface, not
    // the bed, is what an eye sees from above — "seen height".
    let w_at = |cx: u32, cy: u32| -> f32 {
        (c.water[(cy * c.config.size + cx) as usize] as f64 * vm) as f32
    };
    let seen_at = |cx: u32, cy: u32| -> f32 { h_at(cx, cy).max(w_at(cx, cy)) };
    let mut chunks: Vec<ContinentChunk> = Vec::new();
    let mut cy0 = 0u32;
    while cy0 < cells {
        let cy1 = (cy0 + CELLS_PER_CHUNK).min(cells);
        let mut cx0 = 0u32;
        while cx0 < cells {
            let cx1 = (cx0 + CELLS_PER_CHUNK).min(cells);
            let mut b = QuadBuilder::new();
            for cy in cy0..cy1 {
                for cx in cx0..cx1 {
                    let h = h_at(cx * CELL_VOX, cy * CELL_VOX);
                    let w = w_at(cx * CELL_VOX, cy * CELL_VOX);
                    if h <= 0.0 && w <= 0.0 {
                        continue; // ocean: the sea plane covers it
                    }
                    // What an eye sees: water over bed; the top face
                    // and every wall from here speak in seen heights.
                    let hs = h.max(w);
                    let is_wet = w > 0.0;
                    let lx = (cx - cx0) as f32 * cell_m;
                    let lz = (cy - cy0) as f32 * cell_m;
                    let hx1 = lx + cell_m;
                    let hz1 = lz + cell_m;
                    let tint = tile_tint(cx, cy, c.seed);
                    let top = if is_wet {
                        water_rgb()
                    } else {
                        earth_rgb(h, tint)
                    };
                    b.quad(
                        [
                            Vec3::new(lx, hs, lz),
                            Vec3::new(lx, hs, hz1),
                            Vec3::new(hx1, hs, hz1),
                            Vec3::new(hx1, hs, lz),
                        ],
                        Vec3::Y,
                        top,
                    );
                    // Walls toward lower (or ocean/edge) neighbours, in
                    // cell grid space; +Z rows, +X cols as terrain.rs.
                    let nb = |gx: u32, gy: u32| -> f32 {
                        if gx >= cells || gy >= cells {
                            return -1.0; // world edge = drop to floor
                        }
                        seen_at(gx * CELL_VOX, gy * CELL_VOX).max(-1.0)
                    };
                    let wall = if is_wet {
                        water_rgb()
                    } else {
                        wall_rgb(h, tint)
                    };
                    for (dx, dz, ring, n) in [
                        (0i32, -1, 0u8, Vec3::new(0.0, 0.0, -1.0)),
                        (1, 0, 1, Vec3::X),
                        (0, 1, 2, Vec3::new(0.0, 0.0, 1.0)),
                        (-1, 0, 3, Vec3::NEG_X),
                    ] {
                        let gx = cx as i32 + dx;
                        let gy = cy as i32 + dz;
                        if gx < 0 || gy < 0 {
                            continue; // world edge on the minus side
                        }
                        let nh = nb(gx as u32, gy as u32);
                        if nh >= hs {
                            continue;
                        }
                        let bottom = (nh.max(SEA_FLOOR_M)).min(hs);
                        let (x0, x1, z0, z1) = (lx, hx1, lz, hz1);
                        let r = match ring {
                            0 => [
                                Vec3::new(x0, bottom, z0),
                                Vec3::new(x0, hs, z0),
                                Vec3::new(x1, hs, z0),
                                Vec3::new(x1, bottom, z0),
                            ],
                            1 => [
                                Vec3::new(x1, bottom, z0),
                                Vec3::new(x1, hs, z0),
                                Vec3::new(x1, hs, z1),
                                Vec3::new(x1, bottom, z1),
                            ],
                            2 => [
                                Vec3::new(x1, bottom, z1),
                                Vec3::new(x1, hs, z1),
                                Vec3::new(x0, hs, z1),
                                Vec3::new(x0, bottom, z1),
                            ],
                            _ => [
                                Vec3::new(x0, bottom, z1),
                                Vec3::new(x0, hs, z1),
                                Vec3::new(x0, hs, z0),
                                Vec3::new(x0, bottom, z0),
                            ],
                        };
                        b.quad(r, n, wall);
                    }
                }
            }
            if !b.is_empty() {
                chunks.push(ContinentChunk {
                    anchor: Vec3::new(cx0 as f32 * cell_m, 0.0, cy0 as f32 * cell_m),
                    builder: b,
                });
            }
            cx0 = cx1;
        }
        cy0 = cy1;
    }
    chunks
}

/// Convenience for callers without the Continent in hand: generate +
/// mesh. (Dev mode uses this once per seed; is_changed gating upstream
/// means rerolls pay it, menu stays free.)
pub fn generate_and_mesh(seed: u64, size: u32) -> (Continent, Vec<ContinentChunk>) {
    let cfg = ContinentConfig {
        size,
        ..Default::default()
    };
    let c = generate_continent(seed, &cfg).expect("size within contract; dev preview dial");
    let chunks = build_continent_chunks(&c);
    (c, chunks)
}

/// The pure elevation sampler (re-exported for rig placement: the game
/// asks "how high is the ground at spawn" without the whole heightfield).
pub fn surface_height_m(x_m: f64, y_m: f64, seed: u64, size: u32) -> f64 {
    let cfg = ContinentConfig {
        size,
        ..Default::default()
    };
    // The spawn sampler must see the RIVER, not just the mountain:
    // a rig cleared for dry ground still drowns if a channel runs
    // through it. Carves are local (depth caps at half the standing
    // height), so a coarse lower-bound window around the point —
    // min over a grid inside the deepest possible basin — gives a
    // height that is never above truth. Spawn clears water or dies
    // trying; it never spawns under it.
    let raw = elevation_m(x_m, y_m, seed, &cfg);
    let mut lo = raw;
    // Reach: a river's full footprint is stem + widest flood (≈17
    // cells ≈ 4 m) plus the valley halo — 12 m of terrain around
    // the column bounds every carve that can touch it. A wider
    // window would under-estimate by WHOLE valleys (a canyon next
    // to the spawn column pulling the answer down), so the rig
    // would sink 60 m for scenery two ridges away.
    let r = 12.0;
    let mut x = x_m - r;
    while x <= x_m + r {
        let mut y = y_m - r;
        while y <= y_m + r {
            lo = lo.min(elevation_m(x, y, seed, &cfg));
            y += 4.0;
        }
        x += 4.0;
    }
    lo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_cover_terrain_deterministically() {
        let c = cyclone_worldgen::generate_continent(
            2,
            &ContinentConfig {
                size: 512,
                ..Default::default()
            },
        )
        .unwrap();
        let a = build_continent_chunks(&c);
        let b = build_continent_chunks(&c);
        assert!(!a.is_empty(), "seed 2 @ 512 has land");
        assert_eq!(a.len(), b.len());
        let qa: Vec<usize> = a.iter().map(|k| k.builder.quad_count()).collect();
        let qb: Vec<usize> = b.iter().map(|k| k.builder.quad_count()).collect();
        assert_eq!(qa, qb, "chunk contents deterministic");
        // Every top-face quad belongs to a land column; ocean columns
        // appear zero times: sanity on total quad count <= cells*5.
        assert!(
            qa.iter().sum::<usize>() <= (512 / CELL_VOX) as usize * (512 / CELL_VOX) as usize * 5
        );
    }

    /// The shipped chunker DROPS empty chunks — the world is ocean
    /// islands, not a full grid. Any test that indexes chunks by grid
    /// math indexes into a sparse list and learns fiction; the wet
    /// proof therefore FINDS its chunk by anchor, a fact first
    /// witnessed in the very failure that redrew this test (built=1
    /// for 64 grid chunks).
    #[test]
    fn river_quads_paint_water_over_the_bed() {
        let cfg = ContinentConfig {
            size: 512,
            ..Default::default()
        };
        let c = cyclone_worldgen::generate_continent(2, &cfg).unwrap();
        assert!(
            c.water.iter().any(|&w| w > 0),
            "seed 2 @ 512 carries rivers"
        );
        let hue = water_rgb();
        let built = build_continent_chunks(&c);
        // 512² at 8-voxel cells = 64×64 cells: ONE chunk edge, and
        // the empty-drop is invisible at that scale — but the lookup
        // below must still be sparse-correct for full-size worlds.
        let cells = c.config.size / CELL_VOX;
        assert_eq!(built.len(), 1, "a 64-cell world is one chunk block");
        let painted = built.iter().any(|k| k.builder.colours().contains(&hue));
        assert!(painted, "water hue reaches the mesh");

        // Take a stream cell whose bed sits STRICTLY below its water
        // surface AND above sea level: its own quad is guaranteed
        // emitted, so its chunk must exist.
        let vm = c.config.voxel as f64;
        // Match the mesh loop's OWN probe: the cell renders from
        // its corner voxel (multiples of CELL_VOX) — a water cell
        // with dry neighbours can sit below its own surface at the
        // corner and never show. Sample only corner voxels.
        let deep: Vec<usize> = (0..c.config.size / CELL_VOX)
            .flat_map(|cy| (0..c.config.size / CELL_VOX).map(move |cx| (cx, cy)))
            .map(|(cx, cy)| {
                (cy as usize * CELL_VOX as usize) * (c.config.size as usize)
                    + (cx as usize * CELL_VOX as usize)
            })
            .filter(|&i| c.water[i] > c.heights[i] && c.heights[i] > 1)
            .collect();
        assert!(!deep.is_empty(), "the channel carves real depth");
        let d = deep[deep.len() / 2];
        let surf = c.water[d] as f64 * vm;
        let cell = (
            (d % c.config.size as usize) as u32,
            (d / c.config.size as usize) as u32,
        );
        // Chunks are SPARSE (empty ones dropped) and at small sizes
        // WIDER than CELLS_PER_CHUNK — find the block containing the
        // cell by anchor ranges (cell coords), never a grid formula.
        let cell_m = (CELL_VOX as f64 * vm) as f32;
        // UNIT DISCIPLINE: the chunk anchors speak CELLS (2 m); the
        // sampled cell speaks VOXELS (25 cm) — convert once, here.
        let cell_c = (cell.0 / CELL_VOX, cell.1 / CELL_VOX);
        let k = built
            .iter()
            .find(|ch| {
                let cx = (ch.anchor.x / cell_m).round() as u32;
                let cz = (ch.anchor.z / cell_m).round() as u32;
                cell_c.0 >= cx
                    && cell_c.0 < cx + CELLS_PER_CHUNK.min(cells - cx)
                    && cell_c.1 >= cz
                    && cell_c.1 < cz + CELLS_PER_CHUNK.min(cells - cz)
            })
            .unwrap_or_else(|| {
                let near = (0..c.config.size)
                    .filter(|&x| {
                        let i2 = (cell.1 as usize) * (c.config.size as usize) + (x as usize);
                        c.heights[i2] > 0 || c.water[i2] > 0
                    })
                    .count();
                panic!(
                    "chunk @(cell {},{}) missing; voxel row {}: {} land cols; wet@deep idx h{} w{} surf {}; built {}",
                    cell_c.0,
                    cell_c.1,
                    cell.1,
                    near,
                    c.heights[d],
                    c.water[d],
                    surf,
                    built.len()
                )
            });
        assert!(
            k.builder
                .positions()
                .iter()
                .any(|v| (v.y as f64 - surf).abs() < 1e-3),
            "surface {surf} floats over its bed at cell {:?}",
            cell
        );

        // Dry control: same terrain, channel deleted — the water hue
        // must vanish entirely. No phantom rivers.
        let mut dry = cyclone_worldgen::generate_continent(2, &cfg).unwrap();
        dry.water.iter_mut().for_each(|w| *w = 0);
        let dry = build_continent_chunks(&dry);
        assert!(
            !dry.iter().any(|k| k.builder.colours().contains(&hue)),
            "no phantom rivers without the channel"
        );
    }

    /// Full-spec census, run by hand (release): 1 km² generation +
    /// chunking time and quad load — the numbers behind "can this fly".
    #[test]
    #[ignore = "release measurement; run with --release --ignored"]
    fn fullscale_census() {
        let t0 = std::time::Instant::now();
        let (c, chunks) = generate_and_mesh(2, 4000);
        let ms = t0.elapsed().as_millis();
        let quads: usize = chunks.iter().map(|k| k.builder.quad_count()).sum();
        println!(
            "1 km²: gen+mesh {ms} ms, {} chunks, {quads} quads, ocean {:.2}, max {:.0} m",
            chunks.len(),
            c.metrics.ocean_ratio,
            c.metrics.max_height_m
        );
        assert!(ms < 60_000, "full world build regressed hard: {ms} ms");
    }
}
