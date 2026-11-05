//! cyclone_game — screens and flow (spec 01) plus later gameplay specs.

pub mod screens;

use bevy::prelude::*;
use cyclone_engine::CycloneEnginePlugin;

pub struct CycloneGamePlugin;

impl Plugin for CycloneGamePlugin {
    fn build(&self, app: &mut App) {
        // Owns the engine plugins; the binary must not add them again
        // (Bevy 0.20 panics on duplicate plugin types).
        app.add_plugins(CycloneEnginePlugin);
        screens::register(app);
    }
}

/// Test harness: the screens' schedules/systems *without* the engine, task
/// pool, or asset plugins — for headless unit tests that drive the state
/// machine directly (see `tests/flow.rs`).
pub fn register_test(app: &mut App) {
    screens::register(app);
}
