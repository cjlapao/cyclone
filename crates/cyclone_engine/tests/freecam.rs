//! Proves the free camera actually flies in a live (headless) app:
//! RMB+mouse-move aims, WASD flies along the view, wheel zooms FOV, and
//! `CameraHeading` keeps tracking the free yaw for spec-02 occlusion.

use std::time::{Duration, Instant};

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use cyclone_engine::{CameraHeading, CameraRig, CycloneEnginePlugin};
use cyclone_render::{Crate, gen_heading_of};
use cyclone_worldgen::{ProgressNoop, Seed, WorldGenConfig, generate};

#[derive(Resource)]
struct HoldKey(KeyCode);

fn hold(mut kb: ResMut<ButtonInput<KeyCode>>, hold: Option<Res<HoldKey>>) {
    if let Some(h) = hold {
        kb.press(h.0);
    }
}

fn app_with_world() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CycloneEnginePlugin); // brings InputPlugin + Actions + rig
    let map =
        generate(Seed(1), 1, &WorldGenConfig::default(), &ProgressNoop).expect("world for bounds");
    app.insert_resource(CurrentWorldOf(std::sync::Arc::new(map)));
    let cam = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            CameraRig::new(),
            Projection::Perspective(PerspectiveProjection {
                fov: 50.0_f32.to_radians(),
                ..default()
            }),
        ))
        .id();
    app.insert_resource(Cam(cam));
    app.add_systems(Update, cyclone_render::crate_occlusion);
    app.add_systems(First, hold);
    app
}

#[derive(Resource)]
struct Cam(Entity);

// Tiny alias so the engine's CurrentWorld is used without prelude clashes.
use cyclone_engine::CurrentWorld as CurrentWorldOf;

fn wait_frames(app: &mut App, n: usize) {
    for _ in 0..n {
        app.update();
    }
}

#[derive(Clone, Copy)]
struct RigSnap {
    pos: Vec3,
    yaw: f32,
    pitch: f32,
    fov: f32,
    fov_target: f32,
}

fn rig(app: &App) -> RigSnap {
    let cam = app.world().resource::<Cam>().0;
    let r = app.world().get::<CameraRig>(cam).unwrap();
    RigSnap {
        pos: r.position,
        yaw: r.yaw,
        pitch: r.pitch,
        fov: r.fov,
        fov_target: r.fov_target,
    }
}

#[test]
fn free_camera_fly_look_and_zoom() {
    let mut app = app_with_world();
    // Free-fly mode needs auto_orbit == 0 (the default).
    wait_frames(&mut app, 3);

    let start_pos = rig(&app).pos;
    let start_fov = rig(&app).fov;

    // 1) W (Action::Up via KeyW binding) flies along the view. Time uses
    // the wall clock, so hold it for a real 200 ms (~7.6 units of travel).
    app.insert_resource(HoldKey(KeyCode::KeyW));
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_millis(200) {
        app.update();
    }
    let after_w = rig(&app).pos;
    assert!(
        after_w.distance(start_pos) > 0.5,
        "W should fly the camera forward: {start_pos:?} -> {after_w:?}"
    );
    app.world_mut().remove_resource::<HoldKey>();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .release(KeyCode::KeyW);

    // 2) Right-mouse held + MouseMotion aims yaw/pitch.
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Right);
    for _ in 0..3 {
        app.world_mut().write_message(MouseMotion {
            delta: Vec2::new(40.0, -20.0),
        });
        app.update();
    }
    let aimed = rig(&app);
    assert!(
        aimed.pitch > -30.0,
        "mouse up should raise the view: {}",
        aimed.pitch
    );
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Right);

    // 3) Wheel changes the FOV target (scroll down = wider).
    app.world_mut().write_message(MouseWheel {
        unit: bevy::input::mouse::MouseScrollUnit::Pixel,
        x: 0.0,
        y: -120.0,
        window: Entity::PLACEHOLDER,
        phase: bevy::input::touch::TouchPhase::Moved,
    });
    wait_frames(&mut app, 2);
    let zoomed = rig(&app);
    assert!(
        zoomed.fov_target > start_fov,
        "wheel down should widen FOV: {start_fov} -> {}",
        zoomed.fov_target
    );

    // 4) Heading resource tracks the free yaw (spec-02 occlusion).
    let heading0 = app.world().resource::<CameraHeading>().0;
    let yaw = rig(&app).yaw;
    let expect = ((yaw - 45.0) / 90.0).round().rem_euclid(4.0) as u8;
    assert_eq!(heading0, expect, "CameraHeading must follow yaw {yaw}");

    // 5) Spec 02 B4 end to end: a crate hidden from gen heading idx 0
    // must vanish exactly when the rig yaw enters that heading's sector.
    let crate_ent = app
        .world_mut()
        .spawn((
            Crate {
                // Hidden from gen idx 1 ⇒ rig cam heading 2 ⇒ yaw 225°.
                hidden_from: [false, true, false, false],
            },
            bevy::prelude::Visibility::default(),
        ))
        .id();
    let vis = |app: &App| {
        *app.world()
            .get::<bevy::prelude::Visibility>(crate_ent)
            .unwrap()
    };
    let cam = app.world().resource::<Cam>().0;
    for (yaw, want_visible) in [(135.0, true), (225.0, false), (315.0, true)] {
        app.world_mut().get_mut::<CameraRig>(cam).unwrap().yaw = yaw;
        app.update();
        app.update();
        let heading = app.world().resource::<CameraHeading>().0;
        assert_eq!(
            matches!(vis(&app), bevy::prelude::Visibility::Visible),
            want_visible,
            "crate at yaw {yaw} (cam heading {heading}, gen {})",
            gen_heading_of(heading)
        );
    }

    // 6) Bounds: parked at the edge and pushed further, the free camera
    // never crosses the padded box (map 160 + 60).
    let cam = app.world().resource::<Cam>().0;
    app.world_mut().get_mut::<CameraRig>(cam).unwrap().position = Vec3::new(80.0, 40.0, 219.5);
    app.insert_resource(HoldKey(KeyCode::KeyS));
    let t0 = Instant::now();
    while t0.elapsed() < Duration::from_millis(300) {
        app.update();
    }
    let z = rig(&app).pos.z;
    assert!(z <= 220.5, "free camera clamped: z={z}");
}
