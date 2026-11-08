//! Does the InGame flip actually BUILD the planet? The claim the
//! pilot's eyes are supposed to adjudicate — proven windowless here.
//! Own binary: process-global env must not race sibling tests.

use bevy::MinimalPlugins;
use bevy::asset::AssetPlugin;
use bevy::prelude::*;
use cyclone_render::{Crate, WorldBuilderPlugin};
use cyclone_worldgen::{ProgressNoop, Seed, WorldGenConfig, WorldMap, generate};
use std::sync::Arc;

/// Env is process-global and these tests are the sole writers IN
/// THIS BINARY — but the test runner threads them concurrently, so
/// every set/read window needs the mutex (round 184: a preview test
/// mid-run made the gate-closed assertion of its sibling fail;
/// remove_var at test start can't protect a window another thread
/// opens mid-update). Poisoning = a sibling panicked with the env
/// dirty; own it and continue — each test declares its full env set
/// anyway, so a dirty start self-heals.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn env_guard() -> std::sync::MutexGuard<'static, ()> {
    ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner())
}

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
    let _env = env_guard();
    unsafe {
        std::env::set_var("CYCLONE_CONTINENT", "1");
        std::env::set_var("CYCLONE_CONTINENT_SIZE", "400");
        // Island mode through the REAL spawn path (pilot round 179):
        // the mesh must survive stamps, 1 m cells and the sea mesh.
        std::env::set_var("CYCLONE_ISLANDS", "1");
        // Explicitly NOT preview mode: this test proves the gate
        // STAYS closed until the flip.
        std::env::remove_var("CYCLONE_CONTINENT_PREVIEW");
        std::env::remove_var("CYCLONE_CONTINENT_SCALE");
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
    // B0004 flood guard: WorldRoot must carry Visibility, else every
    // auto-inserted child Visibility warns per rebuild (pilot terminal
    // drowned, round 175). This scaffold lacks the render plugin, so
    // the warning can't fire here — the COMPONENT contract still can.
    let (vis, tf) = {
        let w = app.world_mut();
        let mut q =
            w.query_filtered::<(&Visibility, &Transform), With<cyclone_render::WorldRoot>>();
        let mut v = 0usize;
        let mut t = 0usize;
        for (_vis, _tf) in q.iter(w) {
            v += 1;
            t += 1;
        }
        (v, t)
    };
    assert_eq!(
        (vis, tf),
        (1, 1),
        "WorldRoot carries Visibility AND Transform (both B0004 flood guards)"
    );
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

#[test]
fn preview_flag_flips_live_at_boot_nothing_else() {
    // Round 183 re-documented this flag from its insertion site —
    // now the doc's claim carries a test. CYCLONE_CONTINENT_PREVIEW=1
    // must make ContinentLive true WITHOUT any Enter simulation...
    // and true ONLY in that sense: the same world builds, no scale
    // anywhere (the deleted 0.2× myth: a shrunken planet would move
    // WorldBounds — extent stays size·0.25, asserted).
    let _env = env_guard();
    unsafe {
        std::env::set_var("CYCLONE_CONTINENT", "1");
        std::env::set_var("CYCLONE_CONTINENT_SIZE", "400");
        std::env::set_var("CYCLONE_CONTINENT_PREVIEW", "1");
        std::env::set_var("CYCLONE_ISLANDS", "1");
    }
    let mut app = app_scaffold();
    // Production order: the world exists BEFORE the builder system
    // first runs (screens spawn after worldgen; the render plugin
    // inserts ContinentLive(false) at add_plugins).
    app.insert_resource(cyclone_engine::CurrentWorld(small_map()));
    for _ in 0..3 {
        app.update();
    }
    assert!(
        app.world().resource::<cyclone_render::ContinentLive>().0,
        "PREVIEW flips live at boot (menu gate skipped)"
    );
    // And the planet actually BUILT via the boot path — the same
    // contract the manual-flip test proves after a simulated Enter:
    assert_eq!(
        count::<cyclone_render::Crate>(&mut app),
        0,
        "planet, no crates"
    );
    assert!(count::<Mesh3d>(&mut app) > 1, "boot path spawned meshes");
    assert_eq!(count::<cyclone_render::WorldRoot>(&mut app), 1, "one root");
    // Same-world proof: bounds must be the TRUE extent (100 m), not a
    // scaled one — if a phantom 0.2 ever returns, this goes red.
    for _ in 0..3 {
        app.update(); // let the bounds guard run under live=true
    }
    let b = app.world().resource::<cyclone_engine::WorldBounds>();
    assert_eq!(b.max.x, 100.0, "extent = size·0.25, unscaled, ever");
    unsafe { std::env::remove_var("CYCLONE_CONTINENT_PREVIEW") };
}
