//! Window config, fixed-step main loop, top-level state machine.

// Milestones 2..7 wire these up one by one; remove this allow in the polish pass.
#![allow(dead_code)]

mod font;
mod game;
mod heli;
mod iso;
mod render;
mod state;
mod tuning;
mod world;
mod worldgen;

use macroquad::prelude::*;
use state::{GameState, PlayCmd};
use tuning::*;

/// Table of key bindings — remap by editing this one place (spec: Controls).
mod keys {
    use macroquad::input::KeyCode as K;

    pub const UP: &[K] = &[K::Up, K::W];
    pub const DOWN: &[K] = &[K::Down, K::S];
    pub const LEFT: &[K] = &[K::Left, K::A];
    pub const RIGHT: &[K] = &[K::Right, K::D];
    pub const CLIMB: &[K] = &[K::Space, K::Q];
    pub const DESCEND: &[K] = &[K::LeftShift, K::Z];
    pub const FLIP: &[K] = &[K::V, K::Tab];
    pub const MAP: &[K] = &[K::M];
    pub const PAUSE: &[K] = &[K::P, K::Escape];
    pub const CONFIRM: &[K] = &[K::Enter, K::Space];
    pub const FULLSCREEN: &[K] = &[K::F11];
    pub const FULLSCREEN_ALT: &[K] = &[K::Enter]; // with Alt held
}

fn window_conf() -> macroquad::conf::Conf {
    let mut conf = macroquad::conf::Conf::default();
    conf.miniquad_conf.window_title = GAME_NAME.to_string();
    conf.miniquad_conf.window_width = (SCREEN_W * 4.0) as i32;
    conf.miniquad_conf.window_height = (SCREEN_H * 4.0) as i32;
    conf.miniquad_conf.window_resizable = true;
    conf.default_filter_mode = FilterMode::Nearest;
    conf
}

fn any_of(keys: &[macroquad::input::KeyCode]) -> bool {
    keys.iter().any(|k| is_key_down(*k))
}

fn any_pressed(keys: &[macroquad::input::KeyCode]) -> bool {
    keys.iter().any(|k| is_key_pressed(*k))
}

#[macroquad::main(window_conf)]
async fn main() {
    let gfx = render::Gfx::new();
    let mut state = GameState::Title;
    let high: Vec<(String, u32)> = Vec::new(); // filled in M7
    let mut acc = 0.0f32;
    let mut t = 0.0f32;
    let mut fullscreen = false;

    loop {
        let dt = get_frame_time().min(0.25);
        t += dt;
        acc += dt;
        while acc >= PHYSICS_DT {
            step(&mut state);
            acc -= PHYSICS_DT;
        }

        // Fullscreen toggles (window-level, any state).
        if any_pressed(keys::FULLSCREEN)
            || (is_key_down(KeyCode::LeftAlt) && any_pressed(keys::FULLSCREEN_ALT))
        {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }

        gfx.begin_scene();
        match state {
            GameState::Title => {
                clear_background(col_sea());
                gfx.draw_title((t * 2.0) as i32 % 2 == 0, &high);
                if !get_keys_pressed().is_empty() && t > 0.4 {
                    state = GameState::Playing;
                }
            }
            GameState::Playing => {
                gfx.draw_placeholder_play();
                gfx.draw_frame();
                gfx.draw_state_hint(state);
            }
            GameState::Paused => {
                gfx.draw_placeholder_play();
                gfx.draw_frame();
                gfx.draw_paused();
            }
            GameState::Map => {
                gfx.draw_placeholder_play();
                gfx.draw_frame();
            }
            _ => {
                gfx.draw_placeholder_play();
                gfx.draw_frame();
            }
        }
        gfx.present();

        next_frame().await;
    }
}

fn col_sea() -> Color {
    render::col(1)
}

/// Fixed-rate edge-triggered state transitions from input (spec: Game states).
fn step(state: &mut GameState) {
    let cmd = if is_key_pressed(KeyCode::Escape) {
        match *state {
            GameState::Paused => Some(PlayCmd::QuitToTitle),
            GameState::Map => Some(PlayCmd::CloseMap),
            _ => Some(PlayCmd::TogglePause),
        }
    } else if is_key_pressed(KeyCode::P) {
        Some(PlayCmd::TogglePause)
    } else if is_key_pressed(KeyCode::M) {
        Some(PlayCmd::OpenMap)
    } else if is_key_pressed(KeyCode::V) || is_key_pressed(KeyCode::Tab) {
        Some(PlayCmd::FlipView)
    } else {
        None
    };
    if let Some(c) = cmd {
        *state = state::transition(*state, c);
    }
}
