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
    Action, Actions, AppState, CameraRig, CurrentWorld, GameSeed, RigTarget, Settings, WorldConfig,
    random_seed,
};
use cyclone_ui::{Activated, MenuButton, MenuFocus, OverlayActive, Theme, menu_button};
use cyclone_worldgen::{GenError, Progress, Seed, WorldGenConfig, WorldMap, generate};

/// Button ids (the ui crate is game-agnostic; screens own the meaning).
const B_NEW: u32 = 0;
const B_PLAY_SEED: u32 = 1;
const B_SETTINGS: u32 = 3; // overlay panel (spec 01)
/// Row buttons inside the panel: clicking a row adjusts it +1 (mouse-only
/// users must be able to use the panel — spec 01 C2).
const B_ROW: u32 = 100;
const B_QUIT: u32 = 2;
const B_RETRY: u32 = 10;
const B_BACK: u32 = 11;
/// Preview screen buttons (spec 01 C2: mouse-only end to end).
const B_PFLY: u32 = 20;
const B_PREROLL: u32 = 21;
const B_PCOPY: u32 = 22;
const B_PBACK: u32 = 23;

/// Config for normal rounds (assets/worldgen/default.ron if present).
#[derive(Resource)]
pub struct GameConfig(pub WorldGenConfig);

/// Seed queued for the next `Generating` run.
#[derive(Resource)]
pub struct PendingGen {
    pub seed: Seed,
    /// True for machine-rolled seeds: on Exhausted the worker rerolls a
    /// fresh seed instead of stranding the player on the error screen
    /// (the feel pass made rejections likelier; a random seed that needs
    /// more than 8 attempts exists — waiting forever on one WAS the mac CI flake).
    /// Typed seeds stay single-shot and honest.
    pub auto: bool,
}

pub fn register(app: &mut App) {
    app.insert_resource(GameConfig(WorldGenConfig::default()));
    app.add_systems(Startup, load_worldgen_config);
    app.add_systems(OnEnter(AppState::Boot), boot);
    app.add_systems(Update, boot_hold.run_if(in_state(AppState::Boot)));

    app.add_systems(OnEnter(AppState::MainMenu), menu_build);
    app.add_systems(
        Update,
        (menu_seed_paste, menu_seed_input, menu_activate)
            .chain()
            .run_if(in_state(AppState::MainMenu)),
    );
    app.add_systems(Update, settings_toggle.run_if(in_state(AppState::MainMenu)));
    app.add_systems(OnExit(AppState::MainMenu), menu_cleanup);
    app.add_systems(Update, settings_nav.run_if(in_state(AppState::MainMenu)));

    app.add_systems(OnEnter(AppState::Generating), gen_start);
    app.add_systems(
        Update,
        (gen_update, gen_activate).run_if(in_state(AppState::Generating)),
    );

    app.init_resource::<PreviewShowProps>();
    #[cfg(feature = "dev")]
    app.add_systems(
        Update,
        preview_dev_toggle.run_if(in_state(AppState::WorldPreview)),
    );
    app.add_systems(
        OnEnter(AppState::WorldPreview),
        (drain_stale_activations, preview_build).chain(),
    );
    app.add_systems(OnExit(AppState::WorldPreview), preview_show_props);
    app.add_systems(
        Update,
        (preview_activate, preview_topdown, preview_hide_props)
            .chain()
            .run_if(in_state(AppState::WorldPreview)),
    );
    app.add_systems(
        Update,
        preview_keys.run_if(in_state(AppState::WorldPreview)),
    );

    app.add_systems(OnEnter(AppState::InGame), ingame_build);
    #[cfg(feature = "dev")]
    app.add_systems(Update, dev_fps_refresh.run_if(in_state(AppState::InGame)));
    app.add_systems(Update, ingame_keys.run_if(in_state(AppState::InGame)));
}

/// Replace defaults with `assets/worldgen/default.ron` when present (A3).
/// Two candidate paths: relative to the CWD (`cargo run`, zip run in
/// place) and relative to the executable (launchers that change the
/// CWD). First file found wins; an invalid file warns and keeps
/// built-in defaults.
pub fn load_worldgen_config(mut cfg: ResMut<GameConfig>) {
    let rel = std::path::Path::new("assets")
        .join("worldgen")
        .join("default.ron");
    let candidates = std::iter::once(rel.clone()).chain(
        std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join(&rel))),
    );
    for path in candidates {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        match WorldGenConfig::from_ron(&text) {
            Ok(parsed) => {
                *cfg = GameConfig(parsed);
                info!("loaded {path:?}");
            }
            Err(e) => warn!("bad {path:?}, using defaults: {e}"),
        }
        return;
    }
}

// ---------------------------------------------------------------- Boot ----

/// Minimum seconds the Boot placeholder is visible (spec 00 C1: show a
/// boot screen, THEN MainMenu — not an invisible same-frame handoff;
/// also hides the menu popping in while fonts/streaming assets settle).
#[derive(Resource)]
struct BootAt(f32);

const BOOT_SECONDS: f32 = 0.7;

fn boot_hold(time: Res<Time>, at: Res<BootAt>, mut next: ResMut<NextState<AppState>>) {
    if time.elapsed_secs() - at.0 >= BOOT_SECONDS {
        next.set(AppState::MainMenu);
    }
}

