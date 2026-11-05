//! End-to-end flow test with no window and no renderer (spec 01 states):
//! Boot → MainMenu → Generating → WorldPreview → InGame → back, asserting
//! the world actually builds (terrain meshes exist) and state transitions
//! fire from the same inputs the player uses.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use bevy::asset::{AssetApp, AssetPlugin};
use bevy::prelude::*;
use cyclone_engine::{AppState, CurrentWorld, Settings, WorldConfig};
use cyclone_game::CycloneGamePlugin;
use cyclone_render::WorldBuilderPlugin;
use cyclone_ui::{Activated, CycloneUiPlugin, MenuFocus};
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
    flow_app_with(true)
}

/// `auto` adds the drive-through key injections (Enter at preview, Esc
/// in-game) the full-loop tests rely on; preview-level tests need the
/// flow to STOP at the preview instead.
fn flow_app_with(auto: bool) -> App {
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
    if auto {
        app.add_systems(
            Update,
            (
                press_key(KeyCode::Enter, "confirm").run_if(in_state(AppState::WorldPreview)),
                press_key(KeyCode::Escape, "back").run_if(in_state(AppState::InGame)),
            ),
        );
    }
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
    // Spec 01 C7: a state's DespawnOnExit entities must be gone once we
    // leave it — no menu screen bleeding into the game.
    let stale_in_preview = leftovers(&mut app, AppState::MainMenu);
    assert_eq!(
        stale_in_preview, 0,
        "no MainMenu entities survive in preview"
    );
    wait_for(&mut app, "InGame", Duration::from_secs(30), |a| {
        state(a) == AppState::InGame
    });
    assert_eq!(app.world().resource::<CurrentWorld>().0.seed, preview.seed);
    let stale_in_game = leftovers(&mut app, AppState::WorldPreview);
    assert_eq!(stale_in_game, 0, "no preview entities survive in InGame");
    let mut texts = app.world_mut().query::<&Text>();
    assert!(
        texts.iter(app.world()).count() >= 3,
        "corner control-hints are on screen while flying"
    );

    // Esc (injected) → MainMenu with a fresh small background.
    wait_for(&mut app, "back to MainMenu", Duration::from_secs(60), |a| {
        state(a) == AppState::MainMenu && a.world().resource::<CurrentWorld>().0.size.0 == 88
    });

    // Quit button sends AppExit; assert we're still in a sane state first.
    app.world_mut().write_message(cyclone_ui::Activated(2));
    app.update();
    assert_eq!(state(&app), AppState::MainMenu);
}

