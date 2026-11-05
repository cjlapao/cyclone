//! Stage 2 — Pick archetypes (spec 02 A5.2).
//! Base Island always Flat or Hilly; ≥ 2 Plateau|Peak; ≤ 2 Atoll; rest a
//! weighted draw, with deterministic fixes if constraints break.

use rand::Rng;

use crate::map::Archetype;
use crate::pipeline::GenCtx;

fn weighted(rng: &mut impl Rng) -> Archetype {
    match rng.random_range(0..100u32) {
        0..=24 => Archetype::Flat,
        25..=54 => Archetype::Hilly,
        55..=72 => Archetype::Plateau,
        73..=87 => Archetype::Peak,
        _ => Archetype::Atoll,
    }
}

pub fn stage(ctx: &mut GenCtx) -> Result<(), crate::error::GenError> {
    let mut rng = ctx.stage_rng(2);
    for (i, site) in ctx.sites.iter_mut().enumerate() {
        site.archetype = if i == 0 {
            if rng.random_bool(0.5) {
                Archetype::Flat
            } else {
                Archetype::Hilly
            }
        } else {
            weighted(&mut rng)
        };
    }

    // Constraints over the non-base islands, in index order (deterministic).
    let n = ctx.sites.len();
    let mut atolls: Vec<usize> = (1..n)
        .filter(|&i| ctx.sites[i].archetype == Archetype::Atoll)
        .collect();
    while atolls.len() > 2 {
        let i = atolls.pop().expect("non-empty");
        ctx.sites[i].archetype = Archetype::Hilly;
    }
    let pp = (1..n)
        .filter(|&i| matches!(ctx.sites[i].archetype, Archetype::Plateau | Archetype::Peak))
        .count();
    if pp < 2 {
        // Convert the two largest remaining non-constraint islands.
        let mut candidates: Vec<(u32, usize)> = (1..n)
            .filter(|&i| !matches!(ctx.sites[i].archetype, Archetype::Plateau | Archetype::Peak))
            .map(|i| ((ctx.sites[i].radius * 100.0) as u32, i))
            .collect();
        candidates.sort_unstable_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        for (want, (_, i)) in candidates.iter().take(2 - pp).enumerate() {
            ctx.sites[*i].archetype = if want == 0 || rng.random_bool(0.5) {
                Archetype::Plateau
            } else {
                Archetype::Peak
            };
        }
    }
    Ok(())
}
