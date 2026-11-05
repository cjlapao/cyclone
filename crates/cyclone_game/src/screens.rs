//! Screens and flow for spec 01: Boot → MainMenu → Generating → WorldPreview
//! → InGame, with reroll/seed-input/copy and off-thread generation.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use cyclone_engine::{
    Action, Actions, AppState, CameraRig, CurrentWorld, GameSeed, RigTarget, WorldConfig,
    random_seed,
};
use cyclone_ui::{Activated, MenuButton, MenuFocus, Theme, menu_button};
use cyclone_worldgen::{GenError, Progress, Seed, WorldGenConfig, WorldMap, generate};

/// Button ids (the ui crate is game-agnostic; screens own the meaning).
const B_NEW: u32 = 0;
const B_PLAY_SEED: u32 = 1;
const B_QUIT: u32 = 2;
const B_RETRY: u32 = 10;
const B_BACK: u32 = 11;

/// Config for normal rounds (assets/worldgen/default.ron if present).
#[derive(Resource)]
pub struct GameConfig(pub WorldGenConfig);

/// Seed queued for the next `Generating` run.
#[derive(Resource)]
struct PendingGen {
    seed: Seed,
}

pub fn register(app: &mut App) {
    app.insert_resource(GameConfig(WorldGenConfig::default()));
    app.add_systems(Startup, load_worldgen_config);
    app.add_systems(OnEnter(AppState::Boot), boot);

    app.add_systems(OnEnter(AppState::MainMenu), menu_build);
    app.add_systems(
        Update,
        (menu_seed_input, menu_activate).run_if(in_state(AppState::MainMenu)),
    );
    app.add_systems(OnExit(AppState::MainMenu), menu_cleanup);

    app.add_systems(OnEnter(AppState::Generating), gen_start);
    app.add_systems(
        Update,
        (gen_update, gen_activate).run_if(in_state(AppState::Generating)),
    );

    app.add_systems(OnEnter(AppState::WorldPreview), preview_build);
    app.add_systems(
        Update,
        preview_keys.run_if(in_state(AppState::WorldPreview)),
    );

    app.add_systems(OnEnter(AppState::InGame), ingame_build);
    app.add_systems(Update, ingame_keys.run_if(in_state(AppState::InGame)));
}

/// Replace defaults with `assets/worldgen/default.ron` when present (A3).
fn load_worldgen_config(mut cfg: ResMut<GameConfig>) {
    match std::fs::read_to_string("assets/worldgen/default.ron") {
        Err(_) => {}
        Ok(text) => match WorldGenConfig::from_ron(&text) {
            Ok(parsed) => {
                *cfg = GameConfig(parsed);
                info!("loaded assets/worldgen/default.ron");
            }
            Err(e) => warn!("bad assets/worldgen/default.ron, using defaults: {e}"),
        },
    }
}

// ---------------------------------------------------------------- Boot ----

fn boot(mut commands: Commands, mut next: ResMut<NextState<AppState>>) {
    // One persistent orthographic rig camera for every screen (spec 00).
    // AmbientLight is a camera-attached component in 0.20.
    commands.spawn((
        Camera3d::default(),
        CameraRig::new(),
        Projection::Orthographic(OrthographicProjection::default_3d()),
        bevy::light::AmbientLight {
            color: Color::srgb(0.75, 0.85, 1.0),
            brightness: 600.0,
            ..default()
        },
    ));
    next.set(AppState::MainMenu);
}

// ----------------------------------------------------------- Main menu ----

#[derive(Resource)]
struct MenuScreen {
    seed_mode: bool,
    seed_text: String,
    seed_label: Entity,
    error_label: Entity,
}

/// Small world for the menu/preview backdrop; every knob stays inside
/// the generator's validator (helipads ≤ islands−2, …).
pub fn menu_bg_config() -> WorldGenConfig {
    WorldGenConfig {
        size: (88, 88),
        island_count: 6,
        island_radius: (6, 12),
        base_radius: (8, 11),
        crates: 2,
        helipads_extra: (2, 3),
        ..default()
    }
}

