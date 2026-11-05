//! Scene/HUD drawing. Render layer: the only place (with main/hud/map/audio)
//! allowed to touch the graphics API. All pixel art here is hand-drawn data.

use crate::font::Font;
use crate::state::GameState;
use crate::tuning::*;
use macroquad::prelude::*;

// ---------------------------------------------------------------- palette
// ~15-colour hand-picked palette, bright 8-bit feel, modern and clean.
pub const PAL: [Color; 16] = [
    color_u8!(0x18, 0x18, 0x28, 0xff), // 0 OUTLINE near-black
    color_u8!(0x1f, 0xb2, 0xe6, 0xff), // 1 SEA bright cyan
    color_u8!(0x7f, 0xdc, 0xff, 0xff), // 2 SEA_LIGHT shimmer
    color_u8!(0xf8, 0xdc, 0x78, 0xff), // 3 SAND
    color_u8!(0x3c, 0xb4, 0x50, 0xff), // 4 GRASS
    color_u8!(0x20, 0x8c, 0x34, 0xff), // 5 GRASS DARK
    color_u8!(0xa0, 0x78, 0x50, 0xff), // 6 CLIFF/rock
    color_u8!(0x78, 0x58, 0x38, 0xff), // 7 CLIFF shade
    color_u8!(0xec, 0xec, 0xec, 0xff), // 8 BUILDING white
    color_u8!(0xa8, 0xa8, 0xb4, 0xff), // 9 BUILDING shade
    color_u8!(0xd8, 0x3c, 0x32, 0xff), // 10 ROOF / heli red
    color_u8!(0xf8, 0xc8, 0x3c, 0xff), // 11 ACCENT yellow
    color_u8!(0xe8, 0x40, 0x30, 0xff), // 12 WARNING red
    color_u8!(0x1c, 0x1c, 0x30, 0xff), // 13 HUD background
    color_u8!(0x64, 0x6c, 0x88, 0xff), // 14 HUD dim
    color_u8!(0xc8, 0x88, 0x40, 0xff), // 15 CRATE wood
];

pub fn col(i: usize) -> Color {
    PAL[i]
}

/// Presentation bundle created once at startup.
pub struct Gfx {
    pub font: Font,
    pub target: RenderTarget,
}

impl Gfx {
    pub fn new() -> Self {
        let target = render_target(SCREEN_W as u32, SCREEN_H as u32);
        target.texture.set_filter(FilterMode::Nearest);
        Self {
            font: Font::new(),
            target,
        }
    }

    pub fn begin_scene(&self) {
        set_camera(&Camera2D {
            zoom: vec2(2.0 / SCREEN_W, 2.0 / SCREEN_H),
            target: vec2(SCREEN_W / 2.0, SCREEN_H / 2.0),
            render_target: Some(self.target.clone()),
            ..Default::default()
        });
    }

    /// Letterbox-present the 320×240 target at the largest integer scale.
    pub fn present(&self) {
        set_default_camera();
        clear_background(col(0));
        let (ww, wh) = (screen_width(), screen_height());
        let scale = (
            (ww / SCREEN_W).floor().max(1.0),
            (wh / SCREEN_H).floor().max(1.0),
        );
        let s = scale.0.min(scale.1);
        let dw = SCREEN_W * s;
        let dh = SCREEN_H * s;
        let x = (ww - dw) * 0.5;
        let y = (wh - dh) * 0.5;
        draw_texture_ex(
            &self.target.texture,
            x,
            y,
            WHITE,
            DrawTextureParams {
                dest_size: Some(vec2(dw, dh)),
                ..Default::default()
            },
        );
    }

