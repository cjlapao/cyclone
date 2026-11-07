//! Headless truth for the ONE behaviour no windowless box can assume:
//! when the world is replaced, the OLD world's entities must be gone.
//! (Round-174 pilot report: "cannot see a difference" — new worlds
//! stacking over old ones would look exactly like that.)

use bevy::MinimalPlugins;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use cyclone_render::WorldBuilderPlugin;
use cyclone_worldgen::{ProgressNoop, Seed, WorldGenConfig, WorldMap, generate};
use std::sync::Arc;

fn small_map(seed: &str) -> Arc<WorldMap> {
    // Any valid archipelago is a witness: the renderer only consumes
    // fields. Frozen test seeds — must generate at live dials, or a
    // generator change regressed them (round-117 contract).
    let seed: Seed = seed.parse().unwrap();
    let cfg = WorldGenConfig::default();
    match generate(seed, 1, &cfg, &ProgressNoop) {
        Ok(map) => Arc::new(map),
        Err(e) => panic!("test seed must generate at live dials: {e}"),
    }
}

#[test]
fn rebuild_replaces_old_world_entities() {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.add_plugins(WorldBuilderPlugin);
    app.insert_resource(cyclone_engine::WorldConfig(WorldGenConfig::default()));

    // Build world A.
    app.insert_resource(cyclone_engine::CurrentWorld(small_map("AAA-AAA")));
    for _ in 0..3 {
        app.update();
    }
    let meshes_a = {
        let w = app.world_mut();
        let mut q = w.query_filtered::<Entity, (With<cyclone_engine::InWorld>, With<Mesh3d>)>();
        q.iter(w).count()
    };

    // Replace with world B (reroll semantics: CurrentWorld changes).
    app.insert_resource(cyclone_engine::CurrentWorld(small_map("BBB-BBB")));
    for _ in 0..3 {
        app.update();
    }
    let inworld_b = {
        let w = app.world_mut();
        let mut q = w.query_filtered::<Entity, With<cyclone_engine::InWorld>>();
        q.iter(w).count()
    };
    eprintln!("A built {meshes_a} world meshes; after replace: {inworld_b} InWorld entities");
    assert!(meshes_a > 10, "world A built meshes");
    // One 200² world's InWorld entities (chunks+sea+props+lights) ride
    // ~200; two stacked worlds would double. Generous single-world cap:
    let one_world_max = 420;
    assert!(
        inworld_b <= one_world_max,
        "old world NOT cleared: {inworld_b} InWorld entities after replace (stacking)"
    );
}