fn menu_build(
    mut commands: Commands,
    theme: Res<Theme>,
    mut rigs: Query<&mut CameraRig>,
    mut rig_target: ResMut<RigTarget>,
) {
    // Background island: a fresh small world, generated inline (~10 ms).
    let seed = random_seed();
    let cfg = menu_bg_config();
    match generate(seed, 1, &cfg, &cyclone_worldgen::ProgressNoop) {
        Ok(map) => {
            let (w, h) = map.size;
            let centre = Vec3::new(w as f32 * 0.5, 0.0, h as f32 * 0.5);
            commands.insert_resource(WorldConfig(cfg));
            commands.insert_resource(CurrentWorld(Arc::new(map)));
            commands.insert_resource(GameSeed(seed));
            rig_target.position = centre;
            rig_target.auto_orbit = 6.0; // one turn per minute (spec 01)
            for mut cam in &mut rigs {
                cam.set_zoom(0);
            }
        }
        Err(err) => error!("menu background generation failed: {err}"),
    }

    let s = theme.scale;
    let mut buttons: Vec<Entity> = Vec::new();
    let mut seed_label = Entity::PLACEHOLDER;
    let mut error_label = Entity::PLACEHOLDER;

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            DespawnOnExit(AppState::MainMenu),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    left: Val::Px(64.0 * s),
                    top: Val::Px(48.0 * s),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Start,
                    ..default()
                },
                DespawnOnExit(AppState::MainMenu),
            ))
            .with_children(|col| {
                col.spawn(theme.title("CYCLONE RESCUE"));
                col.spawn(Node {
                    height: Val::Px(30.0 * s),
                    ..default()
                });
                buttons.push(menu_button(col, &theme, B_NEW, "New Game"));
                buttons.push(menu_button(col, &theme, B_PLAY_SEED, "Play Seed…"));
                buttons.push(menu_button(col, &theme, B_QUIT, "Quit"));
                seed_label = col
                    .spawn((
                        theme.small(String::new()),
                        DespawnOnExit(AppState::MainMenu),
                    ))
                    .id();
                error_label = col
                    .spawn((
                        Text::new(String::new()),
                        TextFont {
                            font: theme.regular.clone().into(),
                            font_size: FontSize::Px(18.0 * s),
                            ..default()
                        },
                        TextColor(Color::srgb(0.95, 0.35, 0.30)),
                        DespawnOnExit(AppState::MainMenu),
                    ))
                    .id();
            });
            // version + background seed, bottom right (spec 01)
            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(18.0 * s),
                    bottom: Val::Px(12.0 * s),
                    ..default()
                },
                theme.small(format!("v0.1 · seed {}", seed)),
                DespawnOnExit(AppState::MainMenu),
            ));
        });

    commands.insert_resource(MenuScreen {
        seed_mode: false,
        seed_text: String::new(),
        seed_label,
        error_label,
    });
    commands.insert_resource(MenuFocus { buttons, index: 0 });
}

fn menu_cleanup(mut commands: Commands) {
    commands.remove_resource::<MenuScreen>();
    commands.insert_resource(MenuFocus::default());
}

fn menu_seed_input(
    mut menu: ResMut<MenuScreen>,
    mut keys: MessageReader<KeyboardInput>,
    mut next: ResMut<NextState<AppState>>,
    mut pending: Option<ResMut<PendingGen>>,
    mut commands: Commands,
    mut texts: Query<&mut Text>,
    mut focus: ResMut<MenuFocus>,
) {
    if !menu.seed_mode {
        return;
    }
    let mut dirty = false;
    for ev in keys.read() {
        if !ev.state.is_pressed() {
            continue;
        }
        match &ev.logical_key {
            Key::Character(ch) => {
                for c in ch.chars() {
                    if c.is_ascii_alphanumeric() && menu.seed_text.len() < 16 {
                        menu.seed_text.push(c.to_ascii_uppercase());
                        dirty = true;
                    }
                }
            }
            Key::Backspace => {
                dirty |= menu.seed_text.pop().is_some();
            }
            Key::Enter => match menu.seed_text.parse::<Seed>() {
                Ok(seed) => {
                    menu.seed_mode = false;
                    match pending.as_mut() {
                        Some(p) => p.seed = seed,
                        None => commands.insert_resource(PendingGen { seed }),
                    }
                    next.set(AppState::Generating);
                    return;
                }
                Err(_) => {
                    if let Ok(mut t) = texts.get_mut(menu.error_label) {
                        *t = Text::new("Not a valid seed".to_string());
                    }
                }
            },
            Key::Escape => {
                menu.seed_mode = false;
                menu.seed_text.clear();
                dirty = true;
                if let Ok(mut t) = texts.get_mut(menu.error_label) {
                    *t = Text::new(String::new());
                }
                focus.index = 1;
            }
            _ => {}
        }
    }
    if dirty && let Ok(mut t) = texts.get_mut(menu.seed_label) {
        *t = Text::new(if menu.seed_mode {
            format!("Seed: {}▏", menu.seed_text)
        } else {
            String::new()
        });
    }
}

