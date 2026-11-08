//! Ignored diagnostic (NOT a gate): do terrain/prop meshes ACCUMULATE
//! across preview rerolls, or does handle-drop free the old world's
//! assets? Five reroll rounds with `Assets<Mesh>` counted between.
//! Run: cargo test -p cyclone_game --features dev --test mesh_leak_probe -- --ignored --nocapture
//! DIAL STACK LAW (round 297, same as reroll_hitch): the probe measures
//! the dials the SHELL carries — CYCLONE_CONTINENT above CYCLONE_ISLANDS
//! (BOTH for the planet path; islands alone grades classic).
//! The tooth (round 297): final mesh count must return to base+5 —
//! measured dialed (continent+islands+size 400, 2026-11-08): BASE 52,
//! PEAK 55, settled 44: the old planet's meshes are FREED, so the
//! pilot's Esc-R family-hopping never accumulates a graveyard.

use std::time::{Duration, Instant};

use bevy::asset::AssetPlugin;
use bevy::image::Image;
use bevy::pbr::StandardMaterial;
use bevy::prelude::*;
use bevy::render::mesh::Mesh;
use bevy::text::Font;
use cyclone_engine::AppState;
use cyclone_game::CycloneGamePlugin;
use cyclone_render::WorldBuilderPlugin;
use cyclone_ui::{Activated, CycloneUiPlugin};

fn wait_for(app: &mut App, what: &str, want: AppState) {
    let t0 = Instant::now();
    let limit = Duration::from_secs(60);
    while app.world().resource::<State<AppState>>().get() != &want {
        app.update();
        std::thread::sleep(Duration::from_millis(2));
        assert!(
            t0.elapsed() < limit,
            "never reached {what} ({:?})",
            t0.elapsed()
        );
    }
}

#[test]
#[ignore = "manual mesh-leak probe"]
fn mesh_count_across_rerolls() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::input::InputPlugin)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Font>();
    app.add_plugins((WorldBuilderPlugin, CycloneUiPlugin, CycloneGamePlugin));

    wait_for(&mut app, "MainMenu", AppState::MainMenu);
    app.world_mut().write_message(Activated(0)); // New Game
    wait_for(&mut app, "WorldPreview", AppState::WorldPreview);

    let n = |a: &App| a.world().resource::<Assets<Mesh>>().len();
    let base = n(&app);
    println!("meshes at first preview: {base}");
    let mut peak = base;
    let mut last = base;
    let mut floor = base;
    for round in 0..5 {
        app.world_mut().write_message(Activated(21)); // B_PREROLL
        wait_for(&mut app, "WorldPreview again", AppState::WorldPreview);
        // let in-flight asset drops settle
        for _ in 0..5 {
            app.update();
        }
        let now = n(&app);
        peak = peak.max(now);
        last = now;
        floor = floor.min(now);
        println!("after reroll {round}: {now} meshes");
    }
    println!("BASE {base}  PEAK {peak}  (leak-free means post-preview count stays at ~BASE)");
    // Tooth v2 (round 297, same-day repair): v1 compared FINAL to the
    // FIRST world and went RED on a healthy classic run — base swings
    // ±20 by WORLD (boot seeds differ; even continent BASE went 33<->52
    // between identical commands). A leak is growth, so the anchor is
    // the FLOOR of settled rounds: legit world-size spread measured
    // 15 (37..52), slack 25 with margin; a real leak grows ~+30/round
    // and trips this by round three. Suspect your own instrument first.
    assert!(
        last <= floor + 25,
        "mesh leak: final {last} vs floor {floor} — old worlds are not being freed"
    );
}
