//! Helicopter physics: inertial movement, altitude, fuel, landing/crash
//! rules (pure logic). Screen-relative input is mapped through the iso
//! projection so controls feel identical in either view.

use crate::game::Inputs;
use crate::iso::View;
use crate::tuning::*;
use crate::world::Rng;
use crate::world::World;

#[derive(Debug, Clone)]
pub struct Heli {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub vx: f32,
    pub vy: f32,
    pub vz: f32,
    /// 0..1 (1 = full tank).
    pub fuel: f32,
    pub landed: bool,
    pub rotor: f32, // spin phase for animation
    // carried cargo (M4 uses it; physics keeps it across respawns)
    pub crates: u32,
    pub refs: u32,
    pub dead: bool, // set on the tick a crash happens (engine out too)
}

impl Heli {
    pub fn spawn(x: f32, y: f32, z: f32) -> Self {
        Self {
            x,
            y,
            z,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            fuel: 1.0,
            landed: true,
            rotor: 0.0,
            crates: 0,
            refs: 0,
            dead: false,
        }
    }

    /// Screen direction (mx, my) → world direction for the current view.
    fn world_dir(mx: f32, my: f32, view: View) -> (f32, f32) {
        // inverse of sx = rx - ry ; sy = (rx + ry)/2 applied to (mx,my)
        let rx = (2.0 * my + mx) * 0.5;
        let ry = (2.0 * my - mx) * 0.5;
        let (dx, dy) = match view {
            View::North => (rx, ry),
            View::South => (-rx, -ry),
        };
        let l = (dx * dx + dy * dy).sqrt();
        if l < 1e-6 {
            (0.0, 0.0)
        } else {
            (dx / l, dy / l)
        }
    }

    pub fn speed(&self) -> f32 {
        (self.vx * self.vx + self.vy * self.vy).sqrt()
    }

    /// One fixed tick. `refuelling` = sitting on a helipad for fuel.
    pub fn step(
        &mut self,
        dt: f32,
        inp: &Inputs,
        world: &World,
        view: View,
        refuelling: bool,
        _rng: &mut Rng,
    ) {
        self.rotor += dt * (6.0 + 18.0 * self.speed() / HELI_MAX_SPEED);

        if self.dead {
            // engine-out: uncontrolled descent
            self.vz = -3.5;
            self.z = (self.z + self.vz * dt).max(0.0);
            self.land_attempt(world, true);
            return;
        }

        let empty = self.fuel <= 0.0;
        if empty {
            self.fuel = 0.0;
            self.dead = true; // engine cuts; heli descends (spec)
            return;
        }

        if refuelling {
            self.fuel = (self.fuel + dt / REFUEL_SECONDS).min(1.0);
        }

        if self.landed {
            // ground contact: only climb lifts off; controls do nothing
            if inp.climb {
                self.landed = false;
                self.z += CLIMB_RATE * dt * 0.5;
            }
            self.vx = 0.0;
            self.vy = 0.0;
            self.vz = 0.0;
            return;
        }

        // thrust in screen-relative direction
        let (dx, dy) = Self::world_dir(inp.mx, inp.my, view);
        self.vx += dx * HELI_ACCEL * dt;
        self.vy += dy * HELI_ACCEL * dt;
        // drag
        let d = 1.0 - (HELI_DRAG * dt).min(0.9);
        self.vx *= d;
        self.vy *= d;
        // speed cap
        let s = self.speed();
        if s > HELI_MAX_SPEED {
            let k = HELI_MAX_SPEED / s;
            self.vx *= k;
            self.vy *= k;
        }
        // vertical
        self.vz = if inp.climb {
            CLIMB_RATE
        } else if inp.descend {
            -CLIMB_RATE
        } else {
            0.0
        };

        self.x += self.vx * dt;
        self.y += self.vy * dt;
        self.z += self.vz * dt;
        self.x = self.x.clamp(0.0, WORLD_UNITS);
        self.y = self.y.clamp(0.0, WORLD_UNITS);
        self.z = self.z.clamp(0.0, MAX_ALT);

        // fuel burn: base + proportional to speed fraction
        self.fuel = (self.fuel
            - dt / FUEL_SECONDS * (1.0 + FUEL_SPEED_FACTOR * s / HELI_MAX_SPEED))
            .max(0.0);

        // object collision while airborne
        if world.hits_object(self.x, self.y, self.z) {
            self.dead = true;
            return;
        }

        // ground contact?
        let gh = world.h_at(self.x, self.y) as f32;
        if self.z <= gh {
            self.z = gh;
            self.land_attempt(world, false);
        }
    }

