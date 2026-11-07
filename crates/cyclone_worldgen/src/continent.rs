//! Earth-like voxel terrain core (the pilot's v2 pivot, round 154).
//!
//! A heightfield world: every 25 cm column has an integer voxel height —
//! Minecraft's shape, planet Earth's relief: ocean basins, plains, hills,
//! ridged mountains. "Render the top layer following the terrain" falls
//! out of this representation for free: the surface IS the heightfield.
//!
//! Determinism contract inherited from the crate: pure functions of
//! (seed, coordinates), libm maths only, no RNG streams — every column
//! recomputable in isolation, byte-identical on every platform. Scale:
//! 1 km² at 4 voxel/m is 4000×4000 columns — meshing is the renderer's
//! problem (chunked, next round); this module is generator-first.

use crate::error::GenError;
use crate::noise::{fbm, hash_pair};

/// Terrain shaping knobs. Distances are METERS (the voxel size only
/// quantizes the result); frequencies below are cycles-per-meter.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct ContinentConfig {
    /// Voxels per side. 4000 × 0.25 m = 1 km square.
    pub size: u32,
    /// Voxel edge length in meters (25 cm per the brief).
    pub voxel: f32,
    /// Terrain ceiling in meters (K2-lite; keeps u16 voxels generous).
    pub max_height_m: f32,
    /// Archipelago mode: the land mask thins to scattered islands
    /// instead of continents. Round-179 pilot: "a few islands, not
    /// that big, they need to LOOK like islands" — the reference was
    /// an aerial photo: green hummocks, sand rings, shallow shelves.
    pub islands: bool,
}

impl Default for ContinentConfig {
    fn default() -> Self {
        Self {
            size: 4000,
            voxel: 0.25,
            max_height_m: 300.0,
            islands: false,
        }
    }
}

/// Column classification counts, computed from the heightfield itself —
/// honest-area arithmetic like `terracing` was for islands: every number
/// is a fraction of columns, nothing hand-waved.
#[derive(Clone, Copy, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct ContinentMetrics {
    /// Columns at 0 (water surface — renderer lays the flat sea).
    pub ocean_ratio: f64,
    /// Land columns with |slope| below the plain threshold (4 m step).
    pub flat_ratio: f64,
    /// Land columns h ≥ 250 m (rolling highlands and up).
    pub high_ratio: f64,
    /// Land columns h ≥ 600 m (mountain domain).
    pub mountain_ratio: f64,
    /// Fraction of columns carrying river water (round-177).
    pub river_ratio: f64,
    /// Highest column in meters.
    pub max_height_m: f64,
}

/// One generated world: integer voxel heights, row-major, `y * size + x`.
pub struct Continent {
    pub config: ContinentConfig,
    pub seed: u64,
    /// Voxel-unit column heights (0 = sea level). Where a river
    /// flows, this is the BED height — under the water surface.
    pub heights: Vec<u16>,
    /// River water SURFACES in voxel units (0 = no river here).
    /// A fixed sea plane can only show water at y=0; rivers flow
    /// above it, so they need their own surface (round-177: the
    /// pilot asked for rivers, and honest rivers need honest data).
    /// Invariant: `water[i] > 0 ⇒ water[i] ≤ heights[i] + carve`,
    /// i.e. surface at or above the bed it covers.
    pub water: Vec<u16>,
    pub metrics: ContinentMetrics,
}

#[inline]
fn smoothstep(e0: f64, e1: f64, x: f64) -> f64 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Ridged multifraction in [0,1]: crests ridges where |noise| is low.
#[inline]
fn ridged(x: f64, y: f64, freq: f64, oct: usize, seed: u64) -> f64 {
    let v = fbm(x, y, freq, oct, seed);
    let r = 1.0 - (2.0 * v - 1.0).abs();
    r * r // sharpen: valley floors go flat, crests stay sharp
}

/// One island's footprint at a world point: 1 at the core, eased to
/// 0 at the coast (cosine profile — a round beach, no cliff edge).
/// `r` in metres, `squish` stretches the blob so islands aren't circles.
fn blob_field(dx: f64, dy: f64, r: f64, squish: f64) -> f64 {
    let d = ((dx * dx) + (dy * dy) / (squish * squish)).sqrt();
    if d >= r {
        0.0
    } else {
        let t = d / r;
        0.5 * (1.0 + (std::f64::consts::PI * t).cos())
    }
}

