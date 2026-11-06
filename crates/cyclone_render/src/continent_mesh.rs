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
        (0.855f32, 0.769, 0.545) // sand
    } else if h_m < 260.0 {
        let t = (h_m - 14.0) / 246.0;
        (0.30 - 0.10 * t, 0.62 - 0.20 * t, 0.26) // green deepening
    } else if h_m < 600.0 {
        (0.55, 0.47, 0.28) // hills ochre
    } else if h_m < 900.0 {
        (0.46, 0.45, 0.46) // rock
    } else {
        (0.92, 0.93, 0.94) // snow
    };
    Vec4::new(s(r), s(g), s(b), 1.0)
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
                    if h <= 0.0 {
                        continue; // ocean: the sea plane covers it
                    }
                    let lx = (cx - cx0) as f32 * cell_m;
                    let lz = (cy - cy0) as f32 * cell_m;
                    let hx1 = lx + cell_m;
                    let hz1 = lz + cell_m;
                    let tint = tile_tint(cx, cy, c.seed);
                    let top = earth_rgb(h, tint);
                    b.quad(
                        [
                            Vec3::new(lx, h, lz),
                            Vec3::new(lx, h, hz1),
                            Vec3::new(hx1, h, hz1),
                            Vec3::new(hx1, h, lz),
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
                        h_at(gx * CELL_VOX, gy * CELL_VOX).max(-1.0)
                    };
                    let wall = wall_rgb(h, tint);
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
                        if nh >= h {
                            continue;
                        }
                        let bottom = (nh.max(SEA_FLOOR_M)).min(h);
                        let (x0, x1, z0, z1) = (lx, hx1, lz, hz1);
                        let r = match ring {
                            0 => [
                                Vec3::new(x0, bottom, z0),
                                Vec3::new(x0, h, z0),
                                Vec3::new(x1, h, z0),
                                Vec3::new(x1, bottom, z0),
                            ],
                            1 => [
                                Vec3::new(x1, bottom, z0),
                                Vec3::new(x1, h, z0),
                                Vec3::new(x1, h, z1),
                                Vec3::new(x1, bottom, z1),
                            ],
                            2 => [
                                Vec3::new(x1, bottom, z1),
                                Vec3::new(x1, h, z1),
                                Vec3::new(x0, h, z1),
                                Vec3::new(x0, bottom, z1),
                            ],
                            _ => [
                                Vec3::new(x0, bottom, z1),
                                Vec3::new(x0, h, z1),
                                Vec3::new(x0, h, z0),
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
    elevation_m(x_m, y_m, seed, &cfg)
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