    /// Draw the HUD border frame shared by play/map/pause screens.
    pub fn draw_frame(&self) {
        let p = col(13);
        let d = col(14);
        // Right panel and bottom bar.
        draw_rectangle(SCREEN_W - PANEL_W, 0.0, PANEL_W, SCREEN_H, p);
        draw_rectangle(0.0, SCREEN_H - BAR_H, SCREEN_W - PANEL_W, BAR_H, p);
        draw_line(
            SCREEN_W - PANEL_W - 1.0,
            0.0,
            SCREEN_W - PANEL_W - 1.0,
            SCREEN_H,
            1.0,
            d,
        );
        draw_line(
            0.0,
            SCREEN_H - BAR_H - 1.0,
            SCREEN_W - PANEL_W,
            SCREEN_H - BAR_H - 1.0,
            1.0,
            d,
        );
    }

    pub fn draw_title(&self, blink: bool, high: &[(String, u32)]) {
        clear_background(col(1));
        self.font
            .draw_centered(crate::tuning::GAME_NAME, SCREEN_W * 0.5, 28.0, 3.0, col(8));
        self.font.draw_centered(
            "RESCUE HELICOPTER OF THE ISLES",
            SCREEN_W * 0.5,
            58.0,
            1.0,
            col(11),
        );
        let mut y = 86.0;
        for (name, score) in high.iter().take(5) {
            self.font
                .draw_centered(&format!("{name} {score}"), SCREEN_W * 0.5, y, 1.0, col(9));
            y += 10.0;
        }
        if blink {
            self.font
                .draw_centered("PRESS ANY KEY", SCREEN_W * 0.5, 196.0, 2.0, col(8));
        }
        self.font.draw_centered(
            "ARROWS/SPACE/SHIFT  V FLIP  M MAP  P PAUSE",
            SCREEN_W * 0.5,
            226.0,
            1.0,
            col(9),
        );
    }

    pub fn draw_placeholder_play(&self) {
        clear_background(col(1));
        self.font
            .draw_centered("WORLD COMING ONLINE...", SCREEN_W * 0.5, 110.0, 1.0, col(0));
    }

    pub fn draw_paused(&self) {
        draw_rectangle(60.0, 90.0, 128.0, 40.0, col(13));
        self.font
            .draw_centered("PAUSED", 92.0 + 32.0, 100.0, 2.0, col(8));
        self.font
            .draw_centered("P RESUME  ESC QUIT", 92.0 + 32.0, 118.0, 1.0, col(9));
    }

    pub fn draw_state_hint(&self, st: GameState) {
        if st == GameState::Playing {
            return;
        }
        self.font.draw(&format!("{:?}", st), 4.0, 4.0, 1.0, col(9));
    }
}

// ============================================================ M2 world draw
use crate::game::{Game, Phase, PLAY_H, PLAY_W};
use crate::iso::{depth, flip_xy, project};
use crate::world::{ObjKind, TileKind};

// extra face/edge colours beyond the 16-slot palette
const SAND_DARK: Color = color_u8!(0xc8, 0xa4, 0x48, 0xff);
const GRASS_SHADE: Color = color_u8!(0x18, 0x6c, 0x28, 0xff);
const CLIFF_FACE: Color = color_u8!(0x8a, 0x64, 0x38, 0xff);

fn ln(a: Vec2, b: Vec2, w: f32, col: Color) {
    draw_line(a.x, a.y, b.x, b.y, w, col);
}
/// deterministic 12 % shade for side faces (macroquad has no darken())
fn dk(c: Color) -> Color {
    Color {
        r: c.r * 0.88,
        g: c.g * 0.88,
        b: c.b * 0.88,
        a: c.a,
    }
}
fn tri(a: Vec2, b: Vec2, c: Vec2, col: Color) {
    draw_triangle(a, b, c, col);
}
fn quad(a: Vec2, b: Vec2, c: Vec2, d: Vec2, col: Color) {
    tri(a, b, c, col);
    tri(a, c, d, col);
}

/// Screen-space diamond top at tile centre P (iso 32×16).
fn diamond(p: Vec2, hw: f32, hh: f32, col: Color) {
    quad(
        vec2(p.x, p.y - hh),
        vec2(p.x + hw, p.y),
        vec2(p.x, p.y + hh),
        vec2(p.x - hw, p.y),
        col,
    );
}

