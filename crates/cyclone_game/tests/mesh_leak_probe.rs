//! Ignored diagnostic (NOT a gate): do terrain/prop meshes ACCUMULATE
//! across preview rerolls, or does handle-drop free the old world's
//! assets? Five reroll rounds with `Assets<Mesh>` counted between.
//! Run: cargo test -p cyclone_game --features dev --test mesh_leak_probe -- --ignored --nocapture

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
    for round in 0..5 {
        app.world_mut().write_message(Activated(21)); // B_PREROLL
        wait_for(&mut app, "WorldPreview again", AppState::WorldPreview);
        // let in-flight asset drops settle
        for _ in 0..5 {
            app.update();
        }
        let now = n(&app);
        peak = peak.max(now);
        println!("after reroll {round}: {now} meshes");
    }
    println!("BASE {base}  PEAK {peak}  (leak-free means post-preview count stays at ~BASE)");
}
