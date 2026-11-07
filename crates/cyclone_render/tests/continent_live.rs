//! Does the InGame flip actually BUILD the planet? The claim the
//! pilot's eyes are supposed to adjudicate — proven windowless here.
//! Own binary: process-global env must not race sibling tests.

use bevy::MinimalPlugins;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use cyclone_render::{Crate, WorldBuilderPlugin};
use cyclone_worldgen::{ProgressNoop, Seed, WorldGenConfig, WorldMap, generate};
use std::sync::Arc;

fn small_map() -> Arc<WorldMap> {
    // Frozen canary seed (round-117 contract: a frozen seed that stops
    // generating means a generator change regressed it — red is signal).
    let seed: Seed = "AAA-AAA".parse().unwrap();
    match generate(seed, 1, &WorldGenConfig::default(), &ProgressNoop) {
        Ok(map) => Arc::new(map),
        Err(e) => panic!("canary seed must generate at live dials: {e}"),
    }
}

fn app_scaffold() -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.add_plugins(WorldBuilderPlugin);
    app.insert_resource(cyclone_engine::WorldConfig(WorldGenConfig::default()));
    app
}

fn count<T: bevy::ecs::component::Component>(app: &mut App) -> usize {
    let w = app.world_mut();
    let mut q = w.query::<&T>();
    q.iter(w).count()
}

#[test]
fn live_flip_swaps_archipelago_for_planet() {
    unsafe {
        std::env::set_var("CYCLONE_CONTINENT", "1");
        std::env::set_var("CYCLONE_CONTINENT_SIZE", "400");
    }
    let mut app = app_scaffold();
    app.insert_resource(cyclone_engine::CurrentWorld(small_map()));
    for _ in 0..3 {
        app.update();
    }
    // Gate closed (menu semantics): archipelago world, props present.
    let crates_island = count::<Crate>(&mut app);
    assert!(crates_island > 0, "archipelago built with crates");

    // Enter InGame: game flips the gate (same insert as ingame_build).
    app.insert_resource(cyclone_render::ContinentLive(true));
    for _ in 0..3 {
        app.update();
    }
    // Planet built: crates belong to the old world and must be gone
    // (replace-cleans proven in world_rebuild.rs); land meshes exist.
    let crates_now = count::<Crate>(&mut app);
    let meshes = count::<Mesh3d>(&mut app);
    let roots = count::<cyclone_render::WorldRoot>(&mut app);
    eprintln!(
        "island crates {crates_island} → after flip: crates {crates_now}, meshes {meshes}, roots {roots}"
    );
    assert_eq!(
        crates_now, 0,
        "continent world has no crates (islands forgotten)"
    );
    assert!(meshes > 1, "planet spawned land meshes");
    assert_eq!(roots, 1, "exactly one world root stands");
}