pub fn draw_scene(g: &Game, t: f32) {
    // brief camera shake on crash (visual only; sim untouched)
    let shake = if g.phase == crate::game::Phase::Crashed {
        (g.crash_t / CRASH_PAUSE).min(1.0)
    } else {
        0.0
    };
    let view = g.view;
    let (cf0, cf1) = flip_xy(g.cam.0, g.cam.1, view);
    let pcx = PLAY_W * 0.5 + shake * (t * 79.0).sin() * 2.5;
    let pcy = PLAY_H * 0.5 + shake * (t * 97.0).cos() * 2.0;
    clear_background(col(1));

    let wtiles = WORLD_TILES as i32;
    let sgn = if view == crate::iso::View::South {
        -1i32
    } else {
        1
    };
    // flipped tile coord <-> world tile coord
    let w2f = |t: i32| sgn * t + if sgn < 0 { wtiles - 1 } else { 0 };
    let f2w = w2f; // involution
    let fh = |ftx: i32, fty: i32| -> u8 {
        if ftx < 0 || fty < 0 || ftx >= wtiles || fty >= wtiles {
            0
        } else {
            g.world.tile_h(f2w(ftx) as u32, f2w(fty) as u32)
        }
    };

    let cctx = (cf0 / TILE) as i32;
    let ccty = (cf1 / TILE) as i32;
    let r_tiles = ((PLAY_W + PLAY_H) / TILE) as i32 + 4;

    // sea background wash + shimmer happens under tiles (tiles only cover land)
    let frame = (t * 2.5) as u32;
    for fty in (ccty - r_tiles)..=(ccty + r_tiles) {
        for ftx in (cctx - r_tiles)..=(cctx + r_tiles) {
            if fh(ftx, fty) != 0 {
                continue;
            }
            let (rx, ry) = (
                (ftx as f32 + 0.5) * TILE - cf0,
                (fty as f32 + 0.5) * TILE - cf1,
            );
            let sx = pcx + rx - ry;
            let sy = pcy + (rx + ry) * 0.5;
            if sx < -20.0 || sx > PLAY_W + 20.0 || sy < -20.0 || sy > PLAY_H + 20.0 {
                continue;
            }
            let wtx = f2w(ftx);
            let wty = f2w(fty);
            if (crate::world::noise2(wtx, wty, 99).mul_add(1_000_000.0, 0.0) as u32 + frame)
                .is_multiple_of(31)
            {
                draw_line(sx - 3.0, sy, sx + 2.0, sy, 1.0, col(2));
            }
        }
    }

    // depth-sorted land tiles, objects, crates, refugees
    let mut items: Vec<(i32, u8, u32)> = Vec::with_capacity(2500);
    for fty in (ccty - r_tiles)..=(ccty + r_tiles) {
        for ftx in (cctx - r_tiles)..=(cctx + r_tiles) {
            if fh(ftx, fty) == 0 {
                continue;
            }
            items.push((ftx + fty, 0, (fty << 16) as u32 | ftx as u32));
        }
    }
    let push_far = |items: &mut Vec<(i32, u8, u32)>, x: f32, y: f32, sub: u8, id: u32| {
        let (fx, fy) = flip_xy(x, y, view);
        if (fx - cf0).abs() + (fy - cf1).abs() < (r_tiles as f32 + 3.0) * TILE {
            items.push((((fx + fy) / TILE).floor() as i32, sub, id));
        }
    };
    for (i, o) in g.world.objects.iter().enumerate() {
        push_far(&mut items, o.x, o.y, 1, i as u32);
    }
    for (i, c) in g.crates.iter().enumerate() {
        if c.hidden_by == Some(view) {
            continue;
        }
        push_far(&mut items, c.x, c.y, 2, i as u32);
    }
    for (i, rf) in g.refugees.iter().enumerate() {
        push_far(&mut items, rf.x, rf.y, 2, i as u32);
    }
    items.sort_unstable();

    for (d, sub, id) in items {
        match sub {
            0 => {
                let ftx = (id << 16) as i16 as i32;
                let fty = (id >> 16) as i32;
                let _ = d;
                let h = fh(ftx, fty);
                let (rx, ry) = (
                    (ftx as f32 + 0.5) * TILE - cf0,
                    (fty as f32 + 0.5) * TILE - cf1,
                );
                let p = vec2(pcx + rx - ry, pcy + (rx + ry) * 0.5 - h as f32 * HEIGHT_PX);
                if p.x < -20.0 || p.x > PLAY_W + 20.0 || p.y < -60.0 || p.y > PLAY_H + 40.0 {
                    continue;
                }
                let wtx = f2w(ftx) as u32;
                let wty = f2w(fty) as u32;
                let kind = g.world.kind(wtx, wty);
                let (n_l, n_r) = (fh(ftx, fty + 1), fh(ftx + 1, fty));
                // side faces down to lower neighbours
                if n_l < h {
                    let drop = (h - n_l) as f32 * HEIGHT_PX;
                    let fc = match kind {
                        TileKind::Sand => SAND_DARK,
                        TileKind::Grass => GRASS_SHADE,
                        _ => CLIFF_FACE,
                    };
                    quad(
                        vec2(p.x - 16.0, p.y),
                        vec2(p.x, p.y + 8.0),
                        vec2(p.x, p.y + 8.0 + drop),
                        vec2(p.x - 16.0, p.y + drop),
                        fc,
                    );
                    if n_l == 0 {
                        ln(
                            vec2(p.x - 16.0, p.y),
                            vec2(p.x, p.y + 8.0),
                            1.0,
                            Color { a: 0.5, ..col(8) },
                        );
                    }
                }
                if n_r < h {
                    let drop = (h - n_r) as f32 * HEIGHT_PX;
                    let fc = match kind {
                        TileKind::Sand => dk(SAND_DARK),
                        TileKind::Grass => dk(GRASS_SHADE),
                        _ => dk(CLIFF_FACE),
                    };
                    quad(
                        vec2(p.x, p.y + 8.0),
                        vec2(p.x + 16.0, p.y),
                        vec2(p.x + 16.0, p.y + drop),
                        vec2(p.x, p.y + 8.0 + drop),
                        fc,
                    );
                }
                let top = match kind {
                    TileKind::Sand => col(3),
                    TileKind::Grass => {
                        if (crate::world::noise2(wtx as i32, wty as i32, 5) * 7.0) as u32 % 7 == 3 {
                            col(5)
                        } else {
                            col(4)
                        }
                    }
                    _ => col(6),
                };
                diamond(p, 16.0, 8.0, top);
                if g.world.on_helipad_tile(wtx, wty) {
                    diamond(p, 13.0, 6.5, col(9));
                    let x0 = (p.x - 4.0).round();
                    let y0 = (p.y - 4.0).round();
                    draw_rectangle(x0, y0, 2.0, 8.0, col(0));
                    draw_rectangle(x0 + 6.0, y0, 2.0, 8.0, col(0));
                    draw_rectangle(x0, y0 + 3.0, 8.0, 2.0, col(0));
                }
            }
            1 => {
                let o = &g.world.objects[id as usize];
                let (sx, sy) = project(o.x, o.y, o.top(), g.cam, view);
                draw_obj(vec2(pcx + sx, pcy + sy), o, t);
            }
            2 => {
                if id as usize >= g.crates.len() {
                    let rf = &g.refugees[id as usize - g.crates.len()];
                    let (sx, sy) =
                        project(rf.x, rf.y, g.world.h_at(rf.x, rf.y) as f32, g.cam, view);
                    let p = vec2(pcx + sx, pcy + sy);
                    draw_rectangle(p.x - 1.0, p.y - 6.0, 2.0, 2.0, col(3)); // head
                    draw_rectangle(p.x - 1.0, p.y - 4.0, 2.0, 4.0, col(12)); // body
                    let wob = ((t * 5.0) as i32 % 2) as f32;
                    draw_rectangle(p.x + 1.0, p.y - 5.0 - wob, 1.0, 2.0, col(12));
                // waving arm
                } else {
                    let c = &g.crates[id as usize];
                    let gh = g.world.h_at(c.x, c.y) as f32;
                    let (sx, sy) = project(c.x, c.y, gh, g.cam, view);
                    let p = vec2(pcx + sx, pcy + sy - 3.0);
                    draw_box(p, 5.0, 5.0, col(15), col(11), dk(CLIFF_FACE));
                    ln(
                        vec2(p.x - 3.0, p.y - 1.0),
                        vec2(p.x + 3.0, p.y - 1.0),
                        1.0,
                        col(11),
                    );
                    ln(vec2(p.x, p.y - 4.0), vec2(p.x, p.y + 2.0), 1.0, col(11));
                }
            }
            _ => {}
        }
    }

    // cyclone: translucent disc + iso spiral arms
    {
        let (sx, sy) = project(g.storm.x, g.storm.y, 0.0, g.cam, view);
        let p = vec2(pcx + sx, pcy + sy);
        let r_px = CYCLONE_RADIUS * 0.9;
        draw_ellipse(
            p.x,
            p.y,
            r_px * 1.414,
            r_px * 0.707,
            0.0,
            Color {
                a: 0.22,
                r: 0.12,
                g: 0.16,
                b: 0.35,
            },
        );
        let steps = 52;
        for i in 0..steps {
            let f = i as f32 / steps as f32;
            let arm = (g.storm.phase * 1.6 + f * 7.5) % (std::f32::consts::TAU);
            for k in 0..3 {
                let a = arm + k as f32 * std::f32::consts::TAU / 3.0;
                let r = CYCLONE_INNER + (CYCLONE_RADIUS - CYCLONE_INNER) * f;
                let (wx, wy) = (g.storm.x + a.cos() * r, g.storm.y + a.sin() * r);
                let (ax, ay) = project(wx, wy, 1.0, g.cam, view);
                let shade = 0.35 + 0.4 * (1.0 - f);
                draw_rectangle(
                    pcx + ax,
                    pcy + ay,
                    1.0,
                    1.0,
                    Color {
                        r: 0.75 * shade,
                        g: 0.8 * shade,
                        b: 0.95,
                        a: 0.9,
                    },
                );
            }
        }
        draw_ellipse(
            p.x,
            p.y,
            CYCLONE_INNER * 2.4,
            CYCLONE_INNER * 1.2,
            0.0,
            Color {
                a: 0.55,
                r: 0.05,
                g: 0.06,
                b: 0.15,
            },
        );
    }

    // plane: warning lane, then the aircraft and its sea shadow
    match g.planes.state {
        crate::planes::PlaneState::Warning => {
            if !((t * 6.0) as u32).is_multiple_of(2) {
                let mut d = 0.0;
                while d < WORLD_UNITS {
                    let (wx, wy) = (
                        g.planes.x + g.planes.dir.0 * d,
                        g.planes.y + g.planes.dir.1 * d,
                    );
                    let (sx, sy) = project(wx, wy, 0.0, g.cam, view);
                    if sx > -8.0 && sx < PLAY_W + 8.0 && sy > -8.0 && sy < PLAY_H + 8.0 {
                        draw_rectangle(pcx + sx, pcy + sy, 2.0, 1.0, col(12));
                    }
                    d += 3.0 * TILE;
                }
                gfx_hud_warn(t);
            }
        }
        crate::planes::PlaneState::Flying => {
            let (sx0, sy0) = project(g.planes.x, g.planes.y, 0.0, g.cam, view);
            diamond(
                vec2(pcx + sx0, pcy + sy0),
                7.0,
                3.5,
                Color {
                    a: 0.4,
                    r: 0.0,
                    g: 0.0,
                    b: 0.15,
                },
            );
            let (sx, sy) = project(g.planes.x, g.planes.y, g.planes.alt, g.cam, view);
            let p = vec2(pcx + sx, pcy + sy);
            let (dx, dy) = project(
                g.planes.x + g.planes.dir.0 * TILE,
                g.planes.y + g.planes.dir.1 * TILE,
                g.planes.alt,
                g.cam,
                view,
            );
            let nose = vec2(dx - sx, dy - sy);
            let l = (nose.x * nose.x + nose.y * nose.y).sqrt().max(1.0);
            let nx = nose.x / l;
            let ny = nose.y / l;
            ln(vec2(p.x - nx * 8.0, p.y - ny * 8.0), p, 2.0, col(0));
            ln(
                vec2(p.x - nx * 3.0 - ny * 4.0, p.y - ny * 3.0 + nx * 2.0),
                vec2(p.x - nx * 3.0 + ny * 4.0, p.y - ny * 3.0 - nx * 2.0),
                1.0,
                col(9),
            );
            draw_rectangle(
                p.x + nx * 6.0 - 1.0,
                p.y + ny * 6.0 - 1.0,
                2.0,
                2.0,
                col(12),
            );
        }
        crate::planes::PlaneState::Idle => {}
    }

    // helicopter shadow + airframe (spec: above tiles, occluded-outline next round)
    let gh = g.world.h_at(g.heli.x, g.heli.y) as f32;
    let (sx, sy) = project(g.heli.x, g.heli.y, gh, g.cam, view);
    if !g.heli.landed {
        diamond(
            vec2(pcx + sx, pcy + sy),
            6.0,
            3.0,
            Color {
                a: 0.45,
                r: 0.0,
                g: 0.0,
                b: 0.15,
            },
        );
    }
    let (hx, hy) = project(g.heli.x, g.heli.y, g.heli.z, g.cam, view);
    draw_heli(
        vec2(pcx + hx, pcy + hy),
        g.heli.rotor,
        g.phase == Phase::Crashed,
        g.heli.dead,
    );

    // crash explosion
    if g.phase == Phase::Crashed {
        let r = 6.0 + (CRASH_PAUSE - g.crash_t) * 14.0;
        draw_circle(
            pcx + hx,
            pcy + hy - 4.0,
            r,
            Color {
                a: 0.8,
                r: 1.0,
                g: 0.7,
                b: 0.2,
            },
        );
        draw_circle(
            pcx + hx,
            pcy + hy - 4.0,
            r * 0.6,
            Color { a: 0.9, ..col(12) },
        );
    }

    // view-flip fade
    let f = g.flip_fade();
    if f > 0.0 {
        draw_rectangle(
            0.0,
            0.0,
            PLAY_W,
            PLAY_H,
            Color {
                a: f * 0.9,
                ..BLACK
            },
        );
    }
}

