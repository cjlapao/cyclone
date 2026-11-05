//! Scheduled sea-skimming plane runs: 2 s warning, then a pass at altitude
//! 3..5 across a sea lane. Pure logic.

use crate::tuning::*;
use crate::world::{Rng, World};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaneState {
    Idle,
    Warning,
    Flying,
}

#[derive(Debug, Clone)]
pub struct Planes {
    pub state: PlaneState,
    /// Start point and direction of the current/planned run.
    pub x: f32,
    pub y: f32,
    pub dir: (f32, f32),
    pub alt: f32,
    pub warn_t: f32,
    pub next_t: f32,
}

impl Planes {
    pub fn new(rng: &mut Rng) -> Self {
        Self {
            state: PlaneState::Idle,
            x: 0.0,
            y: 0.0,
            dir: (1.0, 0.0),
            alt: 4.0,
            warn_t: 0.0,
            next_t: rng.range(PLANE_MIN_GAP * 0.5, PLANE_MIN_GAP), // first pass early-ish
        }
    }

    /// Pick a straight sea lane across the map; returns false if the region
    /// is unlucky and we should retry soon.
    fn try_lane(&mut self, world: &World, rng: &mut Rng) -> bool {
        for _ in 0..24 {
            let horiz = rng.below(2) == 0;
            let off = rng.range(2.0, 6.0) * TILE;
            let at_far = rng.below(2) == 1;
            let (x0, y0, dir) = if horiz {
                let y = if at_far { WORLD_UNITS - off } else { off };
                (0.0, y, (1.0f32, 0.0f32))
            } else {
                let x = if at_far { WORLD_UNITS - off } else { off };
                (x, 0.0, (0.0f32, 1.0f32))
            };
            // sample the lane: it must be open sea the whole way
            let mut sea = true;
            let mut t = 0.0;
            while t <= WORLD_UNITS {
                if world.h_at(x0 + dir.0 * t, y0 + dir.1 * t) > 0 {
                    sea = false;
                    break;
                }
                t += 2.0 * TILE;
            }
            if sea {
                self.x = x0;
                self.y = y0;
                self.dir = dir;
                self.alt = rng.range(PLANE_ALT_MIN, PLANE_ALT_MAX);
                return true;
            }
        }
        false
    }

    pub fn step(&mut self, dt: f32, world: &World, rng: &mut Rng) {
        match self.state {
            PlaneState::Idle => {
                self.next_t -= dt;
                if self.next_t <= 0.0 {
                    if self.try_lane(world, rng) {
                        self.state = PlaneState::Warning;
                        self.warn_t = PLANE_WARN_TIME;
                    } else {
                        self.next_t = 5.0; // retry soon
                    }
                }
            }
            PlaneState::Warning => {
                self.warn_t -= dt;
                if self.warn_t <= 0.0 {
                    self.state = PlaneState::Flying;
                }
            }
            PlaneState::Flying => {
                self.x += self.dir.0 * PLANE_SPEED * dt;
                self.y += self.dir.1 * PLANE_SPEED * dt;
                let out = self.x > WORLD_UNITS + TILE * 2.0
                    || self.y > WORLD_UNITS + TILE * 2.0
                    || (self.dir.0 == 0.0 && self.y < -TILE * 2.0)
                    || (self.dir.1 == 0.0 && self.x < -TILE * 2.0);
                if out {
                    self.state = PlaneState::Idle;
                    self.next_t = rng.range(PLANE_MIN_GAP, PLANE_MAX_GAP);
                }
            }
        }
    }

    /// Mid-air collision: within one tile horizontally and inside the alt band.
    pub fn hits(&self, hx: f32, hy: f32, hz: f32) -> bool {
        if self.state != PlaneState::Flying {
            return false;
        }
        let d = ((hx - self.x).powi(2) + (hy - self.y).powi(2)).sqrt();
        d < TILE && (hz - self.alt).abs() <= PLANE_ALT_TOL
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::gen_world;

    #[test]
    fn warns_at_least_two_seconds_before_flying() {
        let w = gen_world(1984);
        let mut rng = Rng(9);
        let mut p = Planes::new(&mut rng);
        // fast-forward: force spawn by draining next_t
        p.next_t = 0.0;
        let mut warned_at = None;
        let mut flew_at = None;
        let mut t = 0.0;
        for _ in 0..(60.0 * PHYSICS_HZ) as u32 {
            p.step(PHYSICS_DT, &w, &mut rng);
            if p.state == PlaneState::Warning && warned_at.is_none() {
                warned_at = Some(t);
            }
            if p.state == PlaneState::Flying && flew_at.is_none() {
                flew_at = Some(t);
            }
            t += PHYSICS_DT;
            if flew_at.is_some() {
                break;
            }
        }
        let (warn, fly) = (warned_at.expect("lane found"), flew_at.expect("flew"));
        assert!((fly - warn - PLANE_WARN_TIME).abs() < 2.0 * PHYSICS_DT + 1e-3);
    }

    #[test]
    fn lanes_stay_over_sea() {
        let w = gen_world(1984);
        for s in 0..6u64 {
            let mut rng = Rng(s * 7 + 3);
            let mut p = Planes::new(&mut rng);
            p.next_t = 0.0;
            for _ in 0..600 {
                p.step(PHYSICS_DT, &w, &mut rng);
                if p.state == PlaneState::Warning {
                    break;
                }
            }
            if p.state != PlaneState::Warning {
                continue; // retry path, no lane this round
            }
            let mut t = 0.0;
            while t <= WORLD_UNITS {
                assert_eq!(
                    w.h_at(p.x + p.dir.0 * t, p.y + p.dir.1 * t),
                    0,
                    "plane lane {s} crossed land"
                );
                t += TILE;
            }
        }
    }

    #[test]
    fn collision_needs_altitude_band() {
        let mut rng = Rng(1);
        let mut p = Planes::new(&mut rng);
        p.state = PlaneState::Flying;
        p.x = 100.0;
        p.y = 100.0;
        p.alt = 4.0;
        assert!(!p.hits(100.0, 100.0, 1.0)); // too low
        assert!(!p.hits(100.0, 100.0, 8.0)); // too high
        assert!(p.hits(100.0, 100.0, 4.0));
        assert!(!p.hits(100.0 + TILE * 3.0, 100.0, 4.0)); // passed by
    }
}