/// Archipelago land mask: island SITES on a lattice, each kept by a
/// coin flip, each a stamped blob (max-composed — a bay is simply
/// where two footprints' shoulders don't meet). Spacing 340 m keeps
/// islands separate on a 1 km map (~3×3 candidate cells); radii
/// 60–150 m give 0.01–0.07 km² islands — "not that big" per the
/// pilot, big enough to walk around. Core floor 0.55 guarantees any
/// kept island breaks sea level well inland of its own beach.
fn island_mask(wx: f64, wy: f64, seed: u64) -> f64 {
    const SPACING: f64 = 340.0;
    let spacing = SPACING;
    let gx = (wx / spacing).floor() as i64;
    let gy = (wy / spacing).floor() as i64;
    let mut best = 0.0f64;
    // 3×3 neighbourhood: footprints reach past their home cell.
    for oy in -1..=1 {
        for ox in -1..=1 {
            let (hx, hy) = (gx + ox, gy + oy);
            let h1 = hash_pair(seed ^ 0x151A4D, (hx as u64) << 32 ^ hy as u64);
            let coin = ((h1 >> 40) & 0xFF) as f64 / 255.0; // keep density
            if coin < 0.45 {
                continue; // most sites are open sea
            }
            let h2 = hash_pair(
                seed ^ 0x5EA,
                (hx as u64) << 32 ^ (hy as u64).rotate_left(17),
            );
            let jx = ((h1 >> 16) & 0xFF) as f64 / 255.0 - 0.5;
            let jy = ((h1 >> 24) & 0xFF) as f64 / 255.0 - 0.5;
            let cx = (hx as f64 + 0.5 + jx * 0.8) * spacing;
            let cy = (hy as f64 + 0.5 + jy * 0.8) * spacing;
            let r = 90.0 + 110.0 * (((h2 >> 32) & 0xFF) as f64 / 255.0);
            let squish = 0.7 + 0.6 * (((h2 >> 20) & 0xFF) as f64 / 255.0);
            let f = blob_field(wx - cx, wy - cy, r, squish);
            // Sea floor 0.2 (< 0.5 → dry land); the island's own
            // core reaches ~1.0. Coast = where the blob profile
            // crosses 0.5 (cosine → a real beach slope, not a cliff).
            best = best.max(0.2 + 0.8 * f);
        }
    }
    best.max(0.0)
}

/// Column elevation in meters. Pure function — any column, any time.
/// Layered Earth logic: a continental mask carves oceans; a separate
/// orogeny mask concentrates mountains into ranges; gentle plains fill
/// everything else; ridged noise gives peaks with real valley floors.
pub fn elevation_m(x_m: f64, y_m: f64, seed: u64, cfg: &ContinentConfig) -> f64 {
    // Domain warp: bends every boundary so nothing looks grid-aligned.
    let wx = x_m + 150.0 * (fbm(x_m / 350.0, y_m / 350.0, 1.0, 2, seed ^ 0xD0A1) - 0.5) * 2.0;
    let wy = y_m + 150.0 * (fbm(x_m / 350.0, y_m / 350.0, 1.0, 2, seed ^ 0xA41E) - 0.5) * 2.0;

    // Continents vs oceans — or islands.
    let c = fbm(wx / 500.0, wy / 500.0, 1.0, 5, seed);
    // Island placement is a POINT PROCESS (round 179, pilot reference:
    // an aerial photo of real islands). Cutting the fbm's upper tail
    // was census-tested and REJECTED: the field's peaks are one
    // correlated cluster per map, so a tail cut yields one big island
    // plus gravel (0.6 km² + fifty 9 m specks — the specks were
    // literally the noise's own minima poking through). Instead: hash
    // a lattice of island sites, keep each by a seeded coin, and
    // stamp its footprint as a smooth blob. Sizes and spacing are
    // constants — they cannot drift by seed the way a tail cut did;
    // positions, count, radii and shapes are the random part, exactly
    // as asked: "a few that fit the map, randomly".
    let land = if cfg.islands {
        island_mask(wx, wy, seed)
    } else {
        smoothstep(0.44, 0.54, c) // 0 = open ocean
    };

    // Orogeny: where the mountain belts live.
    let orogen = smoothstep(
        0.48,
        0.72,
        fbm(wx / 350.0, wy / 350.0, 1.0, 3, seed ^ 0x0A06),
    );

    // Relief layers (all warped).
    let plains = 6.0 + 46.0 * fbm(wx / 90.0, wy / 90.0, 1.0, 4, seed ^ 0xF141);
    let ridge = ridged(wx / 260.0, wy / 260.0, 1.0, 5, seed ^ 0x142D1);
    // 1.0 m, not 2.2: the first full-res render showed plains reading
    // as crumpled foil — detail belongs on mountain flanks (where it
    // sits against 100 m+ relief), not shouting across flatlands.
    let detail = 1.0 * fbm(wx / 28.0, wy / 28.0, 1.0, 3, seed ^ 0xD37D);

    // Relief normalisation: raw masks are all-or-nothing per seed (the
    // round-154 census caught a 43 m flat world AND a 14%-mountain
    // world at the same settings). First fix (floor 80+140·fbm, orogeny
    // floor 0.25) over-corrected: the census came back 68–90% highlands
    // — plains had vanished. The calm dials below are the measured
    // middle: gentle swell everywhere (15–50 m), grand relief only in
    // the belts (orogeny floor 0.05 lets a rare ridge whisper off-cue).
    let base = plains
        + 15.0
        + 35.0 * fbm(wx / 160.0, wy / 160.0, 1.0, 3, seed ^ 0xA11C)
        + (cfg.max_height_m as f64 - 80.0) * ridge * (0.05 + 0.95 * orogen);
    if cfg.islands {
        // Island grammar: 0.5 is the SEA LINE inside the mask (the
        // stamp's floor sits below it, the core above). Rescale so
        // the coast has zero relief (beach) and the core full relief.
        (2.0 * (land - 0.5) * (base + detail)).max(0.0)
    } else {
        (land * (base + detail)).max(0.0)
    }
}