fn draw_box(p: Vec2, hw: f32, hh: f32, top: Color, front: Color, side: Color) {
    // iso box centred at p (top-face centre), height implied by hh/hw
    quad(
        vec2(p.x, p.y - hh),
        vec2(p.x + hw, p.y),
        vec2(p.x, p.y + hh),
        vec2(p.x - hw, p.y),
        top,
    );
    let h = hh * 2.0;
    quad(
        vec2(p.x - hw, p.y),
        vec2(p.x, p.y + hh),
        vec2(p.x, p.y + hh + h),
        vec2(p.x - hw, p.y + h),
        front,
    );
    quad(
        vec2(p.x, p.y + hh),
        vec2(p.x + hw, p.y),
        vec2(p.x + hw, p.y + h),
        vec2(p.x, p.y + hh + h),
        side,
    );
}

fn gfx_hud_warn(_t: f32) {
    // "PLANE INBOUND" — full HUD copy arrives in M6; blink marker now
    let blink = ((_t * 6.0) as u32).is_multiple_of(2);
    if blink {
        draw_rectangle(4.0, 4.0, 3.0, 3.0, col(12));
    }
}

fn draw_obj(p: Vec2, o: &crate::world::Obj, _t: f32) {
    match o.kind {
        ObjKind::Tree => {
            let base = vec2(p.x, p.y + o.kind.height_units() as f32 * HEIGHT_PX);
            draw_rectangle(base.x - 1.0, base.y - 7.0, 2.0, 7.0, col(7));
            diamond(vec2(p.x, p.y - 1.0), 7.0, 5.0, col(5));
            diamond(vec2(p.x, p.y - 4.0), 5.0, 3.5, col(4));
        }
        ObjKind::Hut => {
            draw_box(p, 9.0, 4.5, col(10), col(8), col(9));
        }
        ObjKind::Tower => {
            draw_box(p, 7.0, 3.5, col(8), col(8), col(9));
            draw_rectangle(p.x - 1.0, p.y - 8.0, 2.0, 6.0, col(9));
        }
        ObjKind::Lighthouse => {
            draw_box(p, 5.0, 2.5, col(8), col(8), col(9));
            draw_rectangle(p.x - 2.0, p.y - 3.0, 4.0, 3.0, col(12)); // red cap
            let blink = ((_t * 2.0) as u32).is_multiple_of(2);
            if blink {
                draw_circle(p.x, p.y - 5.0, 2.0, col(11));
            }
        }
    }
}

