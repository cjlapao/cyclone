//! cyclone_engine — our engine layer on top of Bevy (spec 00).
//!
//! Owns app states, settings, the action-map input layer, the isometric
//! camera rig, the game RNG and the facade types (`CurrentWorld`, `InWorld`)
//! that let gameplay crates share data without touching `bevy::render`.

mod camera_rig;
mod input;
mod rng;
mod settings;

pub use camera_rig::{CameraHeading, CameraRig, RigFollow, RigTarget, WorldBounds};
pub use input::{Action, Actions};
pub use rng::{GameSeed, random_seed};
pub use settings::{KeyBindings, Settings};

use std::sync::Arc;

use bevy::prelude::*;
use bevy::window::{MonitorSelection, PresentMode, WindowMode};
use cyclone_worldgen::{WorldGenConfig, WorldMap};

/// Top-level app states (spec 00).
#[derive(States, Default, Clone, Eq, PartialEq, Hash, Debug)]
pub enum AppState {
    #[default]
    Boot,
    MainMenu,
    WorldPreview,
    Generating,
    InGame,
}

/// The world every screen renders, set by the generation flow (spec 01
/// "data handoff"). Arc so cheap handles are free.
#[derive(Resource, Clone)]
pub struct CurrentWorld(pub Arc<WorldMap>);

/// Marker for entities belonging to the built 3D world. Lives across
/// WorldPreview → InGame; despawned on reroll/back (spec 01).
#[derive(Component)]
pub struct InWorld;

/// The config `CurrentWorld` was generated from — the renderer needs
/// `level_height` so geometry matches the height grids (spec 02 B1).
#[derive(Resource, Clone)]
pub struct WorldConfig(pub WorldGenConfig);

/// Core plugin set wired by the binary.
pub struct CycloneEnginePlugin;

impl Plugin for CycloneEnginePlugin {
    fn build(&self, app: &mut App) {
        // Headless tests skip DefaultPlugins; own the state plumbing anyway.
        app.add_plugins(bevy::state::app::StatesPlugin)
            .init_state::<AppState>();
        app.add_plugins((
            settings::SettingsPlugin,
            input::ActionsPlugin,
            camera_rig::CameraRigPlugin,
            rng::RngPlugin,
        ));
    }
}

/// Apply window-affecting settings changes live (spec 01: "changes apply
/// instantly") and persist the file.
/// Pub so the persistence wiring has an integration test
/// (`tests/settings_persist.rs`) that the panel test can't reach.
pub fn apply_settings(
    settings: Res<Settings>,
    mut saved: Local<Option<Settings>>,
    mut windows: Query<&mut Window>,
) {
    if !settings.is_changed() || saved.as_ref() == Some(&*settings) {
        return;
    }
    *saved = Some(settings.clone());
    for mut window in &mut windows {
        window.mode = if settings.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        window
            .resolution
            .set(settings.window_width as f32, settings.window_height as f32);
        window.present_mode = if settings.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
    }
    settings.save_to_disk();
}