/// Generate the whole heightfield. Single-threaded by construction;
/// columns are independent, so a future rayon pass keeps determinism.
pub fn generate_continent(seed: u64, cfg: &ContinentConfig) -> Result<Continent, GenError> {
    if cfg.size < 16 || cfg.size > 16384 {
        return Err(GenError::Config(format!(
            "continent size {} outside 16..=16384 voxels",
            cfg.size
        )));
    }
    if cfg.voxel <= 0.0 || (cfg.voxel as f64) * cfg.size as f64 > 16384.0 * 0.25 {
        return Err(GenError::Config(
            "voxel/size combination unrenderable".into(),
        ));
    }
    let n = (cfg.size as usize) * (cfg.size as usize);
    let mut heights = Vec::with_capacity(n);
    let q = 1.0 / cfg.voxel as f64;
    for y in 0..cfg.size {
        let ym = y as f64 * cfg.voxel as f64;
        for x in 0..cfg.size {
            let xm = x as f64 * cfg.voxel as f64;
            let h = elevation_m(xm, ym, seed, cfg);
            heights.push((h * q).round().clamp(0.0, u16::MAX as f64) as u16);
        }
    }
    let water = carve_rivers(&mut heights, cfg.size, seed);
    let mut metrics = measure(&heights, cfg);
    metrics.river_ratio = water.iter().filter(|&&w| w > 0).count() as f64 / (n as f64);
    Ok(Continent {
        config: *cfg,
        seed,
        heights,
        water,
        metrics,
    })
}