fn boot(mut commands: Commands, time: Res<Time>) {
    commands.insert_resource(BootAt(time.elapsed_secs()));
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
            BackgroundColor(Color::srgb(0.05, 0.07, 0.09)),
            DespawnOnExit(AppState::Boot),
        ))
        .with_children(|mid| {
            mid.spawn((
                Text::new("CYCLONE RESCUE"),
                TextFont {
                    font_size: FontSize::Px(54.0),
                    ..default()
                },
                TextColor(Color::srgb(0.85, 0.9, 1.0)),
            ));
            mid.spawn((
                Text::new("free-fly archipelagos"),
                TextFont {
                    font_size: FontSize::Px(17.0),
                    ..default()
                },
                TextColor(Color::srgb(0.45, 0.55, 0.68)),
            ));
        });
    // One persistent perspective rig camera for every screen (spec 00;
    // free-fly since d564517). AmbientLight is a camera-attached component.
    commands.spawn((
        Camera3d::default(),
        CameraRig::new(),
        Projection::Perspective(PerspectiveProjection {
            fov: 50.0_f32.to_radians(),
            aspect_ratio: 16.0 / 9.0,
            near: 0.35,
            far: 3000.0,
            ..default()
        }),
        bevy::light::AmbientLight {
            color: Color::srgb(0.75, 0.85, 1.0),
            brightness: 600.0,
            ..default()
        },
    ));
}

// ----------------------------------------------------------- Main menu ----

#[derive(Resource)]
struct MenuScreen {
    seed_mode: bool,
    seed_text: String,
    seed_label: Entity,
    error_label: Entity,
    /// In-flight Ctrl+V read; bevy 0.20 fetches the clipboard async and
    /// the handle is polled on later frames.
    paste_read: Option<bevy::clipboard::ClipboardRead>,
    /// Set by menu_seed_paste when fresh text should reach the label.
    paste_dirty: bool,
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
    let cfg = menu_bg_config();
    // Backdrop is decorative: on the rare Exhausted seed, roll on.
    let mut seed = random_seed();
    let mut bg = run_generate(seed, &cfg, &cyclone_worldgen::ProgressNoop);
    for _ in 0..6 {
        if bg.is_ok() {
            break;
        }
        seed = random_seed();
        bg = run_generate(seed, &cfg, &cyclone_worldgen::ProgressNoop);
    }
    match bg {
        Ok(map) => {
            let (w, h) = map.size;
            let centre = Vec3::new(w as f32 * 0.5, 0.0, h as f32 * 0.5);
            commands.insert_resource(WorldConfig(cfg));
            commands.insert_resource(CurrentWorld(Arc::new(map)));
            commands.insert_resource(GameSeed(seed));
            rig_target.position = centre;
            rig_target.auto_orbit = 6.0; // one turn per minute (spec 01)
            for mut cam in &mut rigs {
                cam.set_fov(50.0);
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
                buttons.push(menu_button(col, &theme, B_SETTINGS, "Settings"));
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
        paste_read: None,
        paste_dirty: false,
        seed_label,
        error_label,
    });
    commands.insert_resource(MenuFocus { buttons, index: 0 });
}

fn menu_cleanup(mut commands: Commands) {
    commands.remove_resource::<MenuScreen>();
    commands.remove_resource::<SettingsPanel>();
    commands.remove_resource::<OverlayActive>();
    commands.insert_resource(MenuFocus::default());
}

/// Ctrl/Cmd+V in the seed field: bevy 0.20 clipboard reads are async
/// (fetch_text returns a handle polled on later frames), so the fetch
/// starts here and the text lands filtered/capped like manual typing.
fn menu_seed_paste(
    mut menu: ResMut<MenuScreen>,
    kb: Res<bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>>,
    mut clipboard: Option<ResMut<bevy::clipboard::prelude::Clipboard>>,
    mut texts: Query<&mut Text>,
) {
    if !menu.seed_mode {
        return;
    }
    let ctrl = kb.pressed(bevy::input::keyboard::KeyCode::ControlLeft)
        || kb.pressed(bevy::input::keyboard::KeyCode::ControlRight)
        || kb.pressed(bevy::input::keyboard::KeyCode::SuperLeft)
        || kb.pressed(bevy::input::keyboard::KeyCode::SuperRight);
    if ctrl
        && kb
            .get_just_pressed()
            .any(|k| *k == bevy::input::keyboard::KeyCode::KeyV)
        && menu.paste_read.is_none()
        && let Some(cb) = clipboard.as_mut()
    {
        menu.paste_read = Some(cb.fetch_text());
    }
    let Some(read) = menu.paste_read.as_mut() else {
        return;
    };
    match read.poll_result() {
        Some(Ok(text)) => {
            menu.paste_read = None;
            menu.seed_text.clear();
            for c in text.chars().filter(char::is_ascii_alphanumeric) {
                if menu.seed_text.len() >= 16 {
                    break;
                }
                menu.seed_text.push(c.to_ascii_uppercase());
            }
            menu.paste_dirty = true;
        }
        Some(Err(_)) => {
            menu.paste_read = None;
            if let Ok(mut t) = texts.get_mut(menu.error_label) {
                *t = Text::new("clipboard unavailable".to_string());
            }
        }
        None => {}
    }
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
    if menu.paste_dirty {
        menu.paste_dirty = false;
        dirty = true;
    }
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
                        Some(p) => {
                            p.seed = seed;
                            p.auto = false;
                        }
                        None => commands.insert_resource(PendingGen { seed, auto: false }),
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
                    Some(p) => {
                        p.seed = seed;
                        p.auto = true;
                    }
                    None => commands.insert_resource(PendingGen { seed, auto: true }),
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

// ------------------------------------------------------------ Settings ----

/// Opens/closes the settings overlay. A separate system from
/// `menu_activate` (clippy's arg budget) with its own Activated cursor —
/// every MessageReader sees every message.
fn settings_toggle(
    mut rx: MessageReader<Activated>,
    mut panel: Option<ResMut<SettingsPanel>>,
    menu: Option<Res<MenuScreen>>,
    theme: Res<Theme>,
    mut settings: ResMut<Settings>,
    mut commands: Commands,
    mut texts: Query<&mut Text>,
) {
    for act in rx.read() {
        if menu.as_ref().is_some_and(|m| m.seed_mode) {
            continue; // typing owns input while the seed field is open
        }
        if let Some(idx) = act.0.checked_sub(B_ROW)
            && let Some(p) = panel.as_mut()
        {
            // Mouse click on a row: focus it and adjust +1.
            let row = idx as usize;
            if row >= p.rows.len() {
                continue;
            }
            p.row = row;
            let kind = p.rows[row].2;
            adjust_row(&mut settings, kind, 1);
            repaint(p, &settings, &mut texts);
            continue;
        }
        if act.0 != B_SETTINGS {
            continue;
        }
        if let Some(p) = panel.as_ref() {
            let root = p.root;
            commands.entity(root).try_despawn();
            commands.remove_resource::<SettingsPanel>();
            commands.remove_resource::<OverlayActive>();
        } else {
            open_settings_panel(&mut commands, &theme, &settings);
        }
    }
}

/// Row kinds the panel edits (volumes arrive with the audio engine,
/// spec 05 — see DECISIONS.md: no UI that lies about doing something).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RowKind {
    Fullscreen,
    Vsync,
    WindowSize,
    CamSpeed,
}

const WINDOW_SIZES: [(u32, u32); 4] = [(1280, 800), (1600, 900), (1920, 1080), (2560, 1440)];

/// Pure edit for one row; `dir` −1 / +1 (Confirm = +1 for toggles).
pub fn adjust_row(s: &mut Settings, kind: RowKind, dir: i8) {
    match kind {
        RowKind::Fullscreen => s.fullscreen = !s.fullscreen,
        RowKind::Vsync => s.vsync = !s.vsync,
        RowKind::WindowSize => {
            let cur = WINDOW_SIZES
                .iter()
                .position(|(w, h)| *w == s.window_width && *h == s.window_height)
                .unwrap_or(0);
            let n = WINDOW_SIZES.len() as i32;
            let next = (cur as i32 + dir.signum() as i32 + n) % n;
            (s.window_width, s.window_height) = WINDOW_SIZES[next as usize];
        }
        RowKind::CamSpeed => {
            s.camera_rotation_speed = (s.camera_rotation_speed + 0.2 * dir as f32).clamp(0.2, 3.0);
        }
    }
}

fn row_value(s: &Settings, kind: RowKind) -> String {
    match kind {
        RowKind::Fullscreen => on_off(s.fullscreen),
        RowKind::Vsync => on_off(s.vsync),
        RowKind::WindowSize => format!("{} × {}", s.window_width, s.window_height),
        RowKind::CamSpeed => format!("{:.1}", s.camera_rotation_speed),
    }
}

fn row_label(kind: RowKind) -> &'static str {
    match kind {
        RowKind::Fullscreen => "Fullscreen",
        RowKind::Vsync => "VSync",
        RowKind::WindowSize => "Window size",
        RowKind::CamSpeed => "Camera speed",
    }
}

