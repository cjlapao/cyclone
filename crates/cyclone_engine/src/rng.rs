//! RngPlugin (spec 00): the run's GameSeed and random seed drawing.

use bevy::prelude::*;
use cyclone_worldgen::Seed;

/// The seed the current world was generated from (spec 01 handoff).
#[derive(Resource)]
pub struct GameSeed(pub Seed);

/// A fresh uniformly random seed from the OS CSPRNG (getrandom, inside
/// worldgen's Seed type so both sides share one implementation).
pub fn random_seed() -> Seed {
    Seed::random()
}

pub struct RngPlugin;

impl Plugin for RngPlugin {
    fn build(&self, app: &mut App) {
        // A boot-time seed so any early RNG user works; replaced per world.
        app.insert_resource(GameSeed(random_seed()));
    }
}