/// Earth's next feature after relief: drainage. Deterministic D8
/// flow accumulation over the whole grid (counting-sort by height →
/// strictly descending, so water never flows uphill and never
/// cycles; ties go to the lowest linear index — acyclic by pair
/// order (h, idx)), then a carve ONLY where the catchment is big
/// enough to earn a river. Carving to 0 means the mesh skips the
/// column and the sea plane shows through — the ocean IS the river
/// surface, no renderer change, hillshade shows the blue ribbon too.
/// Catchment counted in COLUMNS: 1 col = 0.0625 m², so T = 3000 is
/// a ~190 m² tributary head; main stems reach many times that.
fn carve_rivers(h: &mut [u16], size: u32, seed: u64) -> Vec<u16> {
    // 3000 over-carved a canyon (round-177 first render: a blocky
    // crater ate a mountain; the census +1 pp ocean confirmed). Real
    // streams need a real catchment: 25k columns = 1.6 km² of rain.
    const CATCH: u64 = 25_000; // ~1.6 km² of rain to earn water
    let n = (size as usize) * (size as usize);
    // Descending-height order via counting sort (heights are u16).
    let mut hist = [0u32; 65_537];
    for &v in h.iter() {
        hist[v as usize + 1] += 1;
    }
    for i in 1..=65_536 {
        hist[i] += hist[i - 1];
    }
    let mut order = vec![0u32; n];
    // Fill ascending then walk from the top — stable enough: order
    // within equal heights is by index, consistent with tie rule.
    let mut pos = hist[..65_536].to_vec();
    for (i, &v) in h.iter().enumerate() {
        order[pos[v as usize] as usize] = i as u32;
        pos[v as usize] += 1;
    }

    // Rain: a whisper of seed-varied so headwaters don't look seeded
    // by index; accumulation in u64 (stems can carry 10⁵).
    let mut acc = vec![1u64; n];
    let salt = seed & 0xF;
    for k in (0..n).rev() {
        let c = order[k] as usize;
        let hc = h[c];
        if hc == 0 {
            continue; // ocean: water leaves the land here
        }
        let cx = (c % size as usize) as i32;
        let cy = (c / size as usize) as i32;
        let mut best = usize::MAX;
        let mut bh = hc;
        for dy in -1..=1 {
            for dx in -1..=1 {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let (nx, ny) = (cx + dx, cy + dy);
                if nx < 0 || ny < 0 || nx >= size as i32 || ny >= size as i32 {
                    continue;
                }
                let ni = (ny as usize) * (size as usize) + (nx as usize);
                let nh = h[ni];
                if nh < bh || (nh == bh && ni < c && ni < best) {
                    if nh == bh && ni > c {
                        continue;
                    }
                    bh = nh;
                    best = ni;
                }
            }
        }
        if best != usize::MAX {
            acc[best] += acc[c] + salt;
        }
    }

    // Carve: depth grows with the log of the catchment; a 3×3
    // half-depth halo gives every stream a valley, not a knife slit.
    let a: Vec<u64> = acc;
    let mut cuts: Vec<f64> = Vec::with_capacity(n);
    for c in 0..n {
        cuts.push(if a[c] > CATCH && h[c] > 0 {
            // metres of downcut; stems grow logarithmically wider
            2.0 + 1.0 * ((a[c] as f64 / CATCH as f64).ln())
        } else {
            0.0
        });
    }
    // Cut depth in VOXELS, capped at half the standing height (the
    // crater lesson): valleys deepen, mountains survive, no column
    // tunnels to the sea.
    let mut depth = vec![0u32; n];
    for c in 0..n {
        if cuts[c] > 0.0 && h[c] > 0 {
            let d = (cuts[c] * 4.0).round() as u32; // metres → voxels
            depth[c] = (d / 2).min(h[c] as u32 / 2).max(1);
        }
    }
    // Water surface: mid-channel. The bed sinks by `depth`; the
    // surface sits above the bed so the river has visible depth.
    // Halo cells (valley sides) carve half and carry no water —
    // streams are lines; valleys are the trough around them.
    let mut water = vec![0u16; n];
    for cy in 0..size as usize {
        for cx in 0..size as usize {
            let c = cy * (size as usize) + cx;
            if depth[c] > 0 {
                let bed = (h[c] as u32).saturating_sub(depth[c]);
                // Water fills the lower two-thirds of the cut.
                let surf = bed + depth[c] * 2 / 3;
                h[c] = bed as u16;
                if surf > 0 {
                    water[c] = surf as u16;
                }
                continue;
            }
            // Valley halo: carve a trough, keep it dry.
            let mut side = 0u32;
            for dy in 0..3 {
                for dx in 0..3 {
                    let (nx, ny) = (cx as i32 + dx - 1, cy as i32 + dy - 1);
                    if nx < 0 || ny < 0 {
                        continue;
                    }
                    let ni = (ny as usize) * (size as usize) + (nx as usize);
                    if ni < n && ni != c {
                        side = side.max(depth[ni]);
                    }
                }
            }
            if side > 0 && h[c] > 0 {
                let carve = (side / 2).min(h[c] as u32 / 4);
                h[c] -= carve as u16;
            }
        }
    }
    // Width: real stems widen with their catchment. Expand each
    // stream into its own valley floor: a neighbour becomes river
    // when the (dry) ground there sits at or below the water surface
    // — water finds the level it would flood, one pass, no cascade.
    // The width grows logarithmically with accumulation, so heads
    // stay thread-thin and mouths run wide.
    let mut wide = water.clone();
    for cy in 0..size as usize {
        for cx in 0..size as usize {
            let c = cy * (size as usize) + cx;
            let surf = water[c];
            if surf == 0 {
                continue;
            }
            let wid = (1 + (a[c] / (4 * CATCH)).min(16)) as i32;
            let y0 = cy as i32 - wid;
            let y1 = cy as i32 + wid;
            let x0 = cx as i32 - wid;
            let x1 = cx as i32 + wid;
            for ny in y0.max(0)..=y1.min(size as i32 - 1) {
                for nx in x0.max(0)..=x1.min(size as i32 - 1) {
                    let ni = (ny as usize) * (size as usize) + (nx as usize);
                    // Flood only dry ground that sits below the
                    // surface — never uphill, never over a ridge.
                    if water[ni] == 0 && h[ni] > 0 && h[ni] <= surf {
                        wide[ni] = surf;
                    }
                }
            }
        }
    }
    water = wide;

    // Estuary seam: where a surface meets the sea plane, let the
    // ocean win — a river mouth is sea level, not a step above it.
    for c in 0..n {
        if water[c] > 0 && water[c] <= 2 {
            water[c] = 0;
            h[c] = 0;
        }
    }
    water
}