fn on_off(b: bool) -> String {
    if b { "ON".into() } else { "OFF".into() }
}

#[derive(Resource)]
struct SettingsPanel {
    row: usize,
    root: Entity,
    /// True on the frame the panel opens: the Enter (or click) that
    /// opened it must not also edit a row.
    swallow: bool,
    rows: Vec<(Entity, Entity, RowKind)>, // (label, value, kind)
}

#[expect(
    deprecated,
    reason = "classic Interaction until the picking::hover migration (DECISIONS.md)"
)]
/// Spawn the overlay (spec 01: panel over the backdrop, applies live).
/// Resource changes are what `apply_settings` consumes — display changes
/// hit the window the same frame and persist to disk.
fn open_settings_panel(commands: &mut Commands, theme: &Theme, settings: &Settings) -> Entity {
    const KINDS: [RowKind; 4] = [
        RowKind::Fullscreen,
        RowKind::Vsync,
        RowKind::WindowSize,
        RowKind::CamSpeed,
    ];
    let s = theme.scale;
    let mut rows: Vec<(Entity, Entity, RowKind)> = Vec::new();
    let mut pending_rows: Vec<(Entity, RowKind)> = Vec::new();
    let root = commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Percent(30.0),
                top: Val::Percent(22.0),
                width: Val::Percent(40.0),
                min_height: Val::Px(280.0 * s),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(32.0 * s), Val::Px(28.0 * s)),
                row_gap: Val::Px(10.0 * s),
                ..default()
            },
            BackgroundColor(Color::srgba(0.04, 0.06, 0.09, 0.94)),
            bevy::ui::FocusPolicy::Block,
            DespawnOnExit(AppState::MainMenu),
        ))
        .with_children(|col| {
            col.spawn(theme.title("SETTINGS"));
            col.spawn(theme.small("changes apply live · Esc closes".to_string()));
            col.spawn(Node {
                height: Val::Px(12.0 * s),
                ..default()
            });
            for (i, k) in KINDS.iter().enumerate() {
                let arrow = if i == 0 { "▶ " } else { "  " };
                let label = col
                    .spawn((
                        Node {
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            padding: UiRect::axes(Val::Px(10.0 * s), Val::Px(6.0 * s)),
                            ..default()
                        },
                        // 0.20: ui_focus_system writes Interaction on any
                        // UI node that has one — no Button widget needed
                        // (focus.rs NodeQuery); Block makes the row a hit.
                        bevy::ui::Interaction::default(),
                        bevy::ui::FocusPolicy::Block,
                        MenuButton {
                            id: B_ROW + i as u32,
                        },
                        BackgroundColor(Color::NONE),
                        theme.small(format!("{arrow}{}", row_label(*k))),
                    ))
                    .id();
                pending_rows.push((label, *k));
            }
        })
        .id();
    // Value texts become children of their label rows (commands are
    // deferred, so ordering holds; via `commands` — not `col`, whose
    // spawn auto-inserts ChildOf(root) and would duplicate ChildOf).
    for (label, k) in pending_rows {
        let v = commands
            .spawn((ChildOf(label), theme.small(row_value(settings, k))))
            .id();
        rows.push((label, v, k));
    }
    commands.insert_resource(OverlayActive);
    commands.insert_resource(SettingsPanel {
        row: 0,
        root,
        swallow: true,
        rows,
    });
    root
}

