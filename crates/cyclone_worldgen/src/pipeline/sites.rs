//! Stage 1 — Place island sites (spec 02 A5.1, Bridson Poisson-disc).
//!
//! Base Island at the world centre (±8 tile jitter). Other sites grow by
//! Bridson chains: each active site proposes up to 30 candidates in the
//! annulus `[dmin, 2·dmin]` with `dmin = r_parent + r_cand + min_gap`;
//! a candidate is accepted only if `dist ≥ r_a + r_b + min_gap` for every
//! placed site. Chains keep neighbours at hugging distance, which is what
//! makes the 0.4·fuel helipad coverage of stage 6 solvable. If too few
//! sites fit, retry with all radii scaled ×0.9 (up to 3 times), then Retry.

use std::collections::VecDeque;

use rand::Rng;

use crate::error::GenError;
use crate::map::Archetype;
use crate::pipeline::common::dist;
use crate::pipeline::{GenCtx, Site};

pub fn stage(ctx: &mut GenCtx) -> Result<(), GenError> {
    let mut rng = ctx.stage_rng(1);
    let (w, h) = (ctx.w(), ctx.h());
    let margin = ctx.cfg.margin as f64;
    let gap = ctx.cfg.min_gap as f64;
    let count = ctx.cfg.island_count as usize;

    // Draw every site's radius up-front so the stream is identical between
    // shrink steps.
    let mut radii_base: Vec<f64> = Vec::with_capacity(count);
    radii_base.push(rng.random_range(ctx.cfg.base_radius.0..=ctx.cfg.base_radius.1) as f64);
    for _ in 1..count {
        radii_base.push(rng.random_range(ctx.cfg.island_radius.0..=ctx.cfg.island_radius.1) as f64);
    }

    let mut last_placed = 0usize;
    for scale_step in 0..4u32 {
        let scale = libm::pow(0.9, scale_step as f64);
        let radii: Vec<f64> = radii_base.iter().map(|&r| r * scale).collect();

        // Base Island, jittered up to ±8 tiles from the centre, kept clear
        // of the sea margin.
        let r0 = radii[0];
        let lo0 = margin + r0;
        let bx = ((w as f64 / 2.0) + rng.random_range(-8.0..=8.0)).clamp(lo0, w as f64 - 1.0 - lo0);
        let by = ((h as f64 / 2.0) + rng.random_range(-8.0..=8.0)).clamp(lo0, h as f64 - 1.0 - lo0);
        let mut sites: Vec<Site> = vec![Site {
            centre: (bx as u32, by as u32),
            radius: r0,
            archetype: Archetype::Flat, // set properly in stage 2
        }];
        let mut active: VecDeque<usize> = VecDeque::from([0usize]);

        while sites.len() < count {
            let Some(parent) = active.front().copied() else {
                break; // chains exhausted
            };
            let (px, py) = {
                let p = &sites[parent];
                (p.centre.0 as f64, p.centre.1 as f64)
            };
            let pr = sites[parent].radius;
            for _ in 0..30 {
                let r = radii[sites.len().min(count - 1)];
                let dmin = pr + r + gap;
                // Hug the acceptance boundary: narrow annulus, low-biased
                // sample, so neighbours sit coast-to-coast at ~min_gap.
                // Wide spacing would make stage-6 fuel coverage unsolvable.
                let u = rng.random::<f64>();
                let t = dmin + dmin * 0.25 * u * u;
                let ang = rng.random::<f64>() * std::f64::consts::TAU;
                let fx = px + libm::cos(ang) * t;
                let fy = py + libm::sin(ang) * t;
                let lo = margin + r;
                if fx < lo || fy < lo || fx > w as f64 - 1.0 - lo || fy > h as f64 - 1.0 - lo {
                    continue;
                }
                let ok = sites.iter().all(|s| {
                    dist(fx, fy, s.centre.0 as f64, s.centre.1 as f64) >= s.radius + r + gap
                });
                if ok {
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
            }
            active.pop_front(); // 30 tries spent (spec A5.1)
        }

        last_placed = last_placed.max(sites.len());
        if sites.len() >= count {
            ctx.sites = sites;
            return Ok(());
        }
    }
    Err(GenError::Retry(format!(
        "only {last_placed} of {count} island sites fit"
    )))
}
