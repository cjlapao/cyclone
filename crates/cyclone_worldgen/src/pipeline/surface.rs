//! Stage 5 — Surfaces and cliffs (spec 02 A5.5).

use crate::error::GenError;
use crate::map::{CLIFF_E, CLIFF_N, CLIFF_S, CLIFF_W, Surface};
use crate::pipeline::GenCtx;

const NB: [(i64, i64, u8); 4] = [
    (0, -1, CLIFF_N),
    (1, 0, CLIFF_E),
    (0, 1, CLIFF_S),
    (-1, 0, CLIFF_W),
];

pub fn stage(ctx: &mut GenCtx) -> Result<(), GenError> {
    let (w, h) = (ctx.w(), ctx.h());
    for y in 0..h {
        for x in 0..w {
            let h0 = ctx.map.height_at(x, y);
            let sd = ctx.map.shore_dist.raw(x, y);
            let surface = match h0 {
                0 if sd > 2 => Surface::Sea,
                0 => Surface::Shallow,
                1 => Surface::Sand,
                // Sand bleeds a few tiles uphill in PATCHES (user
                // feedback: "more sand on the borders, does not always
                // need to go around"): hash speckle keeps some level-2
                // coast tiles sandy, others grass — natural, ragged.
                2 if sd <= 4
                    && !crate::noise::hash_pair(
                        crate::noise::hash_pair(x as u64, y as u64) ^ ctx.map.seed.0,
                        0x5A50_4453,
                    )
                    .is_multiple_of(3) =>
                {
                    Surface::Sand
                }
                2..=5 => Surface::Grass,
                _ => Surface::Rock,
            };
            let mut cliff = 0u8;
            if h0 >= 2 {
                for (dx, dy, bit) in NB {
                    let nx = x as i64 + dx;
                    let ny = y as i64 + dy;
                    let nh = if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        0
                    } else {
                        ctx.map.height_at(nx as u32, ny as u32)
                    };
                    if h0.saturating_sub(nh) >= 2 {
                        cliff |= bit;
                    }
                }
            }
            *ctx.map.surface.get_mut(x, y).expect("in bounds") = surface;
            *ctx.map.cliff.get_mut(x, y).expect("in bounds") = cliff;
        }
    }
    Ok(())
}
