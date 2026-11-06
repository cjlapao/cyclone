//! Stage 1 — Place island sites (spec 02 A5.1, Bridson Poisson-disc +
//! per-seed layout styles).
//!
//! Base Island at the world centre (±8 tile jitter). Sites grow by Bridson
//! chains: each active site proposes up to 30 candidates in an annulus
//! beyond `dmin = r_parent + r_cand + min_gap`; a candidate is accepted
//! only if `dist ≥ r_a + r_b + min_gap` for every placed site.
//!
//! Spec 02 A5.2 ("clearly different archipelagos") is served by a style
//! rolled per attempt before any other draw, changing the chain geometry:
//! - **Scatter** (classic): round-robin parents, mid annulus — the
//!   original even archipelago.
//! - **Cluster**: two anchor sites first, then tight hugging chains leashed
//!   ~20 tiles from each anchor; radii biased small → 2–3 dense groups.
//! - **Chain**: the newest site is always the parent with persistent
//!   heading (±0.7 rad) → meandering island snake across the map.
//! - **Sparse**: wide annulus + large-biased radii → fewer, bigger islands
//!   with long open water (the fuel graph does the rest).
//!
//! If too few sites fit, retry with all radii scaled ×0.9 (up to 3 times),
//! then Retry (the attempt index reseeds everything, styles included).

use std::collections::VecDeque;

use rand::Rng;

use crate::error::GenError;
use crate::map::Archetype;
use crate::pipeline::common::dist;
use crate::pipeline::{GenCtx, Site};

const CLUSTER: u8 = 1;
const CHAIN: u8 = 2;
const SPARSE: u8 = 3;