/// Keyboard/gamepad editing of the open panel (mouse users: rows react to
/// their button hover like the menu does — values change on click focus).
fn settings_nav(
    panel: Option<ResMut<SettingsPanel>>,
    mut settings: ResMut<Settings>,
    mut texts: Query<&mut Text>,
    actions: Res<Actions>,
    mut commands: Commands,
) {
    let Some(mut panel) = panel else { return };
    if actions.just_pressed(Action::Back) {
        commands.entity(panel.root).try_despawn();
        commands.remove_resource::<SettingsPanel>();
        commands.remove_resource::<OverlayActive>();
        return;
    }
    let n = panel.rows.len();
    if actions.just_pressed(Action::Up) {
        panel.row = (panel.row + n - 1) % n;
    } else if actions.just_pressed(Action::Down) {
        panel.row = (panel.row + 1) % n;
    }
    let kind = panel.rows[panel.row].2;
    if panel.swallow {
        panel.swallow = false; // eat the Confirm that opened the panel
        repaint(&panel, &settings, &mut texts);
        return;
    }
    let dir = if actions.just_pressed(Action::Confirm) {
        1
    } else if actions.just_pressed(Action::Left) {
        -1
    } else if actions.just_pressed(Action::Right) {
        1
    } else {
        0
    };
    if dir != 0 {
        adjust_row(&mut settings, kind, dir);
    }
    repaint(&panel, &settings, &mut texts);
}

