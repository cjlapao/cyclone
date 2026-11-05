//! Right-hand instrument panel + bottom status bar (drawn over the frame).
//! Pure drawing: reads `Game`, mutates nothing.

use crate::font::Font;
use crate::game::{Game, Phase, PLAY_W};
use crate::iso::flip_xy;
use crate::planes::PlaneState;
use crate::render::col;
use crate::tuning::*;
use macroquad::prelude::{draw_rectangle, Color};

fn fmt_time(t: f32) -> String {
    let t = t.max(0.0) as u32;
    format!("{}:{:02}", t / 60, t % 60)
}

pub fn draw_panel(font: &Font, g: &Game, t: f32) {
    let x = SCREEN_W - PANEL_W + 4.0;

    font.draw("SCORE", x, 4.0, 1.0, col(14));
    font.draw(&format!("{:06}", g.score), x, 12.0, 2.0, col(11));

    // fuel gauge (vertical bar)
    font.draw("FUEL", x, 30.0, 1.0, col(14));
    let fw = 12.0;
    draw_rectangle(x, 38.0, fw, 32.0, col(13).darken_mul());
    let h = (g.heli.fuel * 30.0).round();
    let fcol = if g.heli.fuel < 0.25 { col(12) } else { col(4) };
    draw_rectangle(x + 1.0, 38.0 + (31.0 - h), fw - 2.0, h, fcol);

    // alt / spd read-outs
    font.draw("ALT", x + 20.0, 30.0, 1.0, col(14));
    font.draw(&format!("{}", g.heli.z as u32), x + 20.0, 38.0, 2.0, col(8));
    font.draw("SPD", x + 20.0, 56.0, 1.0, col(14));
    font.draw(
        &format!("{}", g.speed() as u32),
        x + 20.0,
        64.0,
        2.0,
        col(8),
    );

    // cargo manifest
    font.draw("CRATES", x, 74.0, 1.0, col(14));
    font.draw(
        &format!("{}/5", g.carried_crates),
        x + 40.0,
        74.0,
        1.0,
        col(15),
    );
    font.draw("CREW", x, 84.0, 1.0, col(14));
    font.draw(&format!("{}", g.carried_refs), x + 40.0, 84.0, 1.0, col(15));
    font.draw(
        &format!("GOT {}/5", g.delivered_crates),
        x,
        94.0,
        1.0,
        if g.delivered_crates >= NUM_CRATES {
            col(4)
        } else {
            col(14)
        },
    );

    // wind compass: direction to the cyclone (screen-relative)
    font.draw("WIND", x, 108.0, 1.0, col(14));
    let cx = x + 14.0;
    let cy = 134.0;
    let (hx, hy) = flip_xy(g.heli.x, g.heli.y, g.view);
    let (sx, sy) = flip_xy(g.storm.x, g.storm.y, g.view);
    let (dx, dy) = (sx - hx, sy - hy);
    let (svx, svy) = (dx - dy, (dx + dy) * 0.5);
    let l = (svx * svx + svy * svy).sqrt().max(1.0);
    let dist = g.storm.dist_to(g.heli.x, g.heli.y) / TILE;
    for r in [5.0, 9.0] {
        draw_rectangle(cx - r, cy - r * 0.5 - 0.5, r * 2.0, 1.0, col(13));
        draw_rectangle(
            cx - r * 0.5 - 0.5,
            cy - r * 0.5 - r * 0.25,
            1.0,
            r * 0.75,
            col(13),
        );
    }
    macroquad::prelude::draw_line(cx, cy, cx + svx / l * 9.0, cy + svy / l * 5.0, 2.0, col(12));
    font.draw(&format!("{:.0}T", dist), x + 32.0, 126.0, 1.0, col(14));

    // plane danger light
    let danger = g.planes.state != PlaneState::Idle;
    if !danger || ((t * 6.0) as u32).is_multiple_of(2) {
        font.draw(
            "PLANE",
            x,
            150.0,
            1.0,
            if danger { col(12) } else { col(14) },
        );
    }

    // lives as little rotor-icons
    font.draw("HELI", x, 162.0, 1.0, col(14));
    for i in 0..g.lives.min(5) {
        let lx = x + (i as f32) * 10.0;
        draw_rectangle(lx, 172.0, 6.0, 3.0, col(10));
        draw_rectangle(lx - 1.0, 170.0, 8.0, 1.0, col(0));
    }

    font.draw("V:FLIP  M:MAP", x, 196.0, 1.0, col(14));
}

pub fn draw_bar(font: &Font, g: &Game) {
    let y = SCREEN_H - BAR_H + 4.0;
    let msg = match g.phase {
        Phase::Refuelling => "REFUELLING...".to_string(),
        Phase::Landed if g.heli.fuel >= 1.0 => "PARKED - LAND BONUS +10/S".to_string(),
        Phase::Landed => "GROUNDED".to_string(),
        Phase::Crashed => "MAYDAY!".to_string(),
        Phase::Flying => {
            if g.winch_target.is_some() {
                "WINCHING...".to_string()
            } else if g.carried_crates + g.carried_refs > 0 {
                "RETURN TO BASE PAD".to_string()
            } else {
                "SEARCH ISLANDS - E TO WINCH LOW".to_string()
            }
        }
    };
    font.draw(&msg, 6.0, y, 1.0, col(11));
    font.draw(
        &format!("TIME {}", fmt_time(g.time_left)),
        6.0,
        y + 12.0,
        2.0,
        col(8),
    );
    font.draw(
        &format!("ROUND {}", g.round),
        SCREEN_W - PANEL_W - 60.0,
        y,
        1.0,
        col(14),
    );
    font.draw(
        &format!(
            "VIEW {}",
            if g.view == crate::iso::View::South {
                "S"
            } else {
                "N"
            }
        ),
        SCREEN_W - PANEL_W - 60.0,
        y + 10.0,
        1.0,
        col(14),
    );
}

// local shade helper (macroquad Color has none)
trait DarkenMul {
    fn darken_mul(self) -> Color;
}
impl DarkenMul for Color {
    fn darken_mul(self) -> Color {
        Color {
            r: self.r * 0.6,
            g: self.g * 0.6,
            b: self.b * 0.6,
            a: self.a,
        }
    }
}

/// Centre-aligned big message used by map/overlays (kept here for font reuse).
pub fn big_text(font: &Font, text: &str, y: f32, scale: f32, color: Color) {
    let wpx = font.width(text, scale);
    font.draw(text, (PLAY_W - wpx) * 0.5, y, scale, color);
}
