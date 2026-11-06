//! The no-rebuild knob file (spec 02 A3): assets/worldgen/default.ron
//! replaces built-in defaults when valid; when INVALID the game must
//! boot with defaults and warn — never die. Single test per binary:
//! it chdirs, and cargo gives each test file its own process.

use bevy::prelude::*;
use cyclone_game::screens::{GameConfig, load_worldgen_config};

#[test]
fn knob_file_valid_applies_invalid_falls_back() {
    let root = std::env::temp_dir().join(format!("cyclone-knob-{}", std::process::id()));
    let dir = root.join("assets").join("worldgen");
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_current_dir(&root).unwrap();
    let file = std::path::Path::new("assets")
        .join("worldgen")
        .join("default.ron");

    // Garbage file: defaults survive, no panic.
    std::fs::write(&file, "this is not ron {{{").unwrap();
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.insert_resource(GameConfig(cyclone_worldgen::WorldGenConfig::default()));
    app.add_systems(Update, load_worldgen_config);
    app.update();
    let cfg = app.world().resource::<GameConfig>();
    assert_eq!(
        cfg.0.island_count,
        cyclone_worldgen::WorldGenConfig::default().island_count,
        "garbage file must not change defaults"
    );

    // Valid file: numbers land, every field the file omits keeps default.
    std::fs::write(&file, "( island_count: 77 )").unwrap();
    app.update();
    let cfg = app.world().resource::<GameConfig>();
    assert_eq!(cfg.0.island_count, 77, "valid knob file must apply");
    assert_eq!(
        cfg.0.margin,
        cyclone_worldgen::WorldGenConfig::default().margin,
        "omitted fields keep defaults (struct default = partial-ron base)"
    );

    std::env::set_current_dir("/").unwrap();
    let _ = std::fs::remove_dir_all(&root);
}
