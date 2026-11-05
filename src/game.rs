//! The whole simulation in one pure, testable, macroquad-free struct.
//! main.rs feeds it input at a fixed 60 Hz; render/hud/map read it.

use crate::heli::Heli;
use crate::iso::View;
use crate::tuning::*;
use crate::world::*;
use crate::worldgen::*;

/// Screen-space control snapshot, sampled once per physics tick.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs {
    pub mx: f32, // -1..1 screen-left/right
    pub my: f32, // -1..1 screen-up/down
    pub climb: bool,
    pub descend: bool,
    pub flip: bool, // edge-triggered this tick
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
        }
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
                let was_flying = self.phase == Phase::Flying;
                self.heli
                    .step(dt, inp, &self.world, self.view, false, &mut self.rng);
                self.phase = match (self.phase, was_flying, self.heli.landed) {
                    (_, _, true) => {
                        if self.world.on_helipad(self.heli.x, self.heli.y) && self.heli.fuel < 1.0 {
                            Phase::Refuelling
                        } else {
                            Phase::Landed
                        }
                    }
                    _ => Phase::Flying,
                };
            }
        }

        self.camera_step(dt);
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
        self.lives = self.lives.saturating_sub(1);
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
    fn deterministic_sim_from_inputs() {
        let script: Vec<Inputs> = (0..600)
            .map(|i| Inputs {
                mx: if i % 120 < 60 { 1.0 } else { -1.0 },
                my: -0.5,
                climb: i % 200 < 100,
                descend: false,
                flip: i == 300,
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
