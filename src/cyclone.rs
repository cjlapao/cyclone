//! The cyclone: a wandering low-pressure system with a swirl force field.
//! Pure logic — every force the helicopter feels is reproducible from the
//! seed (spec: physics identical at any frame rate).

use crate::tuning::*;
use crate::world::Rng;

#[derive(Debug, Clone)]
pub struct Cyclone {
    pub x: f32,
    pub y: f32,
    pub target: (f32, f32),
    pub track_t: f32,
    pub speed: f32, // already includes the per-round storm factor
    pub phase: f32, // visual swirl
}

impl Cyclone {
    /// Start off a random map corner, far from the central Base Island.
    pub fn new(rng: &mut Rng, storm_factor: f32) -> Self {
        let edge = |r: &mut Rng| {
            if r.below(2) == 0 {
                r.range(4.0, 12.0) * TILE
            } else {
                (WORLD_TILES as f32 - r.range(4.0, 12.0)) * TILE
            }
        };
        let (x, y) = (edge(rng), edge(rng));
        Self {
            x,
            y,
            target: (WORLD_UNITS * 0.5, WORLD_UNITS * 0.5),
            track_t: CYCLONE_TRACK_SECS * 0.5, // first re-target sooner
            speed: CYCLONE_BASE_SPEED * storm_factor,
            phase: 0.0,
        }
    }

    /// Pick a fresh heading every TRACK_SECS: a random point on the map.
    pub fn step(&mut self, dt: f32, rng: &mut Rng) {
        self.phase += dt;
        self.track_t -= dt;
        if self.track_t <= 0.0 {
            self.track_t = CYCLONE_TRACK_SECS;
            self.target = (
                rng.range(8.0, WORLD_UNITS - 8.0),
                rng.range(8.0, WORLD_UNITS - 8.0),
            );
        }
        let dx = self.target.0 - self.x;
        let dy = self.target.1 - self.y;
        let d = (dx * dx + dy * dy).sqrt();
        if d > TILE * 0.5 {
            self.x += dx / d * self.speed * dt;
            self.y += dy / d * self.speed * dt;
        }
    }

    pub fn dist_to(&self, x: f32, y: f32) -> f32 {
        ((x - self.x).powi(2) + (y - self.y).powi(2)).sqrt()
    }

    /// Eye of the storm: anything inside crashes (spec: 1.5 tiles).
    pub fn crashes(&self, x: f32, y: f32) -> bool {
        self.dist_to(x, y) < CYCLONE_INNER
    }

    /// Drift velocity imparted at (x, y). `landed` halves it (spec).
    /// Tangential swirl + slight inward pull, scaled by ring falloff and
    /// deterministic jitter from the shared rng.
    pub fn force(&self, x: f32, y: f32, landed: bool, rng: &mut Rng) -> (f32, f32) {
        let dx = x - self.x;
        let dy = y - self.y;
        let d = (dx * dx + dy * dy).sqrt();
        if d > CYCLONE_RADIUS || d < 1e-3 {
            return (0.0, 0.0);
        }
        let fall = 1.0 - d / CYCLONE_RADIUS; // 0 outer edge → 1 eye
        let (swx, swy) = (-dy / d, dx / d); // counter-clockwise swirl
        let (px, py) = (-dx / d, -dy / d); // inward pull
        let mag = self.speed * (0.35 + 0.65 * fall);
        let j = |r: &mut Rng| r.range(-0.12, 0.12);
        let (jx, jy) = (j(rng), j(rng));
        let f = if landed { CYCLONE_LAND_FACTOR } else { 1.0 };
        (
            (swx * 0.7 + px * 0.35 + jx) * mag * f,
            (swy * 0.7 + py * 0.35 + jy) * mag * f,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_outside_radius_max_inside_eye_side() {
        let mut rng = Rng(1);
        let c = Cyclone::new(&mut rng, 1.0);
        let (fx, fy) = c.force(c.x + CYCLONE_RADIUS * 1.2, c.y, false, &mut rng);
        assert_eq!((fx, fy), (0.0, 0.0));
        let near = c.force(c.x + CYCLONE_INNER * 1.5, c.y, false, &mut rng);
        let mid = c.force(c.x + CYCLONE_RADIUS * 0.8, c.y, false, &mut rng);
        let nm = (near.0.powi(2) + near.1.powi(2)).sqrt();
        let mm = (mid.0.powi(2) + mid.1.powi(2)).sqrt();
        assert!(nm > mm, "force must fall off outward");
    }

    #[test]
    fn landed_halves_force() {
        let mut rng = Rng(2);
        let c = Cyclone::new(&mut rng, 1.0);
        let p = (c.x + CYCLONE_RADIUS * 0.5, c.y + 3.0);
        // same jitter both calls → pin rng between them
        let s = rng.0;
        let a = c.force(p.0, p.1, false, &mut rng);
        rng.0 = s;
        let b = c.force(p.0, p.1, true, &mut rng);
        assert!((a.0 * CYCLONE_LAND_FACTOR - b.0).abs() < 1e-3);
        assert!((a.1 * CYCLONE_LAND_FACTOR - b.1).abs() < 1e-3);
    }

    #[test]
    fn eye_crashes_ring_does_not() {
        let mut rng = Rng(3);
        let c = Cyclone::new(&mut rng, 1.0);
        assert!(c.crashes(c.x + 1.0, c.y));
        assert!(!c.crashes(c.x + CYCLONE_INNER * 2.0, c.y));
    }

    #[test]
    fn retargets_every_track_secs() {
        let mut rng = Rng(4);
        let mut c = Cyclone::new(&mut rng, 1.0);
        let t0 = c.target;
        let mut moved = false;
        for _ in 0..((CYCLONE_TRACK_SECS + 2.0) * PHYSICS_HZ) as u32 {
            c.step(PHYSICS_DT, &mut rng);
            if c.target != t0 {
                moved = true;
            }
        }
        assert!(moved, "storm re-targets within ~30 s");
    }
}