/// Arrow on the focused row + fresh values everywhere.
fn repaint(panel: &SettingsPanel, settings: &Settings, texts: &mut Query<&mut Text>) {
    for (i, (label, value, k)) in panel.rows.iter().enumerate() {
        let arrow = if i == panel.row { "▶ " } else { "  " };
        if let Ok(mut t) = texts.get_mut(*label) {
            *t = Text::new(format!("{arrow}{}", row_label(*k)));
        }
        if let Ok(mut t) = texts.get_mut(*value) {
            *t = Text::new(row_value(settings, *k));
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
        *lock_any(&self.stage) = stage;
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
    let (seed, auto) = pending.map_or((random_seed(), true), |p| (p.seed, p.auto));
    commands.remove_resource::<PendingGen>();
    let cfg = game_cfg.0.clone();
    rig_target.auto_orbit = 2.0; // background keeps moving (spec 01)

    let slot: Arc<Mutex<Option<Result<WorldMap, GenError>>>> = Arc::new(Mutex::new(None));
    let stage: Arc<Mutex<&'static str>> = Arc::new(Mutex::new("Starting…"));
    let prog = Arc::new(AtomicU32::new(0));
    {
        let (stage_t, prog_t, cfg_t) = (stage.clone(), prog.clone(), cfg.clone());
        let stage_w = stage.clone(); // reroll status line (stage_t moves into progress)
        let slot_done = slot.clone();
        // get_or_init: TaskPoolPlugin normally initialises this, but an app
        // without it (tests, custom builds) panics inside `get()`.
        AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default)
            .spawn(async move {
                let progress = SlotProgress {
                    stage: stage_t,
                    prog: prog_t,
                };
                // Auto seeds self-heal a rare Exhausted by rerolling;
                // typed seeds report honestly, exactly as entered.
                let mut seed = seed;
                let mut result = run_generate(seed, &cfg_t, &progress);
                if auto {
                    let mut rolls = 0;
                    while result.is_err() && rolls < 10 {
                        rolls += 1;
                        *stage_w.lock().unwrap() = "Rolling a luckier seed…";
                        seed = random_seed();
                        result = run_generate(seed, &cfg_t, &progress);
                    }
                }
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
/// A generator panic on the worker thread must never poison the main
/// thread's view of these channels: the slot either holds a result or
/// stays None, and a poisoned mutex still guards exactly that.
fn lock_any<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Spec 02 A5 / spec 01 error screen, made panic-proof: a generator
/// panic (the `expect("in bounds")` class of bug in 1000-seed-clean
/// code is possible) becomes a normal `Err` → the friendly Retry/Back
/// screen — instead of poisoning the mutex and stranding the progress
/// bar forever, or killing the process window.
fn run_generate(
    seed: cyclone_worldgen::Seed,
    cfg: &WorldGenConfig,
    progress: &dyn cyclone_worldgen::Progress,
) -> Result<WorldMap, GenError> {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        generate(seed, 1, cfg, progress)
    }))
    .unwrap_or_else(|_| {
        error!("generator panicked; surfacing as a generation error");
        Err(GenError::Invalid(
            "the generator panicked internally".to_string(),
        ))
    })
}

fn slot_t_result(
    slot: &Arc<Mutex<Option<Result<WorldMap, GenError>>>>,
    result: Result<WorldMap, GenError>,
) {
    *lock_any(slot) = Some(result);
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
    let stage = *lock_any(&running.stage);
    if stage != *last_stage {
        *last_stage = stage.to_string();
        if let Ok(mut t) = texts.get_mut(running.stage_label) {
            *t = Text::new(stage.to_string());
        }
    }

    let finished = lock_any(&running.slot).take();
    let Some(result) = finished else { return };
    // Keep the screen ≥300 ms so it doesn't flash (spec 01).
    if running.started.elapsed().as_millis() < 300 {
        *lock_any(&running.slot) = Some(result);
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
                *t = Text::new(
                    "Couldn't build a world — try another seed,\nor fix assets/worldgen/default.ron".to_string(),
                );
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
        // See round-20 DECISIONS: focus never inserts Interaction.
        #[expect(deprecated)]
        {
            bevy::ui::Interaction::default()
        },
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
                    auto: true,
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

/// Tab-top-down flag while the preview screen is up (spec 01).
#[derive(Resource)]
struct PreviewTopDown(bool);

/// `dev`-feature only: F3 reveals crate positions on the preview so
/// the spec-02 acceptance check ("each hidden crate disappears behind
/// a cliff from at least one heading") is actually performable.
#[derive(Default, Resource)]
struct PreviewShowProps(
    /// Only read by the cfg(dev) hide-bypass; dead in default builds.
    #[cfg_attr(not(feature = "dev"), allow(dead_code))]
    bool,
);

/// An `Activated` written the same frame a state change fires can sit
/// unconsumed until the screen is next entered — e.g. Enter at preview
/// queues Fly! behind the exit, and the next visit to preview would
/// auto-fly. Enter always starts with an empty queue.
fn drain_stale_activations(mut rx: MessageReader<Activated>) {
    for _ in rx.read() {}
}

fn preview_build(
    mut commands: Commands,
    world: Option<Res<CurrentWorld>>,
    theme: Res<Theme>,
    mut rigs: Query<&mut CameraRig>,
    mut rig_target: ResMut<RigTarget>,
) {
    commands.insert_resource(PreviewTopDown(false));
    rig_target.pitch_hold = None;
    let Some(world) = world else { return };
    let map = world.0.clone();
    let (w, h) = map.size;
    let s = theme.scale;
    let mut pbtns: Vec<Entity> = Vec::new();

    // Fit the whole archipelago and orbit it slowly (spec 01); the orbit
    // radius auto-frames from WorldBounds.
    for mut rig in &mut rigs {
        rig.set_fov(48.0);
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
                // The island/helipad/difficulty numbers describe the
                // CLASSIC map. Under the terrain-lab dial that map is
                // hidden — the world on screen is the voxel Earth with
                // neither islands, helipads, nor a classic difficulty
                // metric — so showing them is the panel describing a
                // world the pilot is not looking at: a myth rendered
                // as fact (found by the round-219 label audit).
                if cyclone_render::continent_mode().is_some() {
                    panel.spawn(
                        theme
                            .body("Terrain: CONTINENT — 1 km² voxel Earth"),
                    );
                    panel.spawn(theme.small(
                        "terrain-lab dial CYCLONE_CONTINENT=1 · the seed below carves it",
                    ));
                } else {
                    panel.spawn(theme.body(format!("Islands: {islands} (Base Island first)")));
                    panel.spawn(theme.body(format!("Helipads: {pads}")));
                    panel.spawn(Node { height: Val::Px(10.0 * s), ..default() });
                    panel.spawn(theme.body(format!("Difficulty: {diff}")));
                }
                panel.spawn(theme.small(reason));
                panel.spawn(Node {
                    flex_grow: 1.0,
                    ..default()
                });
                panel.spawn(theme.small(
                    "Enter — Fly · Q/E — Rotate · Wheel — Zoom\nR — Reroll · Tab — Top-down · C — Copy · Esc — Back",
                ));
                panel.spawn((theme.small(String::new()), CopyStatus));
                panel.spawn(Node { height: Val::Px(18.0 * s), ..default() });
                // Spec 01 C2: the whole preview usable with the mouse.
                pbtns.push(menu_button(panel, &theme, B_PFLY, "Fly!"));
                pbtns.push(menu_button(panel, &theme, B_PREROLL, "Reroll"));
                pbtns.push(menu_button(panel, &theme, B_PCOPY, "Copy seed"));
                pbtns.push(menu_button(panel, &theme, B_PBACK, "Back"));
            });
        });
    commands.insert_resource(MenuFocus {
        buttons: pbtns,
        index: 0,
    });
}

/// Crate positions are hidden on the preview (spec 01) — enforced every
/// frame so late-spawned props cannot leak, and restored on exit. The
/// The `dev`-feature F3 toggle (below) is this system's bypass.
fn preview_hide_props(
    mut vis: Query<&mut Visibility, With<cyclone_render::Crate>>,
    #[cfg(feature = "dev")] show: Res<PreviewShowProps>,
) {
    #[cfg(feature = "dev")]
    if show.0 {
        return; // F3 on: crate_occlusion alone decides visibility.
    }
    #[cfg(not(feature = "dev"))]
    let _ = &mut vis;
    for mut v in &mut vis {
        *v = Visibility::Hidden;
    }
}

/// Leaving the preview restores crate visibility for the flight.
fn preview_show_props(mut vis: Query<&mut Visibility, With<cyclone_render::Crate>>) {
    for mut v in &mut vis {
        *v = Visibility::Visible;
    }
}

/// Tab toggles the flat top-down view (spec 01; same camera posture
/// the spec-05 map screen will reuse).
fn preview_topdown(
    actions: Res<Actions>,
    mut top: ResMut<PreviewTopDown>,
    mut target: ResMut<RigTarget>,
) {
    if actions.just_pressed(Action::TopDown) {
        top.0 = !top.0;
        target.pitch_hold = if top.0 { Some(-89.5) } else { None };
    }
}

/// F3 (dev builds only) toggles crate reveal on the preview screen;
/// while revealed, spec-02 occlusion alone drives visibility, so
/// turning with Q/E demonstrates crates disappearing per heading.
#[cfg(feature = "dev")]
fn preview_dev_toggle(
    mut rx: bevy::ecs::message::MessageReader<bevy::input::keyboard::KeyboardInput>,
    mut show: ResMut<PreviewShowProps>,
    mut vis: Query<&mut Visibility, With<cyclone_render::Crate>>,
) {
    use bevy::input::keyboard::KeyCode;
    let hit = rx
        .read()
        .any(|e| e.key_code == KeyCode::F3 && e.state == bevy::input::ButtonState::Pressed);
    if hit {
        show.0 = !show.0;
        if show.0 {
            for mut v in &mut vis {
                *v = Visibility::Visible;
            }
            info!("dev: preview crate reveal ON");
        } else {
            info!("dev: preview crate reveal OFF");
        }
    }
}

/// Preview buttons (Fly/Reroll/Copy/Back) — same effects as the keys.
///
/// `CopyStatus`: the silent clipboard was untestable by the player —
/// every C/button press now reports its outcome on a status line.
/// Component marker for the preview copy-status line (exported for
/// headless tests; `flow.rs` asserts the fallback wording).
#[derive(Component)]
pub struct CopyStatus;

fn preview_activate(
    mut rx: MessageReader<Activated>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
    world: Option<Res<CurrentWorld>>,
    mut clipboard: Option<ResMut<bevy::clipboard::prelude::Clipboard>>,
    mut copy_text: Query<&mut Text, With<CopyStatus>>,
) {
    for act in rx.read() {
        match act.0 {
            B_PFLY => {
                next.set(AppState::InGame);
            }
            B_PREROLL => {
                commands.insert_resource(PendingGen {
                    seed: random_seed(),
                    auto: true,
                });
                next.set(AppState::Generating);
            }
            B_PBACK => next.set(AppState::MainMenu),
            B_PCOPY => {
                let seed = world.as_ref().map(|w| w.0.seed.to_string());
                let msg = match (seed, clipboard.as_mut()) {
                    (Some(seed), Some(cb)) => match cb.set_text(seed) {
                        Ok(()) => "copied ✓",
                        Err(e) => {
                            warn!("clipboard: {e}");
                            "copy failed"
                        }
                    },
                    _ => "clipboard unavailable",
                };
                for mut t in copy_text.iter_mut() {
                    *t = Text::new(msg.to_string());
                }
            }
            _ => {}
        }
    }
}

fn preview_keys(
    actions: Res<Actions>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
    mut sent: MessageWriter<Activated>,
) {
    if actions.just_pressed(Action::Reroll) {
        commands.insert_resource(PendingGen {
            seed: random_seed(),
            auto: true,
        });
        next.set(AppState::Generating);
    } else if actions.just_pressed(Action::Back) {
        next.set(AppState::MainMenu);
    } else if actions.just_pressed(Action::CopySeed) {
        // Same route as the button, so the CopyStatus line always answers.
        sent.write(Activated(B_PCOPY));
    }
}

// ------------------------------------------------------------- In game ----

/// Marker for the dev-HUD line that rewrites itself with smoothed FPS
/// (FrameTimeDiagnosticsPlugin is added by the binary under `dev`).
#[cfg(feature = "dev")]
#[derive(Component)]
struct DevFpsText;

/// Half-second rewrite: `{fps} fps · {ms} ms`. Reads the SAME store
/// `LogDiagnosticsPlugin` would log — no second measurement invented,
/// and `Option<Res>` so the headless harness (no diagnostics plugin)
/// runs the system as a no-op instead of panicking on a missing
/// resource. Feel-words ("looks slow") become quotable numbers mid-
/// flight; TUNING's rule, live.
#[cfg(feature = "dev")]
fn dev_fps_refresh(
    mut q: Query<&mut Text, With<DevFpsText>>,
    store: Option<Res<bevy::diagnostic::DiagnosticsStore>>,
    time: Res<Time>,
    mut acc: Local<f32>,
) {
    *acc += time.delta_secs();
    if *acc < 0.5 {
        return;
    }
    *acc = 0.0;
    let Some(store) = store else { return };
    let fps = store
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|d| d.smoothed());
    let ms = store
        .get(&bevy::diagnostic::FrameTimeDiagnosticsPlugin::FRAME_TIME)
        .and_then(|d| d.smoothed());
    for mut text in &mut q {
        *text = Text::from(match (fps, ms) {
            (Some(f), Some(m)) => format!("{f:.0} fps · {m:.1} ms frame"),
            _ => "-- fps".to_string(),
        });
    }
}

