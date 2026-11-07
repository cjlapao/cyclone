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
use crate::noise::fbm;

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
}

impl Default for ContinentConfig {
    fn default() -> Self {
        Self {
            size: 4000,
            voxel: 0.25,
            max_height_m: 300.0,
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
    /// Highest column in meters.
    pub max_height_m: f64,
}

/// One generated world: integer voxel heights, row-major, `y * size + x`.
pub struct Continent {
    pub config: ContinentConfig,
    pub seed: u64,
    /// Voxel-unit column heights (0 = sea level).
    pub heights: Vec<u16>,
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

/// Column elevation in meters. Pure function — any column, any time.
/// Layered Earth logic: a continental mask carves oceans; a separate
/// orogeny mask concentrates mountains into ranges; gentle plains fill
/// everything else; ridged noise gives peaks with real valley floors.
pub fn elevation_m(x_m: f64, y_m: f64, seed: u64, cfg: &ContinentConfig) -> f64 {
    // Domain warp: bends every boundary so nothing looks grid-aligned.
    let wx = x_m + 150.0 * (fbm(x_m / 350.0, y_m / 350.0, 1.0, 2, seed ^ 0xD0A1) - 0.5) * 2.0;
    let wy = y_m + 150.0 * (fbm(x_m / 350.0, y_m / 350.0, 1.0, 2, seed ^ 0xA41E) - 0.5) * 2.0;

    // Continents vs oceans.
    let c = fbm(wx / 500.0, wy / 500.0, 1.0, 5, seed);
    let land = smoothstep(0.44, 0.54, c); // 0 = open ocean

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
    (land * (base + detail)).max(0.0)
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
    let metrics = measure(&heights, cfg);
    Ok(Continent {
        config: *cfg,
        seed,
        heights,
        metrics,
    })
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
}
