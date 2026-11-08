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
use crate::srgb_to_vec4;
use crate::terrain::tile_tint;

/// Voxels per mesh cell edge. 8 × 25 cm = 2 m.
pub // Mesh cell edge in voxels: 4 × 0.25 m = 1 m blocks. Was 8 (2 m) —
// pilot round 179 wanted "more definition" at the same 25 cm voxel;
// island mode pays only for the (sparse) land, the sea is one quad.
const CELL_VOX: u32 = 4;
/// Mesh cells per chunk edge (64 × 1 m = 64 m chunks at CELL_VOX=4;
/// the 2 m-cell era made them 128 m — the sparse-drop still trims the
/// sea, and island mode barely notices the finer cut).
const CELLS_PER_CHUNK: u32 = 64;
/// Sea plane sits at 0; land skirts run to this depth (m).
const SEA_FLOOR_M: f32 = -6.0;

/// Aridity field: the slow regional mask that decides vegetation vs
/// scrub vs rock-flat — Earth reads as REGIONS seen from orbit, not
/// altitude stripes. Seed-coupled; deterministic like everything else.
fn aridity(x_m: f32, y_m: f32, seed: u64) -> f64 {
    cyclone_worldgen::noise::fbm(
        f64::from(x_m) / 300.0,
        f64::from(y_m) / 300.0,
        1.0,
        3,
        seed ^ 0xB10E5,
    )
}

