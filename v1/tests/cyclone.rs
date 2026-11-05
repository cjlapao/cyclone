//! Hazards: the eye kills, the rim warns far, storms obey the seed.

use cyclone_rescue::cyclone::Cyclone;
use cyclone_rescue::game::{Game, Phase};
use cyclone_rescue::tuning::*;
use cyclone_rescue::world::Rng;

#[test]
fn storm_eye_crashes_a_hovering_heli() {
    let mut g = Game::new(9);
    g.heli.landed = false;
    g.heli.z = 4.0;
    g.phase = Phase::Flying;
    // park the eye right on us
    g.storm.x = g.heli.x;
    g.storm.y = g.heli.y;
    g.storm.target = (g.storm.x, g.storm.y);
    g.storm.track_t = 999.0;
    for _ in 0..30 {
        g.step(PHYSICS_DT, &Default::default());
    }
    assert_eq!(g.phase, Phase::Crashed, "eye must destroy the heli");
    assert!(g.lives < START_LIVES);
}

#[test]
fn rim_force_pushes_and_warns_before_contact() {
    let mut rng = Rng(3);
    let c = Cyclone::new(&mut rng, 1.0);
    // half-radius point still feels real drift...
    let (fx, fy) = c.force(c.x + CYCLONE_RADIUS * 0.5, c.y, false, &mut rng);
    let m = (fx * fx + fy * fy).sqrt();
    assert!(m > 0.0);
    // ...while the far field is quiet: the player can see it coming.
    let (fx2, fy2) = c.force(c.x + CYCLONE_RADIUS * 1.05, c.y, false, &mut rng);
    assert_eq!((fx2, fy2), (0.0, 0.0));
}

#[test]
fn storm_paths_are_seed_reproducible() {
    let path = |seed| {
        let mut rng = Rng(seed);
        let mut c = Cyclone::new(&mut rng, 1.3);
        for _ in 0..3600 {
            c.step(PHYSICS_DT, &mut rng);
        }
        (c.x.to_bits(), c.y.to_bits(), c.track_t.to_bits())
    };
    assert_eq!(path(77), path(77));
    assert_ne!(path(77), path(78));
}
