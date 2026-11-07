//! B0004 witness (round 177): replicate the pilot's continent rebuild
//! headless and list every InWorld entity whose parent lacks
//! Visibility — the data behind their terminal flood, before guessing
//! a cause. Run ignored: it needs CYCLONE_CONTINENT in the env.
use bevy::MinimalPlugins;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use cyclone_render::WorldBuilderPlugin;
use cyclone_worldgen::{ProgressNoop, Seed, WorldGenConfig, WorldMap, generate};
use std::sync::Arc;

fn small_map() -> Arc<WorldMap> {
    let seed: Seed = "AAA-AAA".parse().unwrap();
    match generate(seed, 1, &WorldGenConfig::default(), &ProgressNoop) {
        Ok(map) => Arc::new(map),
        Err(e) => panic!("canary seed must generate at live dials: {e}"),
    }
}

#[test]
#[ignore = "witness: sets CYCLONE_CONTINENT itself"]
fn continent_rebuild_has_b0004_violations() {
    // SAFETY: single-threaded test, no other thread reads the env.
    unsafe {
        std::env::set_var("CYCLONE_CONTINENT", "1");
        std::env::set_var("CYCLONE_CONTINENT_SIZE", "256");
    }
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, AssetPlugin::default()));
    app.init_resource::<Assets<Mesh>>();
    app.init_resource::<Assets<Image>>();
    app.init_resource::<Assets<StandardMaterial>>();
    app.add_plugins(WorldBuilderPlugin);
    app.insert_resource(cyclone_engine::WorldConfig(WorldGenConfig::default()));
    app.insert_resource(cyclone_engine::CurrentWorld(small_map()));
    app.insert_resource(cyclone_render::ContinentLive(true));
    app.update();
    let w = app.world_mut();
    let mut q = w.query_filtered::<&ChildOf, With<cyclone_engine::InWorld>>();
    let mut bad = 0usize;
    let mut total = 0usize;
    let mut seen_parents = std::collections::HashSet::new();
    for child_of in q.iter(w) {
        total += 1;
        let p = child_of.parent();
        if seen_parents.insert(p) && w.get::<Visibility>(p).is_none() {
            bad += 1;
            if bad <= 4 {
                println!("PARENT WITHOUT VISIBILITY: {p:?}");
            }
        }
    }
    let roots = w
        .query_filtered::<(), With<cyclone_render::WorldRoot>>()
        .iter(w)
        .count();
    // The OTHER validator bevy registers is for GlobalTransform: a
    // child with Transform whose parent lacks Transform floods the
    // same "inconsistent behaviors" line — the pilot's 40 lines are
    // THIS shape, not Visibility (Visibility's flood was thousands;
    // the fix they pulled left this one, far smaller: the light rig,
    // sea and any non-Transform-rooted parent).
    let mut q2 = w.query_filtered::<&ChildOf, With<Transform>>();
    let mut bad_tf = 0usize;
    let mut seen2 = std::collections::HashSet::new();
    for child_of in q2.iter(w) {
        let p = child_of.parent();
        if seen2.insert(p) && !w.get::<Transform>(p).is_some() {
            bad_tf += 1;
            if bad_tf <= 4 {
                println!("PARENT WITHOUT TRANSFORM: {p:?}");
            }
        }
    }
    println!(
        "WITNESS: InWorld {total}, bad parents {bad}, bad TF parents {bad_tf}, WorldRoots {roots}"
    );
    assert_eq!(bad, 0, "no B0004 parents in continent rebuild");
    assert_eq!(bad_tf, 0, "no B0004-GlobalTransform parents either");
    unsafe {
        std::env::remove_var("CYCLONE_CONTINENT");
        std::env::remove_var("CYCLONE_CONTINENT_SIZE");
    }
}