fn menu_activate(
    mut rx: MessageReader<Activated>,
    mut next: ResMut<NextState<AppState>>,
    mut menu: ResMut<MenuScreen>,
    mut pending: Option<ResMut<PendingGen>>,
    mut commands: Commands,
    mut texts: Query<&mut Text>,
    mut exit: MessageWriter<bevy::app::AppExit>,
) {
    for act in rx.read() {
        if menu.seed_mode {
            continue; // typing owns Enter while the seed field is open
        }
        match act.0 {
            B_NEW => {
                let seed = random_seed();
                match pending.as_mut() {
                    Some(p) => p.seed = seed,
                    None => commands.insert_resource(PendingGen { seed }),
                }
                next.set(AppState::Generating);
            }
            B_PLAY_SEED => {
                menu.seed_mode = true;
                menu.seed_text.clear();
                if let Ok(mut t) = texts.get_mut(menu.seed_label) {
                    *t = Text::new("Seed: ▏".to_string());
                }
                if let Ok(mut t) = texts.get_mut(menu.error_label) {
                    *t = Text::new(String::new());
                }
            }
            B_QUIT => {
                exit.write(bevy::app::AppExit::Success);
            }
            _ => {}
        }
    }
}

// ---------------------------------------------------------- Generating ----

/// Writer the worldgen task feeds progress into (spec 01: shared atomics).
struct SlotProgress {
    stage: Arc<Mutex<&'static str>>,
    prog: Arc<AtomicU32>,
}

impl Progress for SlotProgress {
    fn report(&self, stage: &'static str, fraction: f32) {
        *self.stage.lock().unwrap() = stage;
        self.prog.store(
            (fraction.clamp(0.0, 1.0) * 1000.0) as u32,
            Ordering::Relaxed,
        );
    }
}

#[derive(Resource)]
struct GenRunning {
    slot: Arc<Mutex<Option<Result<WorldMap, GenError>>>>,
    stage: Arc<Mutex<&'static str>>,
    prog: Arc<AtomicU32>,
    started: Instant,
    cfg: WorldGenConfig,
    seed: Seed,
    stage_label: Entity,
    fill: Entity,
    error_label: Entity,
    buttons_shown: bool,
}

