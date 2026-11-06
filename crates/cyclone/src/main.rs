//! Cyclone Rescue — wires plugins together, nothing else (spec 00).

use bevy::{prelude::*, window::WindowResolution};
use cyclone_engine::Settings;
use cyclone_game::CycloneGamePlugin;
use cyclone_render::WorldBuilderPlugin;
use cyclone_ui::CycloneUiPlugin;

const GAME_TITLE: &str = "Cyclone Rescue";

fn main() {
    // Open the window AT the saved settings (spec 00 C5 / spec 01 C6):
    // a restart with 1920×1080 or fullscreen on must not flash a
    // wrong-sized windowed frame before `apply_settings` resizes it.
    // apply_settings still owns live changes + save-on-change; its
    // first-frame write is now a no-op (values already match).
    let settings = Settings::load();
    let window = bevy::window::Window {
        title: GAME_TITLE.into(),
        resolution: WindowResolution::new(settings.window_width, settings.window_height),
        mode: if settings.fullscreen {
            bevy::window::WindowMode::BorderlessFullscreen(bevy::window::MonitorSelection::Current)
        } else {
            bevy::window::WindowMode::Windowed
        },
        present_mode: if settings.vsync {
            bevy::window::PresentMode::AutoVsync
        } else {
            bevy::window::PresentMode::AutoNoVsync
        },
        ..default()
    };
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(window),
        ..default()
    }));
    // CycloneGamePlugin wires the engine layer itself.
    app.add_plugins((WorldBuilderPlugin, CycloneUiPlugin, CycloneGamePlugin));
    // Dev builds measure the thing feel-reports argue about: the dev
    // HUD corner prints smoothed FPS from this store (TUNING's
    // honest-numbers rule, now in-flight as well as in-docs).
    #[cfg(feature = "dev")]
    app.add_plugins(bevy::diagnostic::FrameTimeDiagnosticsPlugin::default());
    app.run();
}
