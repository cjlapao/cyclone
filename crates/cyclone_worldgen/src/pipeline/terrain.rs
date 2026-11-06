//! Stage 3 — Shape terrain (spec 02 A5.3). Elevation fields per island,
//! combined with `max`, then quantised to height levels. All maths via
//! libm/integer ops per determinism rules A2.3–4.
//!
//! Calibration note: the sea cut is e ≥ 0.75, so each archetype curve is
//! tuned to cross that level at ≈0.85 of the site radius — the spec treats
//! `island_radius` as the real land radius (its own island-area checks
//! assume π r²), and the fuel geometry of A5.6 only closes when the land
//! actually reaches the radius the packing worked with.

use crate::map::Archetype;
use crate::noise::fbm;
use crate::noise::hash_pair;
use crate::pipeline::GenCtx;

/// Peak *target* elevation (level units) and detail amplitude per archetype.
fn archetype_profile(a: Archetype) -> (f64, f64) {
    match a {
        // amplitudes are ours to pick (spec leaves detail_amp open): they
        // stay low so level bands span several tiles — A5.6 needs flat
        // 3×3 blocks, and per-tile noise of ±0.7 would cliff-ify everything.
        Archetype::Flat => (2.6, 0.15),
        Archetype::Hilly => (4.0, 0.50),
        Archetype::Plateau => (5.0, 0.15),
        Archetype::Peak => (6.5, 0.55),
        Archetype::Atoll => (1.9, 0.12),
    }
}

/// Descending smoothstep: 1 at x ≤ a, 0 at x ≥ b (b may be < a).
fn sss(a: f64, b: f64, x: f64) -> f64 {
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Normalised radial profile in 0..=1, keyed by d = distance/radius.
/// Each curve hits the 0.75/target sea cut at roughly the site edge.
fn profile(a: Archetype, d: f64, target: f64) -> f64 {
    match a {
        Archetype::Flat => target * sss(1.08, 0.45, d),
        Archetype::Hilly => target * sss(1.05, 0.25, d),
        Archetype::Plateau => target * sss(0.92, 0.62, d),
        Archetype::Peak => {
            let x = (1.0 - d / 1.05).clamp(0.0, 1.0);
            target * libm::pow(x, 1.6)
        }
        Archetype::Atoll => {
            let ring = (1.0 - libm::fabs(d - 0.62) * 4.5).clamp(0.0, 1.0);
            target * ring
        }
    }
}

pub fn stage(ctx: &mut GenCtx) -> Result<(), crate::error::GenError> {
    let (w, h) = (ctx.w(), ctx.h());
    let noise_seed = hash_pair(ctx.seed.0, crate::rng::stage_stream_id(ctx.attempt, 3));
    let peak_for = |a: Archetype| -> u8 {
        match a {
            Archetype::Flat => 3,
            Archetype::Hilly => 5,
            Archetype::Plateau => 6,
            Archetype::Peak => 7,
            Archetype::Atoll => 3,
        }
        .min(ctx.cfg.max_level)
    };

    let mut elev = vec![f64::NEG_INFINITY; (w * h) as usize];
    let mut peak = vec![2u8; (w * h) as usize];
    let mut span = vec![3.0f64; (w * h) as usize]; // max elevation of the winner

    for (si, site) in ctx.sites.iter().enumerate() {
        let (target, amp0) = archetype_profile(site.archetype);
        // Per-site relief multiplier: same archetype rolls different
        // altitudes island to island (user feedback: "islands need
        // different sizes, altitude, etc"). Deterministic hash, 0.7..1.3.
        let mult = {
            let hh = crate::noise::hash_pair(
                noise_seed,
                crate::rng::stage_stream_id(ctx.attempt, 300 + si as u32),
            );
            0.7 + ((hh >> 11) % 601) as f64 / 1000.0
        };
        let amp = amp0 * mult;
        let peak_lvl = (((peak_for(site.archetype) as f64) * mult).round() as u8)
            .clamp(3, ctx.cfg.max_level.max(3));
        let r = site.radius;
        let (cx, cy) = site.centre;
        let pad = 5i64;
        let x0 = (cx as i64 - r as i64 - pad).max(0);
        let x1 = (cx as i64 + r as i64 + pad).min(w as i64 - 1);
        let y0 = (cy as i64 - r as i64 - pad).max(0);
        let y1 = (cy as i64 + r as i64 + pad).min(h as i64 - 1);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = libm::sqrt(
                    ((x as f64 + 0.5 - (cx as f64 + 0.5)) * (x as f64 + 0.5 - (cx as f64 + 0.5)))
                        + ((y as f64 + 0.5 - (cy as f64 + 0.5))
                            * (y as f64 + 0.5 - (cy as f64 + 0.5))),
                ) / r;
                // Zero-mean coastline warp so the coast keeps its radius
                // (spec A5.3 ragged coastline: fbm*0.35 at freq 0.08).
                let warp = (fbm(x as f64, y as f64, 0.08, 3, noise_seed) - 0.5) * 0.7;
                let detail = (fbm(x as f64, y as f64, 0.15, 4, noise_seed ^ 0x005D_EECE_6D1B)
                    - 0.5)
                    * 2.0
                    * amp;
                let e = profile(site.archetype, d + warp, target) + detail;
                let i = (y as u32 * w + x as u32) as usize;
                if e > elev[i] {
                    elev[i] = e;
                    peak[i] = peak_lvl;
                    span[i] = target + amp;
                }
            }
        }
    }

    // Quantise: sea / beach / linear 2..=peak (A5.3).
    for y in 0..h {
        for x in 0..w {
            let i = (y * w + x) as usize;
            let e = elev[i];
            let lvl = if !e.is_finite() || e < 0.75 {
                0
            } else if e < 1.15 {
                1
            } else {
                let p = peak[i].max(2) as f64;
                // t^0.72 bias: real archipelagos spend more time in their
                // upper terraces than a linear ramp admits — the user's
                // "almost flat" was this linear squeeze onto level 2.
                let t = (((e - 1.15) / (span[i] - 1.15).max(0.5)).clamp(0.0, 1.0)).powf(0.72);
                (2.0 + t * (p - 2.0)).round().clamp(2.0, p) as u8
            };
            *ctx.map.height.get_mut(x, y).expect("in bounds") = lvl;
        }
    }

    // Enforce the open-sea margin (A3): nothing but sea near the edge.
    let m = ctx.cfg.margin;
    for y in 0..h {
        for x in 0..w {
            if x >= m && y >= m && x + m < w && y + m < h {
                continue;
            }
            if let Some(v) = ctx.map.height.get_mut(x, y) {
                *v = 0;
            }
        }
    }
    Ok(())
}
