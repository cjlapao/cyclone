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

/// The prewarm era: after any build trigger, the planet lands when
/// the WORKER does — poll with wall-clock patience, then three free
/// frames so deferred spawns (claim → root → children) flush.
fn settle_until(app: &mut App, what: &str, done: impl Fn(&mut App) -> bool) {
    for _ in 0..2400 {
        app.update();
        if done(app) {
            for _ in 0..3 {
                app.update();
            }
            return;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    panic!("{what}: never settled in ~2400 polls");
}

fn planet_built(app: &mut App) -> bool {
    app.world()
        .contains_resource::<cyclone_engine::TerrainFloor>()
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
    settle_until(&mut app, "planet built (worker)", planet_built);
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
    // Myth #9's wiring, witnessed: the planet world ships WITH the
    // terrain oracle (rig cannot sink); crates going 0 while the
    // oracle arrives is the swap in one line.
    assert!(
        app.world()
            .contains_resource::<cyclone_engine::TerrainFloor>(),
        "planet world installs the terrain floor"
    );
    // And the oracle speaks THIS seed's ground, not a stale one:
    // flagship witness (860,496) at seed AAA-AAA-mapped-u64 → same
    // closure the rig will call must agree with surface_height_m.
    let tf = app.world().resource::<cyclone_engine::TerrainFloor>();
    let probe = (tf.0)(860.0, 496.0);
    let truth =
        cyclone_render::continent_mesh::surface_height_m(860.0, 496.0, small_map().seed.0, 400)
            as f32;
    assert!(
        (probe - truth).abs() < 1e-3,
        "oracle ≡ surface fn for the live seed: {probe} vs {truth}"
    );
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
    settle_until(&mut app, "boot-time planet (PREVIEW gate)", planet_built);
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

#[test]
fn seed_swap_invalidates_the_continent_cache() {
    // The cache makes Esc→Enter re-entry instant — the stale-world
    // lie it could tell (reroll, re-enter, see the OLD planet) lives
    // or dies on `k.seed != map.seed.0`. Never tested until round 189.
    let _env = env_guard();
    unsafe {
        std::env::set_var("CYCLONE_CONTINENT", "1");
        std::env::set_var("CYCLONE_CONTINENT_SIZE", "400");
        std::env::set_var("CYCLONE_ISLANDS", "1");
        std::env::remove_var("CYCLONE_CONTINENT_PREVIEW");
        std::env::remove_var("CYCLONE_CONTINENT_SCALE");
    }
    let mut app = app_scaffold();
    app.insert_resource(cyclone_engine::CurrentWorld(small_map()));
    app.insert_resource(cyclone_render::ContinentLive(true)); // fly it
    settle_until(&mut app, "planet built (worker, swap test)", planet_built);
    let root_before = {
        let w = app.world_mut();
        let mut q = w.query_filtered::<Entity, With<cyclone_render::WorldRoot>>();
        let ids: Vec<_> = q.iter(w).collect();
        assert_eq!(ids.len(), 1, "one world stands");
        ids[0]
    };
    // Reroll: a DIFFERENT seed enters (flow: Generating replaces
    // CurrentWorld; same insert the real flow uses).
    let seed_b: Seed = "JJJ-KKK".parse().unwrap();
    let map_b = Arc::new(
        generate(seed_b, 1, &WorldGenConfig::default(), &ProgressNoop)
            .expect("second frozen-form seed generates"),
    );
    assert_ne!(small_map().seed.0, map_b.seed.0, "seeds truly differ");
    app.insert_resource(cyclone_engine::CurrentWorld(map_b));
    // The WAIT state, caught deterministically: one update after
    // the swap the worker task is microseconds old — poll CANNOT
    // have landed a payload — so rebuild must have returned early:
    // the OLD planet still stands, not a void, not a half-mesh.
    // (No timing dice: same-frame is_finished on a just-spawned
    // 400² build is structurally false.)
    app.update();
    let w = app.world_mut();
    let mut q = w.query_filtered::<Entity, With<cyclone_render::WorldRoot>>();
    let ids: Vec<_> = q.iter(w).collect();
    assert_eq!(
        ids.len(),
        1,
        "no void mid-reroll: the previous planet keeps the screen"
    );
    assert_eq!(
        ids[0], root_before,
        "continuity is the SAME entity, not a re-spawned copy"
    );
    settle_until(&mut app, "reroll planet built", |a| {
        let w = a.world_mut();
        let mut q = w.query_filtered::<Entity, With<cyclone_render::WorldRoot>>();
        let ids: Vec<_> = q.iter(w).collect();
        ids.len() == 1 && ids[0] != root_before
    });
    let root_after = {
        let w = app.world_mut();
        let mut q = w.query_filtered::<Entity, With<cyclone_render::WorldRoot>>();
        let ids: Vec<_> = q.iter(w).collect();
        assert_eq!(ids.len(), 1, "still exactly one world (old one despawned)");
        ids[0]
    };
    assert_ne!(
        root_before, root_after,
        "seed swap rebuilt the planet — the cache did not serve a stale world"
    );
}

#[test]
fn same_seed_reentry_respawns_from_cache_not_a_rebuild() {
    // The cache's OTHER promise (FIRST-FLIGHT: "respawns instantly"):
    // Esc→Enter with the SAME seed re-spawns the same meshes — not a
    // second 8 s build. Observable handle-for-handle: the respawned
    // children carry the identical Handle<Mesh> ids.
    let _env = env_guard();
    unsafe {
        std::env::set_var("CYCLONE_CONTINENT", "1");
        std::env::set_var("CYCLONE_CONTINENT_SIZE", "400");
        std::env::set_var("CYCLONE_ISLANDS", "1");
        std::env::remove_var("CYCLONE_CONTINENT_PREVIEW");
        std::env::remove_var("CYCLONE_CONTINENT_SCALE");
    }
    let mut app = app_scaffold();
    app.insert_resource(cyclone_engine::CurrentWorld(small_map()));
    app.insert_resource(cyclone_render::ContinentLive(true));
    settle_until(&mut app, "planet built (worker)", planet_built);
    let grab = |app: &mut App| -> std::collections::HashSet<bevy::asset::AssetId<Mesh>> {
        let w = app.world_mut();
        let mut q = w.query_filtered::<&Mesh3d, With<cyclone_engine::InWorld>>();
        q.iter(w).map(|m| m.0.id()).collect()
    };
    let first = grab(&mut app);
    assert!(!first.is_empty(), "world A meshed");
    let count_before = first.len();
    // Esc: game flips live false — the whole planet despawns…
    app.world_mut()
        .resource_mut::<cyclone_render::ContinentLive>()
        .0 = false;
    for _ in 0..3 {
        app.update();
    }
    // …and Enter: live true again, SAME seed still CurrentWorld.
    app.world_mut()
        .resource_mut::<cyclone_render::ContinentLive>()
        .0 = true;
    for _ in 0..3 {
        app.update();
    }
    let second = grab(&mut app);
    assert_eq!(second.len(), count_before, "same mesh count");
    assert_eq!(
        first, second,
        "same-seed re-entry reused the cached meshes (handle ids identical)"
    );
}

/// Asset-store leak probe: every rebuild ADDS chunk meshes, sea
/// meshes, and materials to the world's Assets<T> — and nothing in
/// the repo has ever REMOVED a superseded one (grep: zero
/// remove/retain/free_unused call sites). Bevy does not GC the
/// asset server on its own. A pilot hour of Esc/R/mid-carve-waits
/// is 100+ rebuilds of terrain meshes nobody can see.
/// This test MEASURES the growth first (probe-don't-assume:
/// 0.20's eviction policy might already save us) and asserts a
/// budget: five rerolls of small maps must not grow the mesh store
/// by more than TWO worlds' worth (live world + bounded slop).
#[test]
fn rerolls_do_not_grow_the_asset_stores() {
    let _env = env_guard();
    unsafe {
        // CLASSIC mode: the leak question is about the classic
        // rebuild path (continent has its own cache discipline).
        std::env::remove_var("CYCLONE_CONTINENT");
        std::env::remove_var("CYCLONE_CONTINENT_SIZE");
        std::env::remove_var("CYCLONE_CONTINENT_PREVIEW");
        std::env::remove_var("CYCLONE_CONTINENT_SCALE");
        std::env::remove_var("CYCLONE_ISLANDS");
    }
    let mut app = app_scaffold();
    let mesh_count =
        |a: &mut App| -> usize { a.world_mut().resource::<bevy::asset::Assets<Mesh>>().len() };
    let n0 = mesh_count(&mut app); // scaffold world
    let mut per_world = Vec::new();
    for s in 1..=5u64 {
        let seed: Seed = format!("SEED-{s:04}")
            .parse()
            .ok()
            .unwrap_or_else(|| "BBBB-BBBB".parse().unwrap());
        let map = Arc::new(
            generate(seed, 1, &WorldGenConfig::default(), &ProgressNoop)
                .expect("classic seeds generate"),
        );
        app.insert_resource(cyclone_engine::CurrentWorld(map));
        for _ in 0..6 {
            app.update();
        }
        per_world.push(mesh_count(&mut app));
    }
    println!("mesh-store size per rebuild: {n0} -> {per_world:?}");
    let peak = *per_world.iter().max().unwrap() as i64;
    let mean = per_world.iter().sum::<usize>() as f64 / per_world.len() as f64;
    // Post-sweep truth: the store holds ~ONE world, whose SIZE
    // varies with that seed's land — counts wobble up and down
    // (46, 39, ...) and a naive 'growth per step > 0' bound would
    // convict that honest wobble. The crime this test exists for
    // is LINEAR ACCUMULATION: measured pre-sweep at +236 over five
    // rerolls (~46/world). Post-sweep a leak would push the tail
    // toward 5x the mean; no honest single-world store peaks above
    // 2.5x its own mean.
    assert!(
        peak as f64 <= 2.5 * mean,
        "store peaks at {peak} vs mean {mean:.0} over 5 rerolls — \
         superseded meshes are not being freed"
    );
}

/// The classic leak test covers the classic rebuild; the CONTINENT
/// path had the sweep's most dangerous branch — the CACHE: a stale
/// cache planet must die at the claim, not haunt the store forever
/// behind 'we reuse via cache'. Two 400² seeds, claimed in sequence;
/// the store after the second claim must hold ONE planet, not two.
/// Claim oracle with no timing dice: TerrainFloor is re-inserted
/// with the NEW planet's closure, so its height at a probe point
/// differs from seed A's — a value-changing oracle, deterministic.
#[test]
fn continent_rerolls_do_not_double_the_asset_store() {
    let _env = env_guard();
    unsafe {
        std::env::set_var("CYCLONE_CONTINENT", "1");
        std::env::set_var("CYCLONE_CONTINENT_SIZE", "400");
        // PLAIN continent mode, not island mode: islands scale with
        // extent, so at 400² they are sub-tile specks and seeds are
        // sea-mostly — the sweep needs DENSE land on every seed to
        // have chunks to clear.
        std::env::remove_var("CYCLONE_ISLANDS");
        std::env::remove_var("CYCLONE_CONTINENT_PREVIEW");
        std::env::remove_var("CYCLONE_CONTINENT_SCALE");
    }
    let mut app = app_scaffold();
    let map_a = small_map();
    app.insert_resource(cyclone_engine::CurrentWorld(map_a));
    app.insert_resource(cyclone_render::ContinentLive(true));
    settle_until(&mut app, "planet A", planet_built);
    let probe = |a: &mut App| -> (usize, f32) {
        let n = a.world_mut().resource::<bevy::asset::Assets<Mesh>>().len();
        let h = a.world().resource::<cyclone_engine::TerrainFloor>().0(200.0, 200.0);
        (n, h)
    };
    let (store_a, h_a) = probe(&mut app);
    assert!(store_a >= 2, "planet A built no meshes to count: {store_a}");
    {
        let w = app.world_mut();
        let mut q = w.query::<&Mesh3d>();
        println!("CENSUS after A: Mesh3d entities = {}", q.iter(w).count());
    }
    let seed_b: Seed = "BBB-BBB".parse().unwrap();
    let map_b = Arc::new(
        generate(seed_b, 1, &WorldGenConfig::default(), &ProgressNoop).expect("second seed"),
    );
    app.insert_resource(cyclone_engine::CurrentWorld(map_b));
    settle_until(&mut app, "planet B claimed (floor oracle moved)", |a| {
        let f = a.world().resource::<cyclone_engine::TerrainFloor>().0(200.0, 200.0);
        f != h_a
    });
    // Patience: 10 more updates so any deferred apply lands, and
    // watch the count STABILISE (printed — drift is data).
    for _ in 0..10 {
        app.update();
    }
    let (store_b, h_b) = probe(&mut app);
    println!("continent store: A {store_a}, B {store_b}; floor {h_a:.1} -> {h_b:.1}");
    {
        let w = app.world_mut();
        let mut q = w.query::<&Mesh3d>();
        let ids: Vec<_> = q.iter(w).map(|m| m.id()).collect();
        println!("entity count with Mesh3d: {}", ids.len());
        let distinct: std::collections::HashSet<_> = ids.iter().collect();
        println!("distinct mesh handles on entities: {}", distinct.len());
    }
    // Two planets alive would read ~2×; the sweep's promise is that
    // B's claim swept A. Bound 1.6× leaves honest slack for A/B
    // differing chunk counts at equal SIZE (they mesh the same
    // grid: counts are expected equal), convicted accumulation.
    assert!(
        store_b <= store_a * 16 / 10 + 8,
        "stale cache planet haunts the store: {store_a} -> {store_b}"
    );
    assert!(
        store_b >= 2,
        "seed B built a sea-only world ({store_b} mesh): the sweep had \
         no chunks to prove it cleared — pick a seed with land"
    );
}