fn gen_start(
    mut commands: Commands,
    pending: Option<Res<PendingGen>>,
    game_cfg: Res<GameConfig>,
    mut rig_target: ResMut<RigTarget>,
) {
    let seed = pending.map(|p| p.seed).unwrap_or_else(random_seed);
    commands.remove_resource::<PendingGen>();
    let cfg = game_cfg.0.clone();
    rig_target.auto_orbit = 2.0; // background keeps moving (spec 01)

    let slot: Arc<Mutex<Option<Result<WorldMap, GenError>>>> = Arc::new(Mutex::new(None));
    let stage: Arc<Mutex<&'static str>> = Arc::new(Mutex::new("Starting…"));
    let prog = Arc::new(AtomicU32::new(0));
    {
        let (stage_t, prog_t, cfg_t) = (stage.clone(), prog.clone(), cfg.clone());
        let slot_done = slot.clone();
        AsyncComputeTaskPool::get()
            .spawn(async move {
                let progress = SlotProgress {
                    stage: stage_t,
                    prog: prog_t,
                };
                let result = generate(seed, 1, &cfg_t, &progress);
                slot_t_result(&slot_done, result);
            })
            .detach();
    }

    let mut stage_label = Entity::PLACEHOLDER;
    let mut track = Entity::PLACEHOLDER;
    let mut error_label = Entity::PLACEHOLDER;
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            DespawnOnExit(AppState::Generating),
        ))
        .with_children(|mid| {
            mid.spawn(label_tuple(
                "GENERATING",
                Color::srgb(0.98, 0.93, 0.80),
                26.0,
            ));
            track = mid
                .spawn((
                    Node {
                        width: Val::Px(420.0),
                        height: Val::Px(10.0),
                        margin: UiRect::top(Val::Px(18.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgba(0.10, 0.13, 0.18, 0.9)),
                ))
                .id();
            // Progress fill: spawned after the closure and parented with
            // ChildOf explicitly (the spawner would force it onto `mid`).
            stage_label = mid.spawn(label_tuple("", Color::WHITE, 18.0)).id();
            error_label = mid
                .spawn(label_tuple("", Color::srgb(0.95, 0.35, 0.30), 20.0))
                .id();
        });
    let fill = commands
        .spawn((
            Node {
                width: Val::Px(0.0),
                height: Val::Px(10.0),
                ..default()
            },
            BackgroundColor(Color::srgb(0.91, 0.55, 0.13)),
            ChildOf(track),
        ))
        .id();

    commands.insert_resource(GenRunning {
        slot,
        stage,
        prog,
        started: Instant::now(),
        cfg,
        seed,
        stage_label,
        fill,
        error_label,
        buttons_shown: false,
    });
}

/// Store the generation result (split out so the closure stays readable).
fn slot_t_result(
    slot: &Arc<Mutex<Option<Result<WorldMap, GenError>>>>,
    result: Result<WorldMap, GenError>,
) {
    *slot.lock().unwrap() = Some(result);
}

/// Plain text components without the Theme resource (ChildSpawner closures
/// have no access to `Res`).
fn label_tuple(text: &str, color: Color, px: f32) -> (Text, TextFont, TextColor) {
    (
        Text::new(text.to_string()),
        TextFont {
            font_size: FontSize::Px(px),
            ..default()
        },
        TextColor(color),
    )
}

fn gen_update(
    mut commands: Commands,
    mut running: Option<ResMut<GenRunning>>,
    mut next: ResMut<NextState<AppState>>,
    mut nodes: Query<&mut Node>,
    mut texts: Query<&mut Text>,
    mut last_stage: Local<String>,
) {
    let Some(running) = running.as_mut() else {
        return;
    };
    let frac = running.prog.load(Ordering::Relaxed) as f32 / 1000.0;
    if let Ok(mut n) = nodes.get_mut(running.fill) {
        n.width = Val::Px(420.0 * frac);
    }
    let stage = *running.stage.lock().unwrap();
    if stage != *last_stage {
        *last_stage = stage.to_string();
        if let Ok(mut t) = texts.get_mut(running.stage_label) {
            *t = Text::new(stage.to_string());
        }
    }

    let finished = running.slot.lock().unwrap().take();
    let Some(result) = finished else { return };
    // Keep the screen ≥300 ms so it doesn't flash (spec 01).
    if running.started.elapsed().as_millis() < 300 {
        *running.slot.lock().unwrap() = Some(result);
        return;
    }
    match result {
        Ok(map) => {
            let cfg = running.cfg.clone();
            let seed = running.seed;
            commands.insert_resource(WorldConfig(cfg));
            commands.insert_resource(CurrentWorld(Arc::new(map)));
            commands.insert_resource(GameSeed(seed));
            next.set(AppState::WorldPreview);
        }
        Err(err) => {
            warn!("worldgen failed: {err}");
            if let Ok(mut t) = texts.get_mut(running.error_label) {
                *t = Text::new("Couldn't build a world from this seed".to_string());
            }
            if !running.buttons_shown {
                running.buttons_shown = true;
                let mut btns: Vec<Entity> = Vec::new();
                let root = commands
                    .spawn((
                        Node {
                            position_type: PositionType::Absolute,
                            left: Val::Percent(36.0),
                            top: Val::Percent(62.0),
                            flex_direction: FlexDirection::Row,
                            ..default()
                        },
                        DespawnOnExit(AppState::Generating),
                    ))
                    .id();
                commands.entity(root).with_children(|row| {
                    btns.push(plain_button(row, B_RETRY, "Retry (new seed)"));
                    btns.push(plain_button(row, B_BACK, "Back"));
                });
                commands.insert_resource(MenuFocus {
                    buttons: btns,
                    index: 0,
                });
            }
        }
    }
}

/// Button spawned from a ChildSpawner closure (no `Res<Theme>` reachable).
fn plain_button(row: &mut ChildSpawnerCommands, id: u32, label: &str) -> Entity {
    row.spawn((
        Node {
            width: Val::Px(220.0),
            height: Val::Px(48.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            margin: UiRect::horizontal(Val::Px(8.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.10, 0.13, 0.18, 0.9)),
        bevy::ui::FocusPolicy::Block,
        MenuButton { id },
    ))
    .with_child((
        Text::new(label.to_string()),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor::WHITE,
    ))
    .id()
}

fn gen_activate(
    mut rx: MessageReader<Activated>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
) {
    for act in rx.read() {
        match act.0 {
            B_RETRY => {
                commands.insert_resource(PendingGen {
                    seed: random_seed(),
                });
                // set() re-runs OnEnter schedules even for the same state.
                next.set(AppState::Generating);
            }
            B_BACK => next.set(AppState::MainMenu),
            _ => {}
        }
    }
}

// -------------------------------------------------------- World preview ----

fn preview_build(
    mut commands: Commands,
    world: Option<Res<CurrentWorld>>,
    theme: Res<Theme>,
    mut rigs: Query<&mut CameraRig>,
    mut rig_target: ResMut<RigTarget>,
) {
    let Some(world) = world else { return };
    let map = world.0.clone();
    let (w, h) = map.size;
    let s = theme.scale;

    // Fit the whole archipelago and orbit it slowly (spec 01).
    for mut rig in &mut rigs {
        rig.zoom = 2;
        rig.view = (w.max(h) as f32) * 1.10;
    }
    rig_target.position = Vec3::new(w as f32 * 0.5, 0.0, h as f32 * 0.5);
    rig_target.auto_orbit = 6.0;

    let islands = map.islands.len();
    let pads = map.helipads.len();
    let diff = format!("{:?}", map.metrics.difficulty);
    let reason = map.metrics.difficulty_reason.clone();
    let seed_disp = map.seed.to_string();

    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::FlexEnd,
                ..default()
            },
            DespawnOnExit(AppState::WorldPreview),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: Val::Percent(33.0),
                    height: Val::Percent(100.0),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::axes(Val::Px(28.0 * s), Val::Px(40.0 * s)),
                    ..default()
                },
                BackgroundColor(Color::srgba(0.05, 0.07, 0.10, 0.78)),
            ))
            .with_children(|panel| {
                panel.spawn(theme.body("WORLD PREVIEW"));
                panel.spawn(Node { height: Val::Px(16.0 * s), ..default() });
                panel.spawn(theme.body(format!("Seed  {seed_disp}")));
                panel.spawn(Node { height: Val::Px(10.0 * s), ..default() });
                panel.spawn(theme.body(format!("Islands: {islands} (Base Island first)")));
                panel.spawn(theme.body(format!("Helipads: {pads}")));
                panel.spawn(Node { height: Val::Px(10.0 * s), ..default() });
                panel.spawn(theme.body(format!("Difficulty: {diff}")));
                panel.spawn(theme.small(reason));
                panel.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                panel.spawn(theme.small(
                    "Enter — Fly · Q/E — Rotate · Wheel — Zoom\nR — Reroll · C — Copy seed · Esc — Back",
                ));
            });
        });
}