/// Where InGame drops the rig on the voxel Earth: (x, y, altitude,
/// pitch). Island mode makes the map centre OCEAN for most seeds, so
/// scan outward (golden-angle ring, ~ms, pure) for standable ground
/// — land on the FLAGSHIP (highest in reach: altitude is the view),
/// altitude clears it by 80 m (never below 120). Nothing standable
/// keeps the old centred hover with the nose down: swim to the
/// nearest island like a castaway, at least you'll SEE them.
/// Extent in metres; x/y in metres; +90 m south of the scan's land
/// find so the island sits ahead of the nose, not under the floor.
fn continent_clear(cont_size: u32, seed: u64, extent_m: f32) -> (f64, f64, f64, f32) {
    let ext = extent_m as f64 * 0.5;
    let _ = cont_size; // scan takes size in voxels for its own bounds
    let (x, y, ground) =
        cyclone_render::continent_mesh::spawn_over_land(ext, ext, seed, cont_size, ext)
            .unwrap_or((ext, ext, 0.0));
    let pitch: f32 = if ground > 0.0 { -18.0 } else { -60.0 };
    // Spawn OVER the found land (round 185: the old +90 m south
    // offset — tuned when islands filled a screen — put the rig over
    // open sea on a 1 km flagship, hovering a claim the docs didn't
    // make. Test enforces it: surface under spawn must be ≥3 m).
    (x, (ground + 80.0).max(120.0), y, pitch)
}