pub fn stage(ctx: &mut GenCtx) -> Result<(), GenError> {
    let mut rng = ctx.stage_rng(1);
    let (w, h) = (ctx.w(), ctx.h());
    let margin = ctx.cfg.margin as f64;
    let gap = ctx.cfg.min_gap as f64;
    let count = ctx.cfg.island_count as usize;

    let style = rng.random_range(0..4u8);
    // Geometry constants are tuned for a 160-tile world; scale them with
    // the map so custom small configs stay solvable (see
    // `custom_configs_respect_bounds`).
    let s = f64::from(w.min(h)) / 160.0;

    // Draw every site's radius up-front so the stream is identical between
    // shrink steps. Styles bias WITHIN the configured range (never outside
    // it — `custom_configs_respect_bounds` guards that): cluster islands
    // small-biased (u^1.6), sparse islands large-biased (u^0.65).
    let (ilo, ihi) = (
        ctx.cfg.island_radius.0 as f64,
        ctx.cfg.island_radius.1 as f64,
    );
    // Small worlds need smaller islands or nothing fits; clamping the
    // ceiling down stays WITHIN the configured range, and defaults (160)
    // are unaffected.
    let ihi = (ihi * s).max(ilo);
    let mut radii_base: Vec<f64> = Vec::with_capacity(count);
    radii_base.push(rng.random_range(ctx.cfg.base_radius.0..=ctx.cfg.base_radius.1) as f64);
    for _ in 1..count {
        let u = rng.random::<f64>();
        let shaped = match style {
            CLUSTER => libm::pow(u, 1.6),
            SPARSE => libm::pow(u, 0.65),
            _ => u,
        };
        radii_base.push(ilo + (ihi - ilo) * shaped);
    }

    let mut last_placed = 0usize;
    for scale_step in 0..4u32 {
        let scale = libm::pow(0.9, scale_step as f64);
        let radii: Vec<f64> = radii_base.iter().map(|&r| r * scale).collect();
        let rmin = radii.iter().copied().fold(f64::INFINITY, f64::min);

        // Base Island, jittered up to ±8 tiles from the centre, kept clear
        // of the sea margin.
        let r0 = radii[0];
        let lo0 = margin + r0;
        let bx = ((w as f64 / 2.0) + rng.random_range(-8.0..=8.0)).clamp(lo0, w as f64 - 1.0 - lo0);
        let by = ((h as f64 / 2.0) + rng.random_range(-8.0..=8.0)).clamp(lo0, w as f64 - 1.0 - lo0);
        let mut sites: Vec<Site> = vec![Site {
            centre: (bx as u32, by as u32),
            radius: r0,
            archetype: Archetype::Flat, // set properly in stage 2
        }];
        let mut active: VecDeque<usize> = VecDeque::from([0usize]);

        // Shared acceptance: in bounds with margin, and clear of every
        // placed site by min_gap.
        let accept = |fx: f64, fy: f64, r: f64, sites: &Vec<Site>| -> bool {
            let lo = margin + r;
            if fx < lo || fy < lo || fx > w as f64 - 1.0 - lo || fy > h as f64 - 1.0 - lo {
                return false;
            }
            sites
                .iter()
                .all(|s| dist(fx, fy, s.centre.0 as f64, s.centre.1 as f64) >= s.radius + r + gap)
        };

        // Cluster maps seed two extra anchors around the base before the
        // chains run, so dense groups form instead of one blob.
        if style == CLUSTER {
            let r_anchor = rmin * 0.8 + 2.0; // anchor body, shrinks with radii
            for _ in 0..2 {
                for _ in 0..60 {
                    let ang = rng.random::<f64>() * std::f64::consts::TAU;
                    let d = rng.random_range(34.0 * s..=48.0 * s);
                    let fx = bx + libm::cos(ang) * d;
                    let fy = by + libm::sin(ang) * d;
                    if accept(fx, fy, r_anchor, &sites) {
                        active.push_back(sites.len());
                        sites.push(Site {
                            centre: (fx as u32, fy as u32),
                            radius: r_anchor,
                            archetype: Archetype::Flat,
                        });
                        break;
                    }
                }
            }
        }

        let mut heading: f64 = rng.random::<f64>() * std::f64::consts::TAU;
        while sites.len() < count {
            let parent = match style {
                CHAIN => active.back().copied(),
                _ => active.front().copied(),
            };
            let Some(parent) = parent else {
                break; // chains exhausted
            };
            let (px, py) = {
                let p = &sites[parent];
                (p.centre.0 as f64, p.centre.1 as f64)
            };
            let pr = sites[parent].radius;
            let before = sites.len();
            for _ in 0..30 {
                let r = radii[sites.len().min(count - 1)];
                let dmin = pr + r + gap;
                // Annulus width per style. Scatter hugs the acceptance
                // boundary (narrow, low-biased) which is what makes the
                // 0.4·fuel helipad coverage of stage 6 solvable; Cluster
                // hugs harder inside its blob; Sparse reaches far.
                let u = rng.random::<f64>();
                let t = if style == CHAIN {
                    // Near-touching pearls: absolute slack, not relative.
                    dmin + 4.0 * s * u
                } else {
                    let spread = match style {
                        CLUSTER => 0.06,
                        SPARSE => 0.9 * (0.4 + 0.6 * s),
                        _ => 0.25,
                    };
                    dmin + dmin * spread * u * u
                };
                if style == CHAIN {
                    heading += rng.random_range(-1.1..=1.1);
                }
                let ang = if style == CHAIN {
                    heading
                } else {
                    rng.random::<f64>() * std::f64::consts::TAU
                };
                let fx = px + libm::cos(ang) * t;
                let fy = py + libm::sin(ang) * t;
                if !accept(fx, fy, r, &sites) {
                    continue;
                }
                if style == CLUSTER && dist(fx, fy, px, py) > 16.0 * s + pr.max(r0) {
                    continue; // stay inside this anchor's blob
                }
                active.push_back(sites.len());
                sites.push(Site {
                    centre: (fx as u32, fy as u32),
                    radius: r,
                    archetype: Archetype::Flat,
                });
                if sites.len() >= count {
                    break;
                }
            }
            if sites.len() == before {
                // Parent spent its 30 tries without placing (spec A5.1):
                // retire it. Chain retires the snake head from the back so
                // the walk can backtrack; others round-robin from the front.
                match style {
                    CHAIN => {
                        let _ = active.pop_back();
                    }
                    _ => {
                        active.pop_front();
                    }
                }
            } else if style == CHAIN {
                while active.len() > 1 {
                    let _ = active.pop_front(); // keep only the snake head
                }
            } else {
                active.pop_front(); // placed; parent retires
            }
        }

        last_placed = last_placed.max(sites.len());
        if sites.len() >= count {
            ctx.sites = sites;
            return Ok(());
        }
    }
    Err(GenError::Retry(format!(
        "only {last_placed} of {count} island sites fit (style {style})"
    )))
}
