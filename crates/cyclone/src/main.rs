//! Cyclone Rescue — wires plugins together, nothing else (spec 00).

use bevy::{prelude::*, window::WindowResolution};
use cyclone_game::CycloneGamePlugin;
use cyclone_render::WorldBuilderPlugin;
use cyclone_ui::CycloneUiPlugin;

const GAME_TITLE: &str = "Cyclone Rescue";

fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(bevy::window::Window {
                title: GAME_TITLE.into(),
                resolution: WindowResolution::new(1280, 800),
                ..default()
            }),
            ..default()
        }))
        // CycloneGamePlugin wires the engine layer itself.
        .add_plugins((WorldBuilderPlugin, CycloneUiPlugin, CycloneGamePlugin))
        .run();
}
