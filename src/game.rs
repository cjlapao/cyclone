//! The whole simulation in one pure, testable, macroquad-free struct.
//! main.rs feeds it input at a fixed 60 Hz; render/hud/map read it.

use crate::heli::Heli;
use crate::iso::View;
use crate::tuning::*;
use crate::world::*;
use crate::worldgen::*;

/// Things the simulation tells the shell to act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameEvent {
    RoundComplete,
    GameOver,
}

/// Screen-space control snapshot, sampled once per physics tick.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    pub mx: f32, // -1..1 screen-left/right
    pub my: f32, // -1..1 screen-up/down
    pub climb: bool,
    pub descend: bool,
    pub flip: bool,  // edge-triggered this tick
    pub winch: bool, // edge-triggered: hook the load below us
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Flying,
    Landed,
    Refuelling,
    Crashed,
}

/// View-independent play area size in px (world.rs/render.rs share it).
pub const PLAY_W: f32 = SCREEN_W - PANEL_W;
pub const PLAY_H: f32 = SCREEN_H - BAR_H;

pub struct Game {
    pub world: World,
    pub view: View,
    pub cam: (f32, f32),
    pub flip_t: f32,
    pub heli: Heli,
    pub phase: Phase,

    // round / objective (M4 wires the gameplay, fields exist now)
    pub seed: u64,
    pub round: u32,
    pub crates: Vec<Crate>,
    pub refugees: Vec<Refugee>,
    pub carried_crates: u32,
    pub carried_refs: u32,
    pub score: u32,
    pub lives: u32,
    pub time_left: f32,
    pub crash_t: f32,
    pub rng: Rng,