/// Elevation ramp: the satellite palette as linear-space vertex
/// colours — with Earth's actual grammar: the lowlands are painted by
/// REGION (aridity), not height (green plains, scrub, bare rock — at
/// any elevation the lowlands span), altitude claims the land only
/// above the vegetation line; sand hugs water at any slope (a beach
/// is a place, not an altitude — cliffs stay rock).
fn earth_rgb(x_m: f32, y_m: f32, h_m: f32, near_wet: bool, tint: f32, seed: u64) -> Vec4 {
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
    // Sand: next to water at any elevation — estuary banks and
    // river-mouth beaches included (a 6 m bank is still a shore).
    if h_m < 2.0 {
        if near_wet {
            // A 1 m island fringe is a BEACH, not the void (island
            // mode: the whole ring sits at fringe heights).
            return srgb_to_vec4(0.80, 0.71, 0.50);
        }
        return Vec4::splat(0.0); // ocean columns are skipped anyway
    }
    let rgb: (f64, f64, f64) = if h_m < 15.0 && near_wet {
        (0.855, 0.769, 0.545) // sand
    } else if h_m < 95.0 {
        // Lowland: the region's own ground, by aridity — never the
        // old three flat bands that read as a traffic light.
        let ar = aridity(x_m, y_m, seed);
        if ar < 0.30 {
            (0.24, 0.55, 0.24) // humid forest green
        } else if ar < 0.55 {
            (0.50, 0.60, 0.30) // grassland
        } else if ar < 0.78 {
            (0.72, 0.62, 0.40) // dry scrub
        } else {
            (0.76, 0.66, 0.47) // near-desert
        }
    } else if h_m < 175.0 {
        // Scrub hills giving way to mountain rock.
        let t = f64::from((h_m - 95.0) / 80.0);
        (
            0.72 * (1.0 - t) + 0.46 * t,
            0.62 * (1.0 - t) + 0.45 * t,
            0.40 * (1.0 - t) + 0.46 * t,
        )
    } else if h_m < 245.0 {
        (0.46, 0.45, 0.46) // rock
    } else {
        (0.92, 0.93, 0.94) // snow
    };
    let (r, g, b) = (rgb.0 as f32, rgb.1 as f32, rgb.2 as f32);
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
fn wall_rgb(x_m: f32, y_m: f32, h_m: f32, tint: f32, seed: u64) -> Vec4 {
    let c = earth_rgb(x_m, y_m, h_m.max(1.0), false, tint, seed);
    Vec4::new(c.x * 0.62, c.y * 0.62, c.z * 0.62, 1.0)
}

/// One chunk: local vertices, world anchor at the chunk corner.
pub struct ContinentChunk {
    pub anchor: Vec3,
    pub builder: QuadBuilder,
}

/// Sea depth buckets (cells per bucket edge) for the water plane.
/// 25 m buckets: bathymetry reads as depth, the table edge as haze.
const SEA_BUCKET: u32 = 100;

/// Distance from a bucket centre to the nearest dry column, by
/// sampling the voxel grid on the bucket's own footprint (max over
/// the bucket = "there is land in this tile"). Exact nearest is a
/// distance transform; at 40 cells across the map the sample is the
/// honest cheap tool and 25 m resolution is below its own error.
fn sea_dist_cells(c: &Continent, bx: u32, by: u32) -> f32 {
    let s = c.config.size as usize;
    // LAND is land — a river crossing an ocean bucket does not make
    // the bucket shallow; water marks channels only over dry ground.
    let wet = |x: u32, y: u32| -> bool { c.heights[(y as usize) * s + (x as usize)] > 0 };
    let cx = bx as usize * SEA_BUCKET as usize + (SEA_BUCKET as usize / 2);
    let cy = by as usize * SEA_BUCKET as usize + (SEA_BUCKET as usize / 2);
    if wet(cx as u32, cy as u32) {
        return 0.0; // land bucket: no quad emitted anyway
    }
    let maxn = c.config.size.div_ceil(SEA_BUCKET);
    let mut best = f32::INFINITY;
    let mut ny = 0u32;
    while ny < maxn {
        let mut nx = 0u32;
        while nx < maxn {
            if wet(
                (nx * SEA_BUCKET + SEA_BUCKET / 2).min(c.config.size - 1),
                (ny * SEA_BUCKET + SEA_BUCKET / 2).min(c.config.size - 1),
            ) {
                let dx = (nx as f32 - bx as f32).abs();
                let dy = (ny as f32 - by as f32).abs();
                best = best.min((dx * dx + dy * dy).sqrt());
            }
            nx += 1;
        }
        ny += 1;
    }
    if best.is_finite() {
        best * SEA_BUCKET as f32
    } else {
        1.0e9
    }
}

/// The sea as terrain data deserves: one quad per 25 m bucket,
/// coloured by distance to shore — shallow cyan over shelves,
/// deep navy in basins. The flat white plane is deleted; Earth's
/// ocean is not a solid colour.
pub fn build_sea(c: &Continent) -> Option<QuadBuilder> {
    let n = c.config.size / SEA_BUCKET;
    if n < 2 {
        return None; // too small to hold a shelf gradient
    }
    let cell_m = SEA_BUCKET as f32 * c.config.voxel;
    let mut b = QuadBuilder::new();
    for by in 0..n {
        for bx in 0..n {
            let cx = bx * SEA_BUCKET + SEA_BUCKET / 2;
            let cy = by * SEA_BUCKET + SEA_BUCKET / 2;
            let s = c.config.size as usize;
            let i = (cy as usize) * s + (cx as usize);
            if c.heights[i] > 0 || c.water[i] > 0 {
                continue; // land bucket: the mesh draws its top
            }
            let d = sea_dist_cells(c, bx, by);
            // Shelf fade over ~1 km of open water (buckets speak
            // cells; 4 cells = 1 m).
            // Shelf ramp 150 m (d in cells of 0.25 m): matches the
            // sheet's turquoise band width.
            let t = (d * 0.25 / 150.0).min(1.0);
            // Shore turquoise → open-ocean navy (the reference photo:
            // a bright band, then deep blue — not one blue).
            let (r, g, bl) = (
                0.180 * (1.0 - t) + 0.012 * t,
                0.650 * (1.0 - t) + 0.055 * t,
                0.720 * (1.0 - t) + 0.160 * t,
            );
            let col = {
                let lin = |v: f32| {
                    if v <= 0.04045 {
                        v / 12.92
                    } else {
                        ((v + 0.055) / 1.055).powf(2.4)
                    }
                };
                Vec4::new(lin(r), lin(g), lin(bl), 1.0)
            };
            let (x0, z0) = (bx as f32 * cell_m, by as f32 * cell_m);
            let (x1, z1) = (x0 + cell_m, z0 + cell_m);
            b.quad(
                [
                    Vec3::new(x0, 0.0, z0),
                    Vec3::new(x0, 0.0, z1),
                    Vec3::new(x1, 0.0, z1),
                    Vec3::new(x1, 0.0, z0),
                ],
                Vec3::Y,
                col,
            );
        }
    }
    (!b.is_empty()).then_some(b)
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
        // Margin probes read one cell past the grid: past-edge is
        // dry, which is exactly 0.
        if cx >= c.config.size || cy >= c.config.size {
            return 0.0;
        }
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
                    // Beach grammar: this cell touches water (cell
                    // probe + its four edges — 2 m cells miss nothing
                    // wider than a single column's channel).
                    // Wet = river water OR the sea (height 0 next
                    // door is sea floor — its neighbours are a shore;
                    // island beach rings live here, not in the water
                    // array, which only marks rivers).
                    let sea_at = |x: u32, y: u32| -> bool {
                        if x >= c.config.size || y >= c.config.size {
                            return true; // past the rim: ocean
                        }
                        c.heights[(y as usize) * (c.config.size as usize) + (x as usize)] == 0
                    };
                    let edge_sea = (cx > 0 && sea_at(cx * CELL_VOX - 1, cy * CELL_VOX))
                        || sea_at(cx * CELL_VOX + CELL_VOX, cy * CELL_VOX)
                        || (cy > 0 && sea_at(cx * CELL_VOX, cy * CELL_VOX - 1))
                        || sea_at(cx * CELL_VOX, cy * CELL_VOX + CELL_VOX);
                    let near_wet = is_wet
                        || edge_sea
                        || w_at(cx * CELL_VOX + CELL_VOX / 2, cy * CELL_VOX + CELL_VOX / 2) > 0.0
                        || (cx > 0 && w_at((cx - 1) * CELL_VOX, cy * CELL_VOX) > 0.0)
                        || w_at((cx + 1) * CELL_VOX, cy * CELL_VOX) > 0.0
                        || (cy > 0 && w_at(cx * CELL_VOX, (cy - 1) * CELL_VOX) > 0.0)
                        || w_at(cx * CELL_VOX, (cy + 1) * CELL_VOX) > 0.0;
                    let xm = cx as f32 * cell_m;
                    let zm = cy as f32 * cell_m;
                    let top = if is_wet {
                        water_rgb()
                    } else {
                        earth_rgb(xm, zm, h, near_wet, tint, c.seed)
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
                        wall_rgb(xm, zm, h, tint, c.seed)
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

/// Re-export of the worldgen crate's ONE reader of CYCLONE_ISLANDS
/// (round 226: the fn body moved to its single home beside the config
/// field it feeds — this path stays for existing callers).
pub use cyclone_worldgen::island_mode;

pub fn generate_and_mesh(seed: u64, size: u32) -> (Continent, Vec<ContinentChunk>) {
    let cfg = ContinentConfig {
        size,
        islands: cyclone_worldgen::island_mode(),
        ..Default::default()
    };
    let c = generate_continent(seed, &cfg).expect("size within contract; dev preview dial");
    let chunks = build_continent_chunks(&c);
    (c, chunks)
}

/// Find the BEST standable ground near a preferred spot, scanning
/// golden-angle rings (40 m steps, ~7 angles/ring — 500 samples per
/// 700 m radius, pure-function cheap). Island mode makes the map
/// centre OCEAN for most seeds: dropping the pilot into open water
/// and saying "look, an island, somewhere" is not a spawn. Land
/// requires 3 m of honest height (the sampler's river-safe lower
/// bound). Preference: HIGHEST ground found within the radius, ties
/// to the nearer (strict > keeps the first touch — and on an island
/// world the highest ground is the flagship: a 4 m bar 160 m out beat
/// a 93 m island 393 m out under first-touch, witnessed 2026-11-08;
/// this is a flying camera, altitude IS the view). None = nothing
/// standable within the search radius — the caller decides the
/// fallback.
pub fn spawn_over_land(
    pref_x_m: f64,
    pref_y_m: f64,
    seed: u64,
    size: u32,
    max_r_m: f64,
) -> Option<(f64, f64, f64)> {
    let step = 40.0;
    let mut r = 0.0f64;
    let golden = std::f64::consts::PI * (3.0 - 5.0f64.sqrt()); // ≈137.5°
    let mut k = 0usize;
    let mut best: Option<(f64, f64, f64)> = None; // (x, y, height)
    while r <= max_r_m {
        // Ring circumference / step samples, golden-angle rotated so
        // successive rings don't march the same spokes.
        let n = ((2.0 * std::f64::consts::PI * r / step) as usize).clamp(1, 36);
        for j in 0..n {
            let a = golden * (k as f64 + j as f64);
            let (x, y) = (pref_x_m + r * a.cos(), pref_y_m + r * a.sin());
            if !(0.0..=size as f64 * 0.25).contains(&x) || !(0.0..=size as f64 * 0.25).contains(&y)
            {
                continue;
            }
            let h = surface_height_m(x, y, seed, size);
            if h >= 3.0 && best.is_none_or(|(_, _, bh)| h > bh) {
                best = Some((x, y, h));
            }
        }
        k += n;
        r += step;
    }
    best
}

/// The pure elevation sampler (re-exported for rig placement: the game
/// asks "how high is the ground at spawn" without the whole heightfield).
pub fn surface_height_m(x_m: f64, y_m: f64, seed: u64, size: u32) -> f64 {
    let cfg = ContinentConfig {
        size,
        islands: cyclone_worldgen::island_mode(),
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

/// The classic archipelago's sea, shaded by the baked `shore_dist`
/// grid (spec 02:66 — the data has ALWAYS been there; until now the
/// sea was one blue plane that never read it). Same ramp constants
/// as `build_sea` (continent) so the two modes share one ocean
/// language, plus spec 02 B2's white foam band at shore_dist ≤ 1.
/// Sea tile = height 0 (the waterline definition the whole pipeline
/// shares). Determinism: integer distance → fixed float ramps, the
/// same arithmetic the continent sea already ships with.
pub fn build_sea_classic(map: &cyclone_worldgen::map::WorldMap) -> Option<QuadBuilder> {
    let (w, h) = map.size;
    if w == 0 || h == 0 {
        return None;
    }
    let mut b = QuadBuilder::new();
    let mut painted = 0u64;
    for y in 0..h {
        for x in 0..w {
            if map.height.raw(x, y) != 0 {
                continue; // land: the terrain mesh paints this tile
            }
            let d = map.shore_dist.raw(x, y).min(15) as f32;
            let t = (d / 15.0).min(1.0);
            let (mut r, mut g, mut bl) = (
                0.180f32 * (1.0 - t) + 0.012 * t,
                0.650 * (1.0 - t) + 0.055 * t,
                0.720 * (1.0 - t) + 0.160 * t,
            );
            // Foam: spec 02's band is shore_dist ≤ 1 — full white at
            // d = 1, fading by d = 3 so the band has an edge, not a
            // cliff. (d = 0 is land-adjacent sub-tiles; the ramp
            // keeps it brightest.)
            let foam = ((3.0 - d) / 2.0).clamp(0.0, 1.0) * 0.85;
            r += (0.90 - r) * foam;
            g += (0.95 - g) * foam;
            bl += (0.97 - bl) * foam;
            let col = crate::srgb_to_vec4(r, g, bl);
            let (x0, z0) = (x as f32, y as f32);
            b.quad(
                [
                    Vec3::new(x0, -0.02, z0),
                    Vec3::new(x0, -0.02, z0 + 1.0),
                    Vec3::new(x0 + 1.0, -0.02, z0 + 1.0),
                    Vec3::new(x0 + 1.0, -0.02, z0),
                ],
                Vec3::Y,
                col,
            );
            painted += 1;
        }
    }
    (painted > 0).then_some(b)
}

#[cfg(test)]
mod tests {
    /// Binary-wide env lock for every test that reads or writes
    /// CYCLONE_ISLANDS (generate_and_mesh reads the env directly —
    /// any unlocked reader blinks between worlds under load).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    fn spawn_scan_finds_an_island_from_the_middle_of_the_sea() {
        // Island mode, seed 42: the map centre is open water (the
        // hillshade proves it). The ring scan must still deliver
        // standable ground, deterministically, within the search.
        // Edition-2024: env mutation is unsafe; tests run single-thread
        // per binary but the scan test owns the dial alone in this file.
        // SAFETY: sole writer of this key in this test binary (grep-
        // proven: the other env-reading tests are in cyclone_render's
        // integration tests, separate processes).
        unsafe { std::env::set_var("CYCLONE_ISLANDS", "1") };
        let (x, y, h) = spawn_over_land(500.0, 500.0, 42, 4000, 500.0)
            .expect("islands exist within 500 m of centre");
        // The flagship contract (witnessed 2026-11-08): first-touch
        // hit a 4 m bar 160 m out; highest-ground must instead pick
        // the ~93 m flagship ~360 m out. This is a flying camera —
        // altitude IS the view.
        assert!(h > 20.0, "flagship, not a sandbar: {h} m");
        let d = ((x - 500.0).powi(2) + (y - 500.0).powi(2)).sqrt();
        assert!(d <= 500.0 + 1.0, "inside the search radius: {d} m");
        eprintln!("scan from open sea: island at ({x:.0}, {y:.0}), {h} m, {d:.0} m out");
        let again = spawn_over_land(500.0, 500.0, 42, 4000, 500.0).unwrap();
        assert_eq!((x, y, h), again, "pure function, same answer");
        // SAFETY: as above — restore after the assertion set.
        unsafe { std::env::remove_var("CYCLONE_ISLANDS") };
    }

    #[test]
    fn islands_wear_sand_rings_at_the_sea() {
        // Island mode with the real dial: every low island column
        // touching sea floor must come back sand — the ring in the
        // pilot's reference photo, now data-driven not altitude-driven.
        let cfg = cyclone_worldgen::ContinentConfig {
            size: 1024,
            islands: true,
            ..Default::default()
        };
        let c = cyclone_worldgen::generate_continent(7, &cfg).unwrap();
        let s = c.config.size as usize;
        let mut beach = 0;
        let mut green_shore = 0;
        for y in 1..c.config.size - 1 {
            for x in 1..c.config.size - 1 {
                let i = y as usize * s + x as usize;
                if c.heights[i] == 0 || c.heights[i] as f64 * c.config.voxel as f64 > 12.0 {
                    continue;
                }
                let sea_n = c.heights[i - 1] == 0
                    || c.heights[i + 1] == 0
                    || c.heights[i - s] == 0
                    || c.heights[i + s] == 0;
                if !sea_n {
                    continue;
                }
                let rgb = earth_rgb(
                    x as f32 * c.config.voxel,
                    y as f32 * c.config.voxel,
                    c.heights[i] as f32 * c.config.voxel,
                    true,
                    1.0,
                    c.seed,
                );
                // Sand is the warm colour: red dominant over blue.
                if rgb.x > rgb.z {
                    beach += 1;
                } else {
                    green_shore += 1;
                }
            }
        }
        assert!(
            beach > 100,
            "island beaches exist: {beach} sand, {green_shore} not"
        );
        assert_eq!(green_shore, 0, "no green at the waterline");
    }

    #[test]
    fn sea_mesh_is_bathymetry_not_a_flat_table() {
        // 2048² not 1024²: the land mask's own wavelength means a
        // 256 m span is all-land (witnessed: 100/100 buckets); a sea
        // needs room to exist — 512 m span, seed 777 (ocean 0.48).
        let cfg = ContinentConfig {
            size: 2048,
            ..Default::default()
        };
        let c = cyclone_worldgen::generate_continent(777, &cfg).unwrap();
        let n = c.config.size / SEA_BUCKET;
        let land_b = (0..n)
            .flat_map(|by| (0..n).map(move |bx| (bx, by)))
            .filter(|(bx, by)| {
                let cx = bx * SEA_BUCKET + SEA_BUCKET / 2;
                let cy = by * SEA_BUCKET + SEA_BUCKET / 2;
                let i = (cy as usize) * (c.config.size as usize) + (cx as usize);
                c.heights[i] > 0 || c.water[i] > 0
            })
            .count();
        let b = build_sea(&c).unwrap_or_else(|| {
            panic!(
                "buckets {n}x{n}, land {land_b}, sea {}",
                n * n - land_b as u32
            )
        });
        let cols: std::collections::HashSet<(u32, u32, u32)> = b
            .colours()
            .iter()
            .step_by(4) // one sample per quad (quads push 4 verts)
            .map(|v| (v.x.to_bits(), v.y.to_bits(), v.z.to_bits()))
            .collect();
        assert!(
            cols.len() > 3,
            "depth varies across the sea: {} distinct hues",
            cols.len()
        );
        assert!(
            !cols.contains(&(1.0f32.to_bits(), 1.0f32.to_bits(), 1.0f32.to_bits())),
            "the flat white table is gone from the sea"
        );
        // The builder's own contract: every quad at the plane. The
        // dark safety plane beneath it is the rebuild's, not this mesh.
        let below = b.positions().iter().filter(|p| p.y < 0.0).count();
        assert_eq!(below, 0, "sea quads sit at 0 — no hidden floor here");
    }

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
        // 512² at 4-voxel cells = 128×128 cells = 2×2 chunk blocks;
        // land spans them all, so none is empty-dropped — and the
        // lookup below must stay sparse-correct for full-size worlds.
        let cells = c.config.size / CELL_VOX;
        assert_eq!(built.len(), 4, "128-cell world is 2×2 chunk blocks");
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
    /// MAINLAND, explicitly: run dial-OFF (its dial-blindness is by
    /// design here), as every number it ever fed was mainland.
    /// Island-scale census: DECISIONS rounds 284-285 (dial SET —
    /// round 285's law: `generate_and_mesh` reads `island_mode()`
    /// AT CALL TIME; a caller that means islands must set the env
    /// dial, not just the config field).
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

    /// Byte-identity for the MESH layer: the worldgen canaries pin the
    /// ground; this pins the SKIN — anchors, positions, colours, quad
    /// counts, hashed. Unit home (in-crate) so pub(crate) builder
    /// accessors serve the digest without widening the product API.
    /// CI runs it on Linux/macOS/Windows: drift red before pilot eyes.
    #[test]
    fn islands_mesh_digest_platform_identical() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        struct IsReset;
        impl Drop for IsReset {
            fn drop(&mut self) {
                unsafe { std::env::remove_var("CYCLONE_ISLANDS") };
            }
        }
        let _reset = IsReset;
        unsafe { std::env::set_var("CYCLONE_ISLANDS", "1") };
        let (_c, chunks) = generate_and_mesh(2, 2048);
        let mut h = blake3::Hasher::new();
        h.update(&(chunks.len() as u64).to_le_bytes());
        // Centimetre quantisation is the point of this digest, not
        // an optimisation: raw f32 hashing (round 194's first cut)
        // went RED on macOS the same day it was pinned — libm's
        // transcendentals drift by ULPs across platforms, and IEEE
        // guarantees none of that away. Quantised to 0.01 m — 1/25
        // of a voxel, ~1000× under visible — the digest tests what
        // actually matters: every platform builds the SAME world
        // structure. Structural drift (chunk sets, island shapes,
        // ordering) still moves the hash by oceans.
        fn cm(h: &mut blake3::Hasher, f: f32) {
            h.update(&((f * 100.0).round_ties_even() as i64).to_le_bytes());
        }
        for k in &chunks {
            for f in k.anchor.to_array() {
                cm(&mut h, f);
            }
            for p in k.builder.positions() {
                for f in p.to_array() {
                    cm(&mut h, f);
                }
            }
            for c in k.builder.colours() {
                for f in c.to_array() {
                    cm(&mut h, f); // colours ride 0..1 — quantises too
                }
            }
            h.update(&(k.builder.quad_count() as u64).to_le_bytes());
        }
        let d = h.finalize().to_string();
        println!("islands mesh digest: {d}");
        assert_eq!(
            d, "b8794e3234a77239b6a3be3fcdf0796255da4c3542fd913158b770ff36199d71",
            "mesh drifted beyond 1 cm — a platform builds DIFFERENT terrain, investigate"
        );
    }

    /// The prewarm's remaining main-thread bill: worker carving is
    /// free, but CLAIMING it assetifies every chunk mesh on the
    /// render thread. 512² default-continent census: if this ever
    /// grows past seconds, the pop-in became the new freeze — this
    /// assert names the exact culprit frame. (Islands 2048² runs in
    /// the rebuild tests; 512² keeps this one census-cheap.)
    #[test]
    fn claim_frame_assetification_is_bounded() {
        // generate_and_mesh reads CYCLONE_ISLANDS — under the same
        // binary-wide lock as the digest test (parallel suite,
        // first run: the continent silently built islands because
        // a sibling had the env set; the cap fired and said so).
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (_c, chunks) = generate_and_mesh(2, 512);
        assert!(!chunks.is_empty());
        let t0 = std::time::Instant::now();
        // Assets is a plain resource — no App scaffold needed for
        // the timing (and the headless tests stay scaffold-free).
        let mut meshes = Assets::<Mesh>::default();
        let n = chunks
            .into_iter()
            .map(|ch| meshes.add(ch.builder.into_mesh()))
            .count();
        let ms = t0.elapsed().as_millis();
        println!("claim-frame assetify: {n} chunks in {ms} ms");
        assert!(ms < 3_000, "the pop-in became a freeze: {ms} ms");
    }

    /// Spec 02 B2's band, witnessed: the classic sea mesh must carry
    /// FOAM (near-white vertices hugging coasts), SHALLOWS (bright
    /// turquoise mid-band) and DEEP (dark navy) — three distinct
    /// registers, not the historic one-blue plane. Determinism:
    /// same map, byte-identical palette sets across builds.
    #[test]
    fn classic_sea_wears_foam_shallows_and_deep() {
        use cyclone_worldgen::{ProgressNoop, Seed, WorldGenConfig, generate};
        let seed: Seed = "AAA-AAAA".parse().unwrap();
        let map = generate(seed, 1, &WorldGenConfig::default(), &ProgressNoop).unwrap();
        let b = build_sea_classic(&map).expect("sea quads on a real map");
        let cols: Vec<Vec4> = b.colours().to_vec();
        let is_white = |c: &Vec4| c.x > 0.35 && c.y > 0.6 && c.z > 0.7; // linear foam
        let is_deep = |c: &Vec4| c.x < 0.01 && c.y < 0.03 && c.z < 0.08;
        let is_mid = |c: &Vec4| c.x > 0.008 && c.x < 0.06 && c.y > 0.12;
        assert!(
            cols.iter().any(is_white),
            "no foam band: the ≤1-tile promise is unspent"
        );
        assert!(cols.iter().any(is_mid), "no turquoise shallows");
        assert!(cols.iter().any(is_deep), "no deep ocean register");
        let b2 = build_sea_classic(&map).unwrap();
        assert_eq!(b.colours(), b2.colours(), "sea palette is deterministic");
    }

    /// Round 228: the dial's value semantics, pinned. '1' is ON;
    /// EVERYTHING ELSE — including '0' — is OFF. (Before this test,
    /// presence itself was ON, so '=0' meant ON: the pilot-typing
    /// trap. env-locked like every CYCLONE_* writer.)
    #[test]
    fn continent_dial_is_strict_one() {
        let _g = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("CYCLONE_CONTINENT", "1") };
        assert!(crate::continent_mode().is_some(), "'1' must be ON");
        unsafe { std::env::set_var("CYCLONE_CONTINENT", "0") };
        assert!(
            crate::continent_mode().is_none(),
            "'=0' read as ON — the round-228 value trap"
        );
        unsafe { std::env::set_var("CYCLONE_CONTINENT", "yes") };
        assert!(crate::continent_mode().is_none(), "'yes' is not '1'");
        unsafe { std::env::remove_var("CYCLONE_CONTINENT") };
        assert!(crate::continent_mode().is_none(), "unset is OFF");

        // The SIZE half of the round-229 value law, same lock, same
        // honesty: the docs promise clamp 64–16 000 (worldgen caps at
        // 16 384) against the 0.25 m/voxel invariant — now law.
        unsafe {
            std::env::set_var("CYCLONE_CONTINENT", "1");
            std::env::set_var("CYCLONE_CONTINENT_SIZE", "10");
        }
        assert_eq!(
            crate::continent_mode().map(|(s, _)| s),
            Some(64),
            "small SIZE must clamp UP to 64, not shrink past sanity"
        );
        unsafe { std::env::set_var("CYCLONE_CONTINENT_SIZE", "999999") };
        assert_eq!(
            crate::continent_mode().map(|(s, _)| s),
            Some(16_000),
            "huge SIZE must clamp DOWN to 16 000"
        );
        unsafe { std::env::set_var("CYCLONE_CONTINENT_SIZE", "1000") };
        let (s_side, extent) = crate::continent_mode().expect("dial on");
        assert_eq!(s_side, 1000);
        assert!(
            (extent - 250.0).abs() < 1e-3,
            "extent must be size × 0.25 m — the voxel invariant of round 225"
        );
        unsafe { std::env::set_var("CYCLONE_CONTINENT_SIZE", "not-a-number") };
        assert_eq!(
            crate::continent_mode().map(|(s, _)| s),
            Some(4000),
            "garbage SIZE falls back to the 4000 default (parse().ok() path)"
        );
        unsafe {
            std::env::remove_var("CYCLONE_CONTINENT_SIZE");
            std::env::remove_var("CYCLONE_CONTINENT");
        }
    }
}
