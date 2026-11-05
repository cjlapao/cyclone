//! cyclone_game — screens and flow (spec 01) plus later gameplay specs.

mod screens;

use bevy::prelude::*;
use cyclone_engine::CycloneEnginePlugin;

pub struct CycloneGamePlugin;

impl Plugin for CycloneGamePlugin {
    fn build(&self, app: &mut App) {
        // Bevy dedupes plugins by type; safe even if the binary added it.
        app.add_plugins(CycloneEnginePlugin);
        screens::register(app);
    }
}