    // winch + objective bookkeeping
    pub winch_t: f32,
    pub winch_target: Option<(bool, usize)>, // (is_crate, index)
    pub delivered_crates: u32,
    pub delivered_refs: u32,
    score_f: f32,
    pub event: Option<GameEvent>,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Self::with_round(seed, 1)
    }

    pub fn with_round(seed: u64, round: u32) -> Self {
        let world = gen_world(seed);
        let (crates, refugees) = place_round(&world, seed, round);
        let base = &world.islands[0];
        let (px, py) = base
            .helipad
            .unwrap_or((World::tile_of(base.cx), World::tile_of(base.cy)));
        let x = (px as f32 + 0.5) * TILE;
        let y = (py as f32 + 0.5) * TILE;
        let z = world.h_at(x, y) as f32;
        let heli = Heli::spawn(x, y, z);
        Self {
            cam: (x, y),
            world,
            view: View::North,
            flip_t: 0.0,
            heli,
            phase: Phase::Landed,
            seed,
            round,
            crates,
            refugees,
            carried_crates: 0,
            carried_refs: 0,
            score: 0,
            lives: START_LIVES,
            time_left: (ROUND_TIME - (round as f32 - 1.0) * ROUND_TIME_STEP).max(MIN_ROUND_TIME),
            crash_t: 0.0,
            rng: Rng(seed ^ round as u64),
            winch_t: 0.0,
            winch_target: None,
            delivered_crates: 0,
            delivered_refs: 0,
            score_f: 0.0,
            event: None,
        }
    }

    /// Next round: same archipelago layout, re-seeded spawns, shorter timer.
    pub fn advance_round(&self) -> Game {
        let mut g = Game::with_round(self.seed, self.round + 1);
        g.score = self.score;
        g.lives = self.lives;
        g.view = self.view;
        g
    }

    /// Speed multiplier the cyclone uses this round (+10 % per round, spec).
    pub fn storm_factor(&self) -> f32 {
        CYCLONE_SPEEDUP.powf((self.round - 1) as f32)
    }

    /// True while the flip fade is running; render blends over it.
    pub fn flip_fade(&self) -> f32 {
        (self.flip_t / FLIP_FADE).clamp(0.0, 1.0)
    }

    /// One fixed physics tick.
    pub fn step(&mut self, dt: f32, inp: &Inputs) {
        // view flip with fade
        if inp.flip && self.flip_t == 0.0 {
            self.view = self.view.flipped();
            self.flip_t = FLIP_FADE;
        }
        if self.flip_t > 0.0 {
            self.flip_t = (self.flip_t - dt).max(0.0);
        }

        match self.phase {
            Phase::Crashed => {
                self.crash_t -= dt;
                if self.crash_t <= 0.0 {
                    self.respawn();
                }
            }
            Phase::Refuelling => {
                // stays landed; fuel handled inside heli step
                self.heli
                    .step(dt, inp, &self.world, self.view, true, &mut self.rng);
                if self.heli.fuel >= 1.0 || !self.world.on_helipad(self.heli.x, self.heli.y) {
                    self.phase = Phase::Landed;
                }
            }
            Phase::Landed | Phase::Flying => {
                self.heli
                    .step(dt, inp, &self.world, self.view, false, &mut self.rng);
                self.phase = if self.heli.landed {
                    if self.world.on_helipad(self.heli.x, self.heli.y) && self.heli.fuel < 1.0 {
                        Phase::Refuelling
                    } else {
                        Phase::Landed
                    }
                } else {
                    Phase::Flying
                };
            }
        }

        // a dead airframe triggers the life loop
        if self.heli.dead && self.phase != Phase::Crashed {
            self.crash();
        }

        self.winch_step(dt, inp);
        self.objective_step();

        // countdown + ground time bonus (spec: +10/s while landed)
        if self.phase != Phase::Crashed {
            self.time_left -= dt;
        }
        if matches!(self.phase, Phase::Landed | Phase::Refuelling) {
            self.score_f += SCORE_PER_SECOND as f32 * dt;
            let whole = self.score_f as u32;
            if whole > 0 {
                self.score += whole;
                self.score_f -= whole as f32;
            }
        }
        if self.time_left <= 0.0 && self.event.is_none() {
            self.time_left = 0.0;
            self.event = Some(GameEvent::GameOver);
        }

        self.camera_step(dt);
    }

    fn winch_step(&mut self, dt: f32, inp: &Inputs) {
        let gh = self.world.h_at(self.heli.x, self.heli.y) as f32;
        let low = self.phase == Phase::Flying && self.heli.z - gh <= WINCH_MAX_ALT;

        if let Some((is_crate, idx)) = self.winch_target {
            self.winch_t -= dt;
            let still = match is_crate {
                true => self
                    .crates
                    .get(idx)
                    .is_some_and(|c| self.load_dist(c.x, c.y) <= WINCH_RADIUS),
                false => self
                    .refugees
                    .get(idx)
                    .is_some_and(|r| self.load_dist(r.x, r.y) <= WINCH_RADIUS),
            };
            if !still || !low {
                self.winch_target = None;
                self.winch_t = 0.0;
                return;
            }
            if self.winch_t <= 0.0 {
                self.winch_target = None;
                match is_crate {
                    true if self.carried_crates < NUM_CRATES => {
                        self.crates.swap_remove(idx);
                        self.carried_crates += 1;
                    }
                    false if self.carried_refs < 8 => {
                        self.refugees.swap_remove(idx);
                        self.carried_refs += 1;
                    }
                    _ => {}
                }
            }
        } else if inp.winch && low {
            let cap_c = self.carried_crates < NUM_CRATES;
            let cap_r = self.carried_refs < 8;
            let ci = self
                .crates
                .iter()
                .position(|c| cap_c && self.load_dist(c.x, c.y) <= WINCH_RADIUS);
            let ri = self
                .refugees
                .iter()
                .position(|r| cap_r && self.load_dist(r.x, r.y) <= WINCH_RADIUS);
            if let Some(i) = ci {
                self.winch_target = Some((true, i));
                self.winch_t = WINCH_LIFT_TIME;
            } else if let Some(i) = ri {
                self.winch_target = Some((false, i));
                self.winch_t = WINCH_LIFT_TIME;
            }
        }
    }

    fn load_dist(&self, x: f32, y: f32) -> f32 {
        ((x - self.heli.x).powi(2) + (y - self.heli.y).powi(2)).sqrt()
    }

    fn objective_step(&mut self) {
        if !matches!(self.phase, Phase::Landed | Phase::Refuelling) {
            return;
        }
        let on_base_pad = match self.world.islands[0].helipad {
            Some((px, py)) => {
                let hx = World::tile_of(self.heli.x);
                let hy = World::tile_of(self.heli.y);
                hx.abs_diff(px) <= 1 && hy.abs_diff(py) <= 1
            }
            None => false,
        };
        if !on_base_pad || self.carried_crates + self.carried_refs == 0 {
            return;
        }
        self.score += self.carried_crates * SCORE_CRATE + self.carried_refs * SCORE_REFUGEE;
        self.delivered_crates += self.carried_crates;
        self.delivered_refs += self.carried_refs;
        self.carried_crates = 0;
        self.carried_refs = 0;
        self.heli.crates = 0;
        self.heli.refs = 0;
        if self.delivered_crates >= NUM_CRATES && self.event.is_none() {
            self.event = Some(GameEvent::RoundComplete);
        }
    }

    fn camera_step(&mut self, dt: f32) {
        // exponential lag toward the helicopter, clamped inside the world
        let k = 1.0 - (-4.0f32 * dt).exp();
        let margin = (PLAY_W + PLAY_H) * 0.5;
        let tx = self.heli.x.clamp(margin, WORLD_UNITS - margin);
        let ty = self.heli.y.clamp(margin, WORLD_UNITS - margin);
        self.cam.0 += (tx - self.cam.0) * k;
        self.cam.1 += (ty - self.cam.1) * k;
    }

    pub fn crash(&mut self) {
        if self.phase == Phase::Crashed {
            return;
        }
        self.phase = Phase::Crashed;
        self.crash_t = CRASH_PAUSE;
        self.winch_target = None;
        self.winch_t = 0.0;
        self.lives = self.lives.saturating_sub(1);
        if self.lives == 0 && self.event.is_none() {
            self.event = Some(GameEvent::GameOver);
        }
    }

    fn respawn(&mut self) {
        let base = &self.world.islands[0];
        let (px, py) = base
            .helipad
            .unwrap_or((World::tile_of(base.cx), World::tile_of(base.cy)));
        let x = (px as f32 + 0.5) * TILE;
        let y = (py as f32 + 0.5) * TILE;
        // keep cargo (spec: collected cargo is kept across crashes)
        let (cc, cr) = (self.carried_crates, self.carried_refs);
        self.heli = Heli::spawn(x, y, self.world.h_at(x, y) as f32);
        self.heli.crates = cc;
        self.heli.refs = cr;
        self.phase = Phase::Landed;
    }

    /// Ground speed, for HUD.
    pub fn speed(&self) -> f32 {
        (self.heli.vx * self.heli.vx + self.heli.vy * self.heli.vy).sqrt() / TILE
    }
}

