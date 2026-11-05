//! The M-screen: whole-archipelago chart with objective markers.
//! World maps 1 px = 2 tiles (96/2 = 48 px per axis, scaled ×2 into the view).

use crate::font::Font;
use crate::game::{Game, PLAY_H, PLAY_W};
use crate::render::col;
use crate::tuning::*;
use crate::world::TileKind;
use macroquad::prelude::{draw_rectangle, Color, BLACK, WHITE};

const MAP_PX: f32 = 192.0; // world (96 tiles) -> 96 px -> scaled 2× = 192 px

pub fn draw_map(font: &Font, g: &Game, t: f32) {
    let ox = (PLAY_W - MAP_PX) * 0.5;
    let oy = (PLAY_H - MAP_PX) * 0.5;
    draw_rectangle(0.0, 0.0, PLAY_W, PLAY_H, col(13));
    draw_rectangle(ox - 2.0, oy - 2.0, MAP_PX + 4.0, MAP_PX + 4.0, col(1));
    draw_rectangle(
        ox,
        oy,
        MAP_PX,
        MAP_PX,
        Color {
            a: 1.0,
            r: 0.06,
            g: 0.35,
            b: 0.55,
        },
    );

    let px = |tx: u32| ox + tx as f32 * 2.0;
    let py = |ty: u32| oy + ty as f32 * 2.0;

    // terrain, 2-tile blocks (sampled)
    for ty in (0..WORLD_TILES).step_by(2) {
        for tx in (0..WORLD_TILES).step_by(2) {
            let kind = g.world.kind(tx, ty);
            if kind == TileKind::Sea {
                continue;
            }
            let c = match kind {
                TileKind::Sand => col(3),
                TileKind::Grass => col(4),
                _ => col(6),
            };
            draw_rectangle(px(tx), py(ty), 4.0, 4.0, c);
        }
    }

    // pads
    for isl in &g.world.islands {
        if let Some((tx, ty)) = isl.helipad {
            draw_rectangle(
                px(tx) - 1.0,
                py(ty) - 1.0,
                5.0,
                5.0,
                if isl.is_base { col(11) } else { col(8) },
            );
        }
    }

    // crates (all of them — the chart is the pilot's planning tool,
    // hidden crates are only invisible in the *side view*)
    for c in &g.crates {
        let bx = px((c.x / TILE) as u32);
        let by = py((c.y / TILE) as u32);
        draw_rectangle(bx, by, 3.0, 3.0, col(15));
    }
    // refugees as small arrows
    for r in &g.refugees {
        let bx = px((r.x / TILE) as u32);
        let by = py((r.y / TILE) as u32);
        draw_rectangle(bx + 1.0, by, 1.0, 3.0, col(12));
        draw_rectangle(bx, by + 1.0, 3.0, 1.0, col(12));
    }

    // storm with heading tick
    let sxx = ox + g.storm.x / WORLD_UNITS * MAP_PX;
    let syy = oy + g.storm.y / WORLD_UNITS * MAP_PX;
    macroquad::prelude::draw_circle(sxx, syy, 5.0, col(12));
    macroquad::prelude::draw_circle_lines(sxx, syy, 8.0, 1.0, col(2));
    macroquad::prelude::draw_line(
        sxx,
        syy,
        sxx + (g.storm.target.0 - g.storm.x).signum() * 8.0,
        syy + (g.storm.target.1 - g.storm.y).signum() * 8.0,
        1.0,
        col(8),
    );

    // plane lane while scheduled/incoming/flying
    if g.planes.state != crate::planes::PlaneState::Idle {
        let lx = ox + g.planes.x / WORLD_UNITS * MAP_PX;
        let ly = oy + g.planes.y / WORLD_UNITS * MAP_PX;
        let hx = ox + (g.planes.x + g.planes.dir.0 * WORLD_UNITS) / WORLD_UNITS * MAP_PX;
        let hy = oy + (g.planes.y + g.planes.dir.1 * WORLD_UNITS) / WORLD_UNITS * MAP_PX;
        if ((t * 6.0) as u32).is_multiple_of(2) {
            macroquad::prelude::draw_line(
                lx,
                ly,
                hx.clamp(ox, ox + MAP_PX),
                hy.clamp(oy, oy + MAP_PX),
                1.0,
                col(12),
            );
        }
    }

    // helicopter blip
    let hx = ox + g.heli.x / WORLD_UNITS * MAP_PX;
    let hy = oy + g.heli.y / WORLD_UNITS * MAP_PX;
    if !((t * 3.0) as u32).is_multiple_of(3) {
        draw_rectangle(hx - 2.0, hy - 2.0, 4.0, 4.0, WHITE);
    }

    font.draw("CHART   M:CLOSE   V:FLIP VIEW", ox, oy - 14.0, 1.0, col(14));
    font.draw(
        &format!("SEED {}", g.seed),
        ox + MAP_PX - 54.0,
        oy + MAP_PX + 6.0,
        1.0,
        col(14),
    );
    let _ = BLACK;
}
