//! End-to-end flow test with no window and no renderer (spec 01 states):
//! Boot → MainMenu → Generating → WorldPreview → InGame → back, asserting
//! the world actually builds (terrain meshes exist) and state transitions
//! fire from the same inputs the player uses.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use bevy::asset::{AssetApp, AssetPlugin};
use bevy::prelude::*;
use cyclone_engine::{AppState, CurrentWorld, WorldConfig};
use cyclone_game::CycloneGamePlugin;
use cyclone_render::WorldBuilderPlugin;
use cyclone_ui::CycloneUiPlugin;
use cyclone_worldgen::WorldGenConfig;

#[derive(Resource, Default)]
struct OnceFlag {
    seen: HashSet<&'static str>,
}

#[derive(Resource)]
struct ReleaseNext(KeyCode);

/// One-shot key injector: presses for exactly one frame; `release_keys`
/// lifts it before the engine samples ButtonInput (First) → just_pressed.
fn press_key(key: KeyCode, why: &'static str) -> impl FnMut(&mut World) + Send + Sync + 'static {
    move |world: &mut World| {
        if world.resource::<OnceFlag>().seen.contains(why) {
            return;
        }
        world.resource_mut::<OnceFlag>().seen.insert(why);
        world.resource_mut::<ButtonInput<KeyCode>>().press(key);
        world.insert_resource(ReleaseNext(key));
    }
}

fn release_keys(
    mut commands: Commands,
    release: Option<Res<ReleaseNext>>,
    mut kb: ResMut<ButtonInput<KeyCode>>,
) {
    if let Some(r) = release {
        kb.release(r.0);
        commands.remove_resource::<ReleaseNext>();
    }
}

fn state(app: &App) -> AppState {
    app.world().resource::<State<AppState>>().get().clone()
}

/// Spin frames until `cond` or fail loudly.
fn wait_for(app: &mut App, what: &str, timeout: Duration, cond: impl Fn(&App) -> bool) {
    let start = Instant::now();
    while !cond(app) {
        app.update();
        if start.elapsed() > timeout {
            panic!("timeout waiting for {what} (state {:?})", state(app));
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}

fn flow_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .init_asset::<Mesh>()
        .init_asset::<Image>()
        .init_asset::<StandardMaterial>()
        .init_asset::<Font>();
    app.add_plugins((WorldBuilderPlugin, CycloneUiPlugin, CycloneGamePlugin));
    app.init_resource::<OnceFlag>();
    app.add_systems(First, release_keys);
    app.add_systems(
        Update,
        (
            press_key(KeyCode::Enter, "confirm").run_if(in_state(AppState::WorldPreview)),
            press_key(KeyCode::Escape, "back").run_if(in_state(AppState::InGame)),
        ),
    );
    app
}

#[test]
fn full_flow_offscreen() {
    let mut app = flow_app();

    // Boot hands over to MainMenu, which spawns a small background world.
    wait_for(&mut app, "MainMenu", Duration::from_secs(30), |a| {
        state(a) == AppState::MainMenu && a.world().contains_resource::<CurrentWorld>()
    });
    let menu_world = app.world().resource::<CurrentWorld>().0.clone();
    assert_eq!(menu_world.size.0, 88, "menu background is the small world");

    // "New Game" == Activated(0), the same message the button sends.
    app.world_mut().write_message(cyclone_ui::Activated(0));
    wait_for(&mut app, "WorldPreview", Duration::from_secs(60), |a| {
        state(a) == AppState::WorldPreview
    });

    let preview = app.world().resource::<CurrentWorld>().0.clone();
    assert_eq!(preview.size.0, 160, "new game uses the full config");
    assert!(preview.islands.len() >= 4);
    let mut roots = app
        .world_mut()
        .query_filtered::<Entity, With<cyclone_render::WorldRoot>>();
    assert!(
        roots.iter(app.world()).next().is_some(),
        "WorldBuilderPlugin built the scene"
    );
    let mesh_count = app.world().resource::<Assets<Mesh>>().iter().count();
    assert!(
        mesh_count > 10,
        "chunked terrain meshes exist: {mesh_count}"
    );
    let lh = app.world().resource::<WorldConfig>().0.level_height;
    assert!((lh - WorldGenConfig::default().level_height).abs() < f32::EPSILON);

    // Enter (injected) → InGame; same world stays loaded.
    wait_for(&mut app, "InGame", Duration::from_secs(30), |a| {
        state(a) == AppState::InGame
    });
    assert_eq!(app.world().resource::<CurrentWorld>().0.seed, preview.seed);

    // Esc (injected) → MainMenu with a fresh small background.
    wait_for(&mut app, "back to MainMenu", Duration::from_secs(60), |a| {
        state(a) == AppState::MainMenu && a.world().resource::<CurrentWorld>().0.size.0 == 88
    });

    // Quit button sends AppExit; assert we're still in a sane state first.
    app.world_mut().write_message(cyclone_ui::Activated(2));
    app.update();
    assert_eq!(state(&app), AppState::MainMenu);
}

/// The menu backdrop must generate for any seed — the menu hangs without
/// a world otherwise (this regression killed the flow test once already).
#[test]
fn menu_backdrop_always_generates() {
    use cyclone_worldgen::{ProgressNoop, Seed, generate};
    for k in 0..64u64 {
        let seed = Seed(k.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xC0FFEE);
        let cfg = cyclone_game::screens::menu_bg_config();
        generate(seed, 1, &cfg, &ProgressNoop).unwrap_or_else(|e| panic!("seed {seed}: {e}"));
    }
}
