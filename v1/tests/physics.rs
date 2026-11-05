//! Acceptance: identical physics at any display frame rate (fixed 60 Hz
//! accumulator), and input-script reproducibility.

use cyclone_rescue::game::{Game, Inputs, Phase};
use cyclone_rescue::tuning::*;

/// Emulate main.rs's accumulator at `fps`; run for `secs` of game time.
fn run(fps: f32, secs: f32, script: &dyn Fn(f32) -> Inputs) -> (u32, u32, u32, u32) {
    let mut g = Game::new(2024);
    g.heli.landed = false;
    g.heli.z = 4.0;
    g.phase = Phase::Flying;
    let frame_dt = 1.0 / fps;
    let mut acc = 0.0f32;
    let mut sim_t = 0.0f32;
    let target = (secs * PHYSICS_HZ) as u32; // same physics steps every fps
    let mut steps: u32 = 0;
    while steps < target {
        acc += frame_dt.min((target - steps) as f32 * PHYSICS_DT - acc);
        while acc >= PHYSICS_DT && steps < target {
            let inp = script(sim_t);
            g.step(PHYSICS_DT, &inp);
            acc -= PHYSICS_DT;
            sim_t += PHYSICS_DT;
            steps += 1;
        }
    }
    (
        g.heli.x.to_bits(),
        g.heli.y.to_bits(),
        g.heli.z.to_bits(),
        steps,
    )
}

fn script(t: f32) -> Inputs {
    Inputs {
        mx: if (t * 0.7).sin() > 0.0 { 1.0 } else { -1.0 },
        my: (t * 0.35).cos() * 0.8,
        climb: (t * 0.9).sin() > 0.2,
        descend: (t * 0.9).sin() < -0.6,
        flip: false,
        winch: false,
    }
}

#[test]
fn same_result_at_30_60_144_fps() {
    let a = run(30.0, 12.0, &script);
    let b = run(60.0, 12.0, &script);
    let c = run(144.0, 12.0, &script);
    assert_eq!(a.3, b.3);
    assert_eq!(b.3, c.3);
    // heli may have crashed/respawned mid-run; compare full state bits
    assert_eq!(a, c, "30 fps != 144 fps state");
    assert_eq!(a, b);
}

#[test]
fn seed_replay_is_bitwise_identical() {
    let a = run(60.0, 20.0, &script);
    let b = run(60.0, 20.0, &script);
    assert_eq!(a, b);
}