fn ingame_build(
    mut commands: Commands,
    world: Res<CurrentWorld>,
    seed: Res<GameSeed>,
    theme: Res<Theme>,
    mut rig_target: ResMut<RigTarget>,
    mut rigs: Query<&mut CameraRig>,
) {
    rig_target.auto_orbit = 0.0; // free fly (was: static iso view)
    let map = &world.0;
    // Dev builds announce dev terrain-lab mode BEFORE the generator
    // runs — the 8 s one-time build must not read as a freeze (the
    // window paints once this system returns).
    #[cfg(feature = "dev")]
    if std::env::var("CYCLONE_CONTINENT").is_ok() {
        eprintln!(
            "continent mode: building 1 km² voxel world (one-time, ~8–10 s — release census: 8.7 islands / 8.3 continent)…"
        );
    }
    // The renderer builds the planet only while this is true — set at
    // the one place that knows the pilot is actually flying.
    if std::env::var("CYCLONE_CONTINENT").is_ok() {
        commands.insert_resource(cyclone_render::ContinentLive(true));
    }
    // Start OVER the Base Island, archipelago ahead. Starting beyond
    // the south edge showed the user a black void past the sea plane's
    // edge and one lonely island — the intro screenshot complaint.
    let base = map
        .islands
        .first()
        .map_or((map.size.0 as f32 * 0.5, map.size.1 as f32 * 0.5), |i| {
            (i.centre.0 as f32 + 0.5, i.centre.1 as f32 + 0.5)
        });
    // Continent preview mode (CYCLONE_CONTINENT=1): the world the rig
    // lands into is the voxel-Earth heightfield, not the archipelago —
    // start centred, clearing whatever ground is actually there.
    // ONE reader of the terrain-lab dials (continent_mode owns size
    // AND clamp): the re-parse/re-clamp this duplicates lived three
    // rounds would have drifted the spawn map from the mesh map the
    // day either clamp moved (the island_mode second-reader bit,
    // generalised).
    let mode = cyclone_render::continent_mode();
    let clear = if let Some((cont_size, extent_m)) = mode {
        let _ = cont_size; // consumed below too via mode — keep the destructure honest
        continent_clear(cont_size, map.seed.0, extent_m)
    } else {
        // archipelago: the fixed launch lane over the home island
        (base.0 as f64, 26.0, base.1 as f64 + 46.0, -18.0)
    };
    for mut rig in &mut rigs {
        let (x, y, z, pitch) = clear;
        rig.position = Vec3::new(x as f32, y as f32, z as f32);
        rig.yaw = 0.0;
        rig.pitch = pitch;
        rig.set_fov(55.0);
    }

    // Corner hints until spec 05's HUD exists (free camera is new muscle).
    let s = theme.scale;
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(28.0 * s),
                bottom: Val::Px(24.0 * s),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Start,
                row_gap: Val::Px(4.0 * s),
                ..default()
            },
            bevy::ui::FocusPolicy::Pass,
            DespawnOnExit(AppState::InGame),
        ))
        .with_children(|col| {
            col.spawn(theme.small("WASD fly · Space up · Shift down · Ctrl boost".to_string()));
            col.spawn(theme.small("Hold RMB + move to aim · Wheel zoom".to_string()));
            col.spawn(theme.small(format!("Seed {} · Q/E snap view · Esc menu", seed.0)));
            // Dev builds quote the tuning numbers on the spot, so a
            // feel-report can cite the data without leaving the game
            // (TUNING.md: feel-words map to dials via these metrics).
            #[cfg(feature = "dev")]
            // Re-read the SAME single source, bound where it's used.
            if let Some((cont_size, _)) = cyclone_render::continent_mode() {
                // Square KILOMETRES, not metres: a 1000 m side is a
                // km² world — the old line quoted the side as if it
                // were the area (a 1000× unit lie).
                let km = cont_size as f64 * 0.25 / 1000.0;
                col.spawn(theme.small(format!(
                    "CONTINENT {:.2} km² · 25 cm voxels · Esc then R rerolls",
                    km * km
                )));
            } else {
                col.spawn(theme.small(format!(
                    "terracing {:.2} · {} islands · {:?}",
                    world.0.metrics.terracing,
                    world.0.islands.len(),
                    world.0.metrics.difficulty
                )));
            }
            // Live FPS line — rewrites itself, see dev_fps_refresh.
            #[cfg(feature = "dev")]
            col.spawn((theme.small("-- fps".to_string()), DevFpsText));
        });
}

fn ingame_keys(
    actions: Res<Actions>,
    mut next: ResMut<NextState<AppState>>,
    mut commands: Commands,
) {
    if actions.just_pressed(Action::Back) {
        // Esc returns to the PREVIEW of the same world (CurrentWorld is
        // untouched, so no rebuild); Back/Escape on the preview quits to
        // the menu. Round 187: the dev HUD's "R rerolls terrain" is
        // made TRUE — the preview's own R handler discards whatever
        // key arrived with it, so InGame Esc pre-arms a reroll here.
        // No PendingGen is consumed until Generating runs, so quitting
        // from the preview instead of pressing R simply abandons it
        // (the menu never reads it).
        if std::env::var("CYCLONE_CONTINENT").is_ok() {
            commands.insert_resource(PendingGen {
                seed: random_seed(),
                auto: true,
            });
        }
        next.set(AppState::WorldPreview);
    }
}