/// Spec 02 acceptance: a seed typed/copied from another machine must show
/// the identical world. Drives the *real* typing path: Play Seed button →
/// per-character KeyboardInput messages → Enter.
#[test]
fn play_seed_shows_identical_world() {
    use bevy::input::ButtonState;
    use bevy::input::keyboard::{Key, KeyboardInput};
    use cyclone_worldgen::{ProgressNoop, Seed, generate};
    use smol_str::SmolStr;

    let mut app = flow_app();
    wait_for(&mut app, "MainMenu", Duration::from_secs(30), |a| {
        state(a) == AppState::MainMenu && a.world().contains_resource::<CurrentWorld>()
    });
    // GameConfig holds the full-round config the app will generate with
    // (WorldConfig is the menu backdrop's until generation completes).
    let seed: Seed = "0000-0000-00042".parse().unwrap();
    let want =
        generate(seed, 1, &WorldGenConfig::default(), &ProgressNoop).expect("direct generate");
    let typed = seed.to_string(); // e.g. "0000-0000-00042"

    app.world_mut().write_message(cyclone_ui::Activated(1)); // Play Seed
    app.update();
    for c in typed.chars() {
        app.world_mut().write_message(KeyboardInput {
            key_code: KeyCode::KeyA, // unused by menu_seed_input
            logical_key: Key::Character(SmolStr::new(c.to_string())),
            state: ButtonState::Pressed,
            text: Some(SmolStr::new(c.to_string())),
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Enter,
        logical_key: Key::Enter,
        state: ButtonState::Pressed,
        text: Some(SmolStr::new("\r")),
        repeat: false,
        window: Entity::PLACEHOLDER,
    });
    wait_for(
        &mut app,
        "WorldPreview via typed seed",
        Duration::from_secs(60),
        |a| state(a) == AppState::WorldPreview,
    );
    let got = app.world().resource::<CurrentWorld>().0.clone();
    assert_eq!(got.seed, want.seed);
    // On completion the app republishes its real config to WorldConfig.
    let shown = app.world().resource::<WorldConfig>().0.clone();
    assert_eq!(shown.size, WorldGenConfig::default().size);
    assert_eq!(got.height, want.height, "terrain must be bit-identical");
    assert_eq!(got.crates, want.crates, "crate placement must be identical");
    assert_eq!(got.helipads, want.helipads);
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

#[test]
fn settings_panel_opens_edits_live_and_closes() {
    let mut app = flow_app();
    wait_for(&mut app, "menu", Duration::from_secs(10), |a| {
        state(a) == AppState::MainMenu && a.world().contains_resource::<MenuFocus>()
    });
    // The Settings button joined the keyboard focus list (4 buttons).
    assert_eq!(app.world().resource::<MenuFocus>().buttons.len(), 4);
    use cyclone_ui::Activated;
    app.world_mut().write_message(Activated(3)); // open
    app.update();
    app.update();
    // The opening Enter must not also flip row 0 (swallow frame).
    let fs_before = app.world().resource::<Settings>().fullscreen;
    assert!(!fs_before, "fresh panel has not edited anything");
    // Navigate to VSync (row 1) and flip it — live, in the resource.
    tap(&mut app, KeyCode::ArrowDown);
    let before = app.world().resource::<Settings>().vsync;
    tap(&mut app, KeyCode::Enter);
    assert_ne!(
        app.world().resource::<Settings>().vsync,
        before,
        "Enter on the VSync row flips it live"
    );
    // Panel open ⇒ Enter through the background focus list is ignored:
    // still in MainMenu, not New Game.
    assert_eq!(state(&app), AppState::MainMenu);
    // Mouse-only path: a click on a row writes Activated(B_ROW+row) —
    // the VSync row is 101 — and adjusts it live.
    let before = app.world().resource::<Settings>().vsync;
    app.world_mut().write_message(Activated(101));
    app.update();
    assert_ne!(
        app.world().resource::<Settings>().vsync,
        before,
        "clicking the VSync row flips it (mouse-only users)"
    );
    // Esc closes; the next Enter is the focused button again (New Game).
    tap(&mut app, KeyCode::Escape); // closes the overlay
    // The Down inside the panel did NOT move the background focus (the
    // OverlayActive marker gates focus_nav while open), so focus is
    // still on New Game: Enter leaves the menu.
    tap(&mut app, KeyCode::Enter);
    wait_for(&mut app, "left menu", Duration::from_secs(60), |a| {
        state(a) != AppState::MainMenu
    });
}

/// Press, one frame, release, one frame — imperative key tap.
fn tap(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(key);
    app.update();
}

/// Entities still alive that were scheduled to die on exiting `st`.
fn leftovers(app: &mut App, st: AppState) -> usize {
    app.world_mut()
        .query::<&DespawnOnExit<AppState>>()
        .iter(app.world())
        .filter(|d| d.0 == st)
        .count()
}

/// Spec 01 preview screen: buttons for the mouse, crates hidden, Tab
/// top-down — the parts of the preview the old keyboard-only test
/// never exercised.
#[test]
fn preview_screen_mouse_and_topdown() {
    let mut app = flow_app_with(false);
    app.init_resource::<Probes>();
    app.add_systems(
        Last,
        |rigs: Query<&cyclone_engine::CameraRig, With<Camera>>,
         crates: Query<(&cyclone_render::Crate, &Visibility)>,
         heading: Option<Res<cyclone_engine::CameraHeading>>,
         mut p: ResMut<Probes>| {
            if let Ok(r) = rigs.single() {
                p.rig_pitch = r.pitch;
            }
            let h = heading.map_or(0, |h| cyclone_render::gen_heading_of(h.0));
            p.crates = crates.iter().count();
            p.hidden = crates
                .iter()
                .filter(|(_, v)| **v == Visibility::Hidden)
                .count();
            // Crates this heading is allowed to see (occlusion truth).
            p.want = crates.iter().filter(|(c, _)| !c.hidden_from[h]).count();
        },
    );
    wait_for(&mut app, "MainMenu", Duration::from_secs(60), |a| {
        state(a) == AppState::MainMenu && a.world().contains_resource::<CurrentWorld>()
    });
    app.world_mut().write_message(Activated(0)); // New Game
    wait_for(&mut app, "preview", Duration::from_secs(60), |a| {
        state(a) == AppState::WorldPreview
    });

    // Four buttons in the preview panel, keyboard focus included.
    assert_eq!(app.world().resource::<MenuFocus>().buttons.len(), 4);

    // Crates hidden while previewing (spec 01); scene really has crates.
    assert!(
        Probes::of(&app).crates > 0 && Probes::of(&app).hidden == Probes::of(&app).crates,
        "every crate hidden on the preview"
    );

    // Tab → top-down: pitch locked near straight down.
    tap(&mut app, KeyCode::Tab);
    let pitch = Probes::of(&app).rig_pitch;
    assert!(pitch < -85.0, "top-down pitch locked (got {pitch})");
    tap(&mut app, KeyCode::Tab);
    wait_for(&mut app, "orbit back", Duration::from_secs(2), |a| {
        Probes::of(a).rig_pitch > -60.0
    });

    // Copy button must not panic headless (no clipboard resource).
    app.world_mut().write_message(Activated(22));
    app.update();
    assert_eq!(state(&app), AppState::WorldPreview);

    // Fly button → InGame; crates come back under the occlusion rule
    // (restored = visible EXCEPT the ones this heading hides by spec).
    app.world_mut().write_message(Activated(20));
    wait_for(&mut app, "InGame", Duration::from_secs(60), |a| {
        state(a) == AppState::InGame
    });
    app.update();
    let p = Probes::of(&app);
    assert!(p.crates > 0, "world has crates");
    assert_eq!(
        p.hidden,
        p.crates - p.want,
        "exactly the occluded crates are hidden — the rest restored"
    );
}

#[derive(Default, Resource)]
struct Probes {
    rig_pitch: f32,
    crates: usize,
    hidden: usize,
    want: usize,
}

impl Probes {
    fn of(app: &App) -> Probes {
        let r = app.world().resource::<Probes>();
        Probes {
            rig_pitch: r.rig_pitch,
            crates: r.crates,
            hidden: r.hidden,
            want: r.want,
        }
    }
}