/// Is the tile (tx,ty) on land in either view? (helper for tests/map)
pub fn is_land(w: &World, tx: u32, ty: u32) -> bool {
    w.tile_h(tx, ty) > 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iso::flip_xy;

    #[test]
    fn flip_keeps_screen_relative_input() {
        // Pushing "up" must move the heli screen-up in either view:
        // screen up = decreasing (fx+fy). Check the world delta sign.
        for view in [View::North, View::South] {
            let mut g = Game::new(11);
            g.view = view;
            g.heli.z = 3.0;
            g.heli.landed = false;
            g.phase = Phase::Flying;
            let before = iso_depth(&g);
            let inp = Inputs {
                my: -1.0,
                ..Default::default()
            };
            for _ in 0..30 {
                g.step(PHYSICS_DT, &inp);
            }
            assert!(
                iso_depth(&g) < before,
                "view {view:?} up must decrease depth"
            );
        }
    }

    fn iso_depth(g: &Game) -> f32 {
        let (fx, fy) = flip_xy(g.heli.x, g.heli.y, g.view);
        fx + fy
    }

    #[test]
    fn winch_lifts_crate() {
        let mut g = Game::new(5);
        let pad = g.world.islands[0].helipad.unwrap();
        let x = (pad.0 as f32 + 0.5) * TILE;
        let y = (pad.1 as f32 + 0.5) * TILE;
        g.heli = Heli::spawn(x, y, g.world.h_at(x, y) as f32 + 1.0);
        g.heli.landed = false;
        g.phase = Phase::Flying;
        g.crates.push(Crate {
            x,
            y,
            island: 0,
            hidden_by: None,
        });
        let n = g.crates.len();
        let inp = Inputs {
            winch: true,
            ..Default::default()
        };
        for _ in 0..60 {
            g.step(PHYSICS_DT, &inp);
        }
        assert_eq!(g.crates.len(), n - 1);
        assert_eq!(g.carried_crates, 1);
    }

    #[test]
    fn delivery_scores_and_completes_round() {
        let mut g = Game::new(5);
        let pad = g.world.islands[0].helipad.unwrap();
        let x = (pad.0 as f32 + 0.5) * TILE;
        let y = (pad.1 as f32 + 0.5) * TILE;
        g.heli = Heli::spawn(x, y, g.world.h_at(x, y) as f32);
        g.phase = Phase::Landed;
        for c in 0..5 {
            g.carried_crates = 1;
            g.carried_refs = 2;
            g.objective_step();
            assert_eq!(g.carried_crates, 0);
            let want = (c + 1) * (SCORE_CRATE + 2 * SCORE_REFUGEE);
            assert!(g.score >= want);
        }
        assert_eq!(g.event, Some(GameEvent::RoundComplete));
    }

    #[test]
    fn timer_runs_out_and_crashes_end_game() {
        let mut g = Game::new(5);
        g.time_left = 0.5;
        for _ in 0..60 {
            g.step(PHYSICS_DT, &Inputs::default());
        }
        assert_eq!(g.event, Some(GameEvent::GameOver));

        let mut g2 = Game::new(5);
        for _ in 0..3 {
            g2.event = None;
            g2.heli.dead = true;
            for _ in 0..300 {
                g2.step(PHYSICS_DT, &Inputs::default());
                if g2.phase == Phase::Landed && !g2.heli.dead {
                    break;
                }
            }
        }
        assert_eq!(g2.lives, 0);
        assert_eq!(g2.event, Some(GameEvent::GameOver));
    }

    #[test]
    fn next_round_shortens_timer_and_speeds_storm() {
        let g = Game::with_round(5, 1);
        let n = g.advance_round();
        assert_eq!(n.round, 2);
        assert!((n.time_left - (ROUND_TIME - ROUND_TIME_STEP)).abs() < 1e-3);
        assert!((g.storm_factor() - 1.0).abs() < 1e-6);
        assert!((n.storm_factor() - CYCLONE_SPEEDUP).abs() < 1e-6);
    }

    #[test]
    fn deterministic_sim_from_inputs() {
        let script: Vec<Inputs> = (0..600)
            .map(|i| Inputs {
                mx: if i % 120 < 60 { 1.0 } else { -1.0 },
                my: -0.5,
                climb: i % 200 < 100,
                descend: false,
                flip: i == 300,
                winch: i == 400,
            })
            .collect();
        let run = || {
            let mut g = Game::new(77);
            g.phase = Phase::Flying;
            g.heli.z = 4.0;
            for inp in &script {
                g.step(PHYSICS_DT, inp);
            }
            (
                g.heli.x.to_bits(),
                g.heli.y.to_bits(),
                g.heli.z.to_bits(),
                g.view == View::South,
            )
        };
        assert_eq!(run(), run());
    }
}
