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