#[cfg(test)]
mod settings_tests {
    use super::*;

    #[test]
    fn adjust_row_touches_every_knob() {
        let mut s = Settings::default();
        adjust_row(&mut s, RowKind::Fullscreen, 1);
        assert!(s.fullscreen);
        adjust_row(&mut s, RowKind::Vsync, 1);
        assert!(!s.vsync); // default ON → OFF
        let w0 = s.window_width;
        adjust_row(&mut s, RowKind::WindowSize, 1);
        assert_ne!(s.window_width, w0);
        // full cycle back, and wraps from first entry going down
        for _ in 0..4 {
            adjust_row(&mut s, RowKind::WindowSize, -1);
        }
        assert_eq!(s.window_width, WINDOW_SIZES[1].0, "cycle is a loop");
        for _ in 0..2 {
            adjust_row(&mut s, RowKind::WindowSize, -1);
        }
        assert_eq!(s.window_width, WINDOW_SIZES[3].0, "wraps past the start");
        adjust_row(&mut s, RowKind::CamSpeed, 1);
        assert!(
            (s.camera_rotation_speed - (Settings::default().camera_rotation_speed + 0.2)).abs()
                < 1e-5
        );
        for _ in 0..50 {
            adjust_row(&mut s, RowKind::CamSpeed, -1);
        }
        assert!((s.camera_rotation_speed - 0.2).abs() < 1e-5, "clamped low");
    }
}

#[cfg(test)]
mod gen_panic_tests {
    use super::*;
    use cyclone_worldgen::ProgressNoop;

    /// A nonsense config must surface as `Err(Config)` THROUGH the
    /// catch_unwind wrapper (the generating screen's error path consumes
    /// it exactly like any generator error).
    #[test]
    fn wrapper_surfaces_config_errors() {
        let cfg = WorldGenConfig {
            island_count: 1, // below any legal archipelago
            ..WorldGenConfig::default()
        };
        let seed: Seed = "0000-0000-00001".parse().unwrap();
        let err = run_generate(seed, &cfg, &ProgressNoop)
            .unwrap_err()
            .to_string();
        assert!(err.contains("invalid config"), "got {err}");
    }
}

#[cfg(test)]
mod continent_clear_tests {
    use super::*;
    /// Same discipline as cyclone_render's live tests: process-global
    /// env, runner threads — any test that WRITES env in this binary
    // shares this lock (one today; siblings join by using it too).
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn flagship_spawn_and_castaway_fallback_are_honest() {
        // The spawn claims FIRST-FLIGHT makes the pilot — flagship
        // altitude, castaway nose-down — never executed by any test
        // until round 185 (the round-184 lesson: only a test proves
        // a consumer is REACHED). Env-guarded: sole writer in this
        // binary for these keys (grep-proven at authoring).
        let _guard = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        unsafe { std::env::set_var("CYCLONE_ISLANDS", "1") };
        // seed 42 (numeric — the pure scan's own unit, not the
        // Crockford UI form): flagship exists within a 1 km map.
        // Order IS (x, up, z, pitch) — the rig's own Vec3 slots;
        // destructuring (x, y, alt, ...) sampled sea at the altitude
        // and cried wolf (myth #4 of the day, mine, caught by the
        // witness line — instrumentation before introspection).
        let (x, alt, y, pitch) = continent_clear(4000, 42, 1000.0);
        assert!(
            alt >= 120.0 && pitch == -18.0,
            "flagship: {alt} m, {pitch}°"
        );
        // The spawn must hover over the land the scan FOUND (the
        // removed +90 m offset put it over sea — myth #3, caught by
        // this line's first run): sample AT the spawn, southward-
        // looking camera, flagship filling the frame below.
        let h = cyclone_render::continent_mesh::surface_height_m(x, y, 42, 4000);
        eprintln!("spawn clear ({x:.0},{y:.0}) hovers {alt:.0} m over {h:.0} m of land");
        assert!(h >= 3.0, "clear sits over the land it found: {h} m");
        // Castaway fallback: a 16 m map whose centre happens to be
        // SEA (blob interiors vary by seed — seed 42's centre at 16
        // m is inside a blob for most seeds, myth #5 of the day was
        // my premise, not the code) can reach no land in one
        // ring-sample. Search pure seeds for one; the scan's
        // determinism makes the found seed a permanent fixture.
        // Destructure in the RIG's slots — (x, UP, z, pitch) — the
        // day's fourth faceplant is naming z in the up slot; names
        // that lie are worse than no names.
        let is_castaway = |sd: u64| {
            let (px, up, pz, pitch) = continent_clear(64, sd, 16.0);
            pitch == -60.0
                && (px - 8.0).abs() < 1e-9
                && (pz - 8.0).abs() < 1e-9
                && (up - 120.0).abs() < 1e-6
        };
        let castaway = (1u64..64).find(|sd| is_castaway(*sd));
        let fallback_seed = castaway.expect("some small-seed centre is sea: fallback must exist");
        let (px, up, pz, pitch) = continent_clear(64, fallback_seed, 16.0);
        assert_eq!((px, pz), (8.0, 8.0), "centred hover");
        assert!(
            (up - 120.0).abs() < 1e-6 && pitch == -60.0,
            "nose down, never below 120"
        );
        eprintln!("castaway seed {fallback_seed}: hover {up:.0} m, pitch {pitch}°");
        unsafe { std::env::remove_var("CYCLONE_ISLANDS") };
    }
}