fn preview_keys(
    actions: Res<Actions>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
    world: Option<Res<CurrentWorld>>,
    clipboard: Option<ResMut<bevy::clipboard::prelude::Clipboard>>,
) {
    if actions.just_pressed(Action::Reroll) {
        commands.insert_resource(PendingGen {
            seed: random_seed(),
        });
        next.set(AppState::Generating);
    } else if actions.just_pressed(Action::Confirm) {
        next.set(AppState::InGame);
    } else if actions.just_pressed(Action::Back) {
        next.set(AppState::MainMenu);
    } else if actions.just_pressed(Action::CopySeed)
        && let Some(w) = world
        && let Some(mut cb) = clipboard
    {
        let seed = w.0.seed.to_string();
        if let Err(e) = cb.set_text(seed) {
            warn!("clipboard: {e}");
        }
    }
}

// ------------------------------------------------------------- In game ----

fn ingame_build(mut rig_target: ResMut<RigTarget>, mut rigs: Query<&mut CameraRig>) {
    rig_target.auto_orbit = 0.0;
    for mut rig in &mut rigs {
        rig.set_zoom(1);
    }
}

fn ingame_keys(actions: Res<Actions>, mut next: ResMut<NextState<AppState>>) {
    if actions.just_pressed(Action::Back) {
        next.set(AppState::MainMenu);
    }
}