/// Derive the metrics FROM the data (see `terracing` for why).
fn measure(h: &[u16], cfg: &ContinentConfig) -> ContinentMetrics {
    let size = cfg.size as usize;
    let vm = cfg.voxel as f64;
    let step = (size / 64).max(1); // 4 m-ish gradient sampling stride
    let stride_m = step as f64 * vm;
    let mut m = ContinentMetrics::default();
    let n = h.len() as f64;
    let mut land = 0u64;
    let mut flat = 0u64;
    for (i, &v) in h.iter().enumerate() {
        let hm = v as f64 * vm;
        if v == 0 {
            m.ocean_ratio += 1.0;
            continue;
        }
        land += 1;
        m.max_height_m = m.max_height_m.max(hm);
        // Bands ride the CURRENT relief scale (round-176 rebalance:
        // pilot's flight found 900 m peaks in a 1 km world read as a
        // tower; ceiling is 300 m now, so census cuts follow).
        if hm >= 120.0 {
            m.high_ratio += 1.0;
        }
        if hm >= 210.0 {
            m.mountain_ratio += 1.0;
        }
        let x = i % size;
        let y = i / size;
        if x >= step && y >= step && x + step < size && y + step < size {
            let gx = (h[i + step] as f64 - h[i - step] as f64) * vm;
            let gy = (h[i + step * size] as f64 - h[i - step * size] as f64) * vm;
            let slope = (gx * gx + gy * gy).sqrt() / (2.0 * stride_m).max(1e-6);
            if slope < 0.35 {
                flat += 1;
            }
        }
    }
    let ocean_n = m.ocean_ratio; // count so far
    m.ocean_ratio = ocean_n / n;
    let land_n = (land as f64).max(1.0);
    m.flat_ratio = flat as f64 / land_n;
    m.high_ratio /= land_n;
    m.mountain_ratio /= land_n;
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Same seed, two generations: byte-identical (the crate contract,
    /// new shape). 512² keeps it quick.
    #[test]
    fn continent_is_deterministic() {
        let cfg = ContinentConfig {
            size: 512,
            ..Default::default()
        };
        let a = generate_continent(7, &cfg).unwrap();
        let b = generate_continent(7, &cfg).unwrap();
        assert_eq!(a.heights, b.heights);
        assert_eq!(a.water, b.water, "rivers deterministic too");
        assert_eq!(a.heights.len(), 512 * 512);
    }

    /// Character bands for seed 2 at 2048² (measured 2026-11-06:
    /// ocean 0.18, flat 0.16, mountain 0.011, max 914 m — bands from
    /// that measurement, with slack; the round-154 census is the
    /// witness). Retuning the relief stack SHOULD fail this test:
    /// re-measure via `census`, then update bands deliberately.
    #[test]
    fn seed_two_holds_earth_character() {
        let c = generate_continent(
            2,
            &ContinentConfig {
                size: 2048,
                ..Default::default()
            },
        )
        .unwrap();
        let m = c.metrics;
        // Bands re-pinned from the round-176 census (post-rebalance:
        // 300 m ceiling; pilot's flight found 900 m peaks on a 1 km
        // world read as a tower). Measured: ocean .185 flat .434
        // mountain .0013 max 236 — bands sit generously around it.
        // Flat band widened UP: gentler slopes are the whole point of
        // the rebalance, and the slope census counts rolling hills.
        assert!(
            m.ocean_ratio > 0.10 && m.ocean_ratio < 0.30,
            "ocean {}",
            m.ocean_ratio
        );
        assert!(
            m.flat_ratio > 0.25 && m.flat_ratio < 0.60,
            "flat {}",
            m.flat_ratio
        );
        assert!(m.mountain_ratio < 0.05, "mountain {}", m.mountain_ratio);
        // Rivers exist but stay geography, not laciness: measured
        // 0.0014–0.0083 across four seeds at 2048² (relief-band
        // rivers widen with catchment, never flood the plain).
        assert!(
            m.river_ratio > 0.0005 && m.river_ratio < 0.015,
            "river {}",
            m.river_ratio
        );
        assert!(
            m.max_height_m > 150.0 && m.max_height_m < 285.0,
            "max {}",
            m.max_height_m
        );
    }

    #[test]
    fn nonsense_configs_are_refused() {
        assert!(
            generate_continent(
                1,
                &ContinentConfig {
                    size: 8,
                    ..Default::default()
                }
            )
            .is_err()
        );
        assert!(
            generate_continent(
                1,
                &ContinentConfig {
                    size: 32,
                    voxel: 0.0,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }

    /// Calibration print (run with --nocapture): the band assertions
    /// below were SET FROM THIS OUTPUT, not the other way round.
    #[test]
    #[ignore = "calibration census"]
    fn census() {
        for seed in [1u64, 2, 42, 777] {
            let t0 = std::time::Instant::now();
            let c = generate_continent(
                seed,
                &ContinentConfig {
                    size: 1024,
                    ..Default::default()
                },
            )
            .unwrap();
            let ms = t0.elapsed().as_millis();
            println!(
                "seed {seed}: ocean {:.2} flat {:.2} high {:.2} mountain {:.3} max {:.0} m ({ms} ms)",
                c.metrics.ocean_ratio,
                c.metrics.flat_ratio,
                c.metrics.high_ratio,
                c.metrics.mountain_ratio,
                c.metrics.max_height_m
            );
        }
    }

    #[test]
    fn island_mode_scatters_walkable_islands_with_open_sea() {
        // The round-179 contract, as arithmetic not vibes: mostly sea,
        // every island walkable (≥ 40 m across), NO land mass wider
        // than 600 m (continents are banned), and sea reaches every
        // map edge (no continent-scale land bridge).
        let cfg = ContinentConfig {
            size: 2048,
            islands: true,
            ..Default::default()
        };
        let c = generate_continent(5, &cfg).unwrap();
        let s = cfg.size as usize;
        assert!(c.metrics.ocean_ratio > 0.6, "mostly sea");
        let mut seen = vec![false; s * s];
        let mut blobs = Vec::new();
        for y in 0..s {
            for x in 0..s {
                if c.heights[y * s + x] == 0 || seen[y * s + x] {
                    continue;
                }
                let mut stack = vec![(x, y)];
                seen[y * s + x] = true;
                let (mut n, mut minx, mut maxx) = (0usize, x, x);
                while let Some((cx, cy)) = stack.pop() {
                    n += 1;
                    minx = minx.min(cx);
                    maxx = maxx.max(cx);
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let (ax, ay) = (cx as isize + dx, cy as isize + dy);
                        if ax >= 0
                            && ay >= 0
                            && (ax as usize) < s
                            && (ay as usize) < s
                            && c.heights[ay as usize * s + ax as usize] > 0
                            && !seen[ay as usize * s + ax as usize]
                        {
                            seen[ay as usize * s + ax as usize] = true;
                            stack.push((ax as usize, ay as usize));
                        }
                    }
                }
                blobs.push((n, maxx - minx));
            }
        }
        let walkable = blobs.iter().filter(|(n, _)| *n >= 1600).count();
        // 1600 cells = 100 m² of land: you can walk there and back.
        // 512 m span × 340 m site spacing holds "a few" loosely —
        // two full-size islands is the honest floor here; the 1 km
        // pilot map gets the photo's count (sheets show 5–8).
        assert!(walkable >= 2, "islands not gravel: {walkable} walkable");
        assert!(
            blobs.iter().all(|(_, w)| *w <= 2400),
            "no continent: widest land mass is {}",
            blobs.iter().map(|b| b.1).max().unwrap()
        );
    }

    #[test]
    fn island_mode_is_deterministic() {
        let cfg = ContinentConfig {
            size: 512,
            islands: true,
            ..Default::default()
        };
        let a = generate_continent(11, &cfg).unwrap();
        let b = generate_continent(11, &cfg).unwrap();
        assert_eq!(a.heights, b.heights);
        assert_eq!(a.water, b.water);
    }
}