/// Side-view rescue helicopter, ~20×12 px, original design.
fn draw_heli(p: Vec2, rotor: f32, crashed: bool, dead: bool) {
    let p = p.round();
    let x = p.x - 10.0;
    let y = p.y - 10.0;
    if crashed {
        return;
    }
    // skids
    draw_rectangle(x + 3.0, y + 10.0, 11.0, 1.0, col(0));
    draw_rectangle(x + 5.0, y + 8.0, 1.0, 2.0, col(0));
    draw_rectangle(x + 11.0, y + 8.0, 1.0, 2.0, col(0));
    // tail boom
    draw_rectangle(x, y + 4.0, 8.0, 2.0, col(10));
    draw_rectangle(x - 1.0, y + 1.0, 2.0, 4.0, col(10));
    // body
    draw_rectangle(x + 7.0, y + 3.0, 9.0, 5.0, col(10));
    draw_rectangle(x + 13.0, y + 4.0, 3.0, 2.0, col(1)); // cockpit glass
    draw_rectangle(x + 7.0, y + 2.0, 5.0, 1.0, col(8)); // white top stripe
                                                        // rotor mast + blades (2-frame spin)
    draw_rectangle(x + 10.0, y, 1.0, 3.0, col(0));
    let spin = (rotor * 3.0) as u32 % 2;
    if spin == 0 {
        ln(vec2(x - 2.0, y + 0.5), vec2(x + 23.0, y + 0.5), 1.0, col(0));
    } else {
        ln(vec2(x + 5.0, y + 0.5), vec2(x + 16.0, y + 0.5), 1.0, col(0));
    }
    if dead {
        // smoke puffs
        let puff = ((rotor * 2.0) as u32 % 3) as f32;
        draw_circle(
            x + 10.0 + puff,
            y - 2.0 - puff,
            1.5,
            Color { a: 0.6, ..col(14) },
        );
    }
}

/// 1px white outline hint when the heli hides behind a tall object.
pub fn heli_occluded(g: &Game) -> bool {
    let d = depth(g.heli.x, g.heli.y, g.view);
    g.world.objects.iter().any(|o| {
        let oz = o.top();
        oz > g.heli.z
            && depth(o.x, o.y, g.view) > d
            && (o.x - g.heli.x).abs() < TILE * 1.2
            && (o.y - g.heli.y).abs() < TILE * 1.2
    })
}

impl Default for Gfx {
    fn default() -> Self {
        Self::new()
    }
}
