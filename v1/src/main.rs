//! Window config, fixed-step main loop, top-level state machine.

use cyclone_rescue::audio::Audio;
use cyclone_rescue::game;
use cyclone_rescue::game::{Game, Inputs};
use cyclone_rescue::hud;
use cyclone_rescue::iso;
use cyclone_rescue::map;
use cyclone_rescue::render;
use cyclone_rescue::state::{self, GameState, PlayCmd};
use cyclone_rescue::tuning::*;
use macroquad::prelude::*;

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
    pub const WINCH: &[K] = &[K::E, K::Enter];
    pub const MUTE: &[K] = &[K::K];
    pub const MAP: &[K] = &[K::M];
    pub const PAUSE: &[K] = &[K::P, K::Escape];
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
    let mut acc = 0.0f32;
    let mut t = 0.0f32;
    let mut fullscreen = false;
    let mut game: Option<Game> = None;
    let mut audio = Some(Audio::load().await);
    let mut high = load_high();

    loop {
        let dt = get_frame_time().min(0.25);
        t += dt;
        acc += dt;
        while acc >= PHYSICS_DT {
            step(&mut state, game.as_mut());
            acc -= PHYSICS_DT;
        }

        // audio: drain queued cues, keep rotor hum alive, mute toggle
        if any_pressed(keys::MUTE) {
            if let Some(a) = audio.as_mut() {
                a.toggle_mute();
            }
        }
        if let (Some(a), Some(g)) = (audio.as_mut(), game.as_mut()) {
            for e in g.fx.drain(..) {
                a.play(e);
            }
            let flying = g.phase == game::Phase::Flying;
            a.rotor(flying, g.speed() / HELI_MAX_SPEED);
        } else if let Some(a) = audio.as_mut() {
            a.rotor(false, 0.0);
        }

        // Fullscreen toggles (window-level, any state).
        if any_pressed(keys::FULLSCREEN)
            || (is_key_down(KeyCode::LeftAlt) && any_pressed(keys::FULLSCREEN_ALT))
        {
            fullscreen = !fullscreen;
            set_fullscreen(fullscreen);
        }

        // simulation events end the play session
        if let Some(g) = game.as_mut() {
            if let Some(ev) = g.event.take() {
                match ev {
                    game::GameEvent::RoundComplete => {
                        if let Some(a) = audio.as_mut() {
                            a.play(game::Sfx::Round);
                        }
                        state = GameState::RoundComplete;
                    }
                    game::GameEvent::GameOver => {
                        if let Some(a) = audio.as_mut() {
                            a.play(game::Sfx::Over);
                        }
                        record_high(&mut high, g.score, g.round, g.seed);
                        state = GameState::GameOver;
                    }
                }
            }
        }

        gfx.begin_scene();
        match state {
            GameState::Title => {
                clear_background(col_sea());
                gfx.draw_title((t * 2.0) as i32 % 2 == 0, &high);
                if !get_keys_pressed().is_empty() && t > 0.4 {
                    game = Some(Game::new(BASE_SEED));
                    state = GameState::Playing;
                }
            }
            GameState::Playing | GameState::Paused | GameState::Map => {
                if let Some(g) = &game {
                    if state == GameState::Map {
                        map::draw_map(&gfx.font, g, t);
                    } else {
                        render::draw_scene(g, t);
                        if render::heli_occluded(g) {
                            // keep the airframe findable behind tall objects
                            let p = iso_hud_pos(g);
                            draw_circle_lines(p.x, p.y - 4.0, 9.0, 1.0, WHITE);
                        }
                    }
                }
                gfx.draw_frame();
                if let Some(g) = &game {
                    if state != GameState::Map {
                        hud::draw_panel(&gfx.font, g, t);
                    }
                    hud::draw_bar(&gfx.font, g);
                }
                if state == GameState::Paused {
                    gfx.draw_paused();
                }
            }
            GameState::RoundComplete | GameState::GameOver => {
                if let Some(g) = &game {
                    render::draw_scene(g, t);
                }
                gfx.draw_frame();
                let dim = Color { a: 0.55, ..BLACK };
                draw_rectangle(0.0, 0.0, game::PLAY_W, game::PLAY_H, dim);
                let (msg, sub) = if state == GameState::RoundComplete {
                    let g = game.as_ref().unwrap();
                    (
                        "MISSION COMPLETE",
                        format!("SCORE {}  TIME LEFT {}s", g.score, g.time_left as i32),
                    )
                } else {
                    let sc = game.as_ref().map(|g| g.score).unwrap_or(0);
                    ("GAME OVER", format!("SCORE {sc}  SEED {BASE_SEED}"))
                };
                gfx.font
                    .draw_centered(msg, game::PLAY_W * 0.5, 80.0, 3.0, render::col(11));
                gfx.font
                    .draw_centered(&sub, game::PLAY_W * 0.5, 110.0, 1.0, render::col(8));
                let hint = if state == GameState::RoundComplete {
                    "PRESS ENTER FOR NEXT ROUND"
                } else {
                    "PRESS ENTER"
                };
                if (t * 2.0) as i32 % 2 == 0 {
                    gfx.font
                        .draw_centered(hint, game::PLAY_W * 0.5, 150.0, 1.0, WHITE);
                }
                if !get_keys_pressed().is_empty() && t > 0.2 {
                    if state == GameState::RoundComplete {
                        game = game.as_ref().map(|g| g.advance_round());
                        state = GameState::Playing;
                    } else {
                        game = None;
                        state = GameState::Title;
                    }
                    t = 0.0;
                }
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

fn hi_path() -> std::path::PathBuf {
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .or_else(|| std::env::var_os("APPDATA"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join(".cyclone_rescue_highscores")
}

/// One stored line: "score|round|seed".
struct Hi {
    score: u32,
    round: u32,
    seed: u64,
}

fn load_high() -> Vec<(String, u32)> {
    let mut v: Vec<Hi> = std::fs::read_to_string(hi_path())
        .unwrap_or_default()
        .lines()
        .filter_map(|l| {
            let mut it = l.split('|');
            Some(Hi {
                score: it.next()?.parse().ok()?,
                round: it.next()?.parse().ok()?,
                seed: it.next()?.parse().ok()?,
            })
        })
        .collect();
    v.sort_by_key(|h| std::cmp::Reverse(h.score));
    v.truncate(HIGH_SCORES);
    v.into_iter()
        .map(|h| (format!("R{} S{}", h.round, h.seed), h.score))
        .collect()
}

fn record_high(high: &mut Vec<(String, u32)>, score: u32, round: u32, seed: u64) {
    high.push((format!("R{round} S{seed}"), score));
    high.sort_by_key(|h| std::cmp::Reverse(h.1));
    high.truncate(HIGH_SCORES);
    let txt: String = high
        .iter()
        .map(|(tag, s)| {
            // tag is "R<round> S<seed>" — rebuild the numeric line from it
            let round = tag[1..].split(' ').next().unwrap_or("1");
            let seed = tag
                .split(' ')
                .nth(1)
                .and_then(|t| t.strip_prefix('S'))
                .unwrap_or("0");
            format!("{s}|{round}|{seed}\n")
        })
        .collect();
    let _ = std::fs::write(hi_path(), txt);
}

fn iso_hud_pos(g: &Game) -> Vec2 {
    let (sx, sy) = iso::project(g.heli.x, g.heli.y, g.heli.z, g.cam, g.view);
    vec2(game::PLAY_W * 0.5 + sx, game::PLAY_H * 0.5 + sy)
}

/// Screen-space control snapshot for the physics tick (spec: Controls).
fn read_inputs() -> Inputs {
    let mx = any_of(keys::RIGHT) as i32 as f32 - any_of(keys::LEFT) as i32 as f32;
    let my = any_of(keys::DOWN) as i32 as f32 - any_of(keys::UP) as i32 as f32;
    Inputs {
        mx,
        my,
        climb: any_of(keys::CLIMB),
        descend: any_of(keys::DESCEND),
        flip: any_pressed(keys::FLIP),
        winch: any_pressed(keys::WINCH),
    }
}

/// Fixed-rate edge-triggered state transitions from input (spec: Game states).
fn step(state: &mut GameState, game: Option<&mut Game>) {
    let cmd = if any_pressed(keys::PAUSE) {
        match *state {
            GameState::Paused => Some(PlayCmd::QuitToTitle),
            GameState::Map => Some(PlayCmd::CloseMap),
            _ => Some(PlayCmd::TogglePause),
        }
    } else if any_pressed(keys::MAP) {
        Some(PlayCmd::OpenMap)
    } else if any_pressed(keys::FLIP) {
        Some(PlayCmd::FlipView)
    } else {
        None
    };
    if let Some(c) = cmd {
        *state = state::transition(*state, c);
    }
    if *state == GameState::Playing {
        if let Some(g) = game {
            g.step(PHYSICS_DT, &read_inputs());
        }
    }
}