    fn land_attempt(&mut self, world: &World, engine_out: bool) {
        let flat = world.landing_ok(self.x, self.y);
        let slow = self.speed() <= LAND_MAX_SPEED;
        if flat && (slow || engine_out && self.speed() <= LAND_MAX_SPEED) {
            // safe touchdown
            self.landed = true;
            self.vx = 0.0;
            self.vy = 0.0;
            self.vz = 0.0;
        } else {
            self.dead = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::worldgen::gen_world;

    fn world() -> World {
        gen_world(1984)
    }

    #[test]
    fn gentle_touchdown_survives() {
        let w = world();
        let (px, py) = w.islands[0].helipad.unwrap();
        let x = (px as f32 + 0.5) * TILE;
        let y = (py as f32 + 0.5) * TILE;
        let mut h = Heli::spawn(x, y, w.h_at(x, y) as f32 + 1.5);
        h.landed = false;
        let mut rng = Rng(1);
        for _ in 0..60 {
            // hover down slowly, no horizontal speed
            h.step(
                PHYSICS_DT,
                &Inputs {
                    descend: true,
                    ..Default::default()
                },
                &w,
                View::North,
                false,
                &mut rng,
            );
            if h.dead {
                break;
            }
        }
        assert!(!h.dead, "slow landing on the pad must survive");
        assert!(h.landed);
    }

    #[test]
    fn fast_or_slope_landing_crashes() {
        let w = world();
        let gh = w.h_at(w.islands[1].cx, w.islands[1].cy) as f32;
        let mut h = Heli::spawn(w.islands[1].cx, w.islands[1].cy, gh);
        h.landed = false;
        h.vx = 50.0; // too fast
        let mut rng = Rng(1);
        h.step(
            PHYSICS_DT,
            &Inputs::default(),
            &w,
            View::North,
            false,
            &mut rng,
        );
        assert!(h.dead || !w.landing_ok(h.x, h.y));

        // over the sea always crashes on contact
        let mut h2 = Heli::spawn(2.0, 2.0, 0.5);
        h2.landed = false;
        for _ in 0..20 {
            h2.step(
                PHYSICS_DT,
                &Inputs {
                    descend: true,
                    ..Default::default()
                },
                &w,
                View::North,
                false,
                &mut rng,
            );
            if h2.dead {
                break;
            }
        }
        assert!(h2.dead, "sea contact crashes");
    }

    #[test]
    fn fuel_out_cuts_engine_and_drops_heli() {
        let w = world();
        let mut h = Heli::spawn(48.0 * TILE, 48.0 * TILE, 6.0);
        h.landed = false;
        h.fuel = PHYSICS_DT / FUEL_SECONDS * 1.1; // one tick of fuel
        let mut rng = Rng(1);
        let mut descended = false;
        for _ in 0..120 {
            h.step(
                PHYSICS_DT,
                &Inputs::default(),
                &w,
                View::North,
                false,
                &mut rng,
            );
            if h.dead && h.z > 0.5 {
                descended = true;
                break;
            }
        }
        assert!(descended, "engine-out descends before touching down");
    }

    #[test]
    fn refuel_fills_tank() {
        let w = world();
        let (px, py) = w.islands[0].helipad.unwrap();
        let mut h = Heli::spawn((px as f32 + 0.5) * TILE, (py as f32 + 0.5) * TILE, 1.0);
        h.fuel = 0.1;
        let mut rng = Rng(1);
        for _ in 0..(REFUEL_SECONDS * PHYSICS_HZ) as u32 + 5 {
            h.step(
                PHYSICS_DT,
                &Inputs::default(),
                &w,
                View::North,
                true,
                &mut rng,
            );
        }
        assert!(h.fuel >= 1.0 - 1e-3);
    }
}
