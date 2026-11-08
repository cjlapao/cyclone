//! Free camera rig (user-directed evolution of spec 00's iso rig).
//!
//! The camera is a perspective camera with a position in space, yaw/pitch
//! aim, and three usage modes driven by data, not code paths:
//!
//! * **Orbit** (menus, world preview): `RigTarget::auto_orbit != 0.0` — the
//!   rig orbits `RigTarget::position` at an auto-framed radius.
//! * **Chase-orbit**: a `RigFollow` entity exists — orbit around it, keeping
//!   the user's aim (the future helicopter controller spawns one).
//! * **Free fly** (in game): WASD fly along the view, Space/Shift altitude,
//!   Left-Ctrl boost, hold **Right Mouse** + move to aim, wheel = FOV zoom.
//!
//! `CameraHeading` (0..=3) keeps tracking the nearest quadrant of the free
//! yaw, so spec 02's heading-based crate occlusion stays live.

use std::f32::consts::FRAC_PI_2;

use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;

use crate::input::Action;
use crate::settings::Settings;
use crate::{Actions, CurrentWorld};

/// Nearest canonical heading of the free yaw (0 = looking down -Z, i.e.
/// yaw 45°, matching the original isometric headings 45+90·h).
#[derive(Resource, Default)]
pub struct CameraHeading(pub u8);

/// Vertical FOV limits for the wheel zoom.
pub const FOV_MIN: f32 = 25.0;
pub const FOV_MAX: f32 = 80.0;

const SNAP_SECONDS: f32 = 0.3; // Q/E 90° turn easing
const ORBIT_PITCH: f32 = -32.0; // menu/preview arcade tilt, degrees
const PITCH_LIMIT: f32 = 85.0;
const FLY_BASE: f32 = 38.0; // world units / second at fov 45
const BOOST_X: f32 = 4.5;
const LOOK_SENS: f32 = 0.16; // degrees per pixel at rotation_speed 1
const FREE_PAD: f32 = 60.0; // how far off-map the free camera may go
const HOVER_DIST: f32 = 60.0; // chase-orbit distance to a RigFollow entity

#[derive(Component)]
pub struct CameraRig {
    /// Camera position in world space (XZ ground plane, Y up).
    pub position: Vec3,
    /// Yaw about +Y in degrees (YXZ euler; 0 looks down -Z).
    pub yaw: f32,
    /// Pitch about local X in degrees; negative looks down.
    pub pitch: f32,
    /// Smoothed vertical FOV in degrees; `fov_target` is what input sets.
    pub fov: f32,
    /// What the wheel/keys ask for; `fov` eases toward it.
    pub fov_target: f32,
    /// Current orbit radius (smoothed); 0 until an orbit ever runs.
    pub orbit_radius: f32,
    snap_t: f32,
    snap_from: f32,
    snap_to: f32,
}

impl Default for CameraRig {
    fn default() -> Self {
        Self::new()
    }
}

impl CameraRig {
    pub fn new() -> Self {
        Self {
            // Anywhere sensible before the first screen places it.
            position: Vec3::new(44.0, 55.0, 130.0),
            yaw: 0.0,
            pitch: -30.0,
            fov: 50.0,
            fov_target: 50.0,
            orbit_radius: 0.0,
            snap_t: 1.0,
            snap_from: 0.0,
            snap_to: 0.0,
        }
    }

    /// Set the (smoothed) vertical FOV target.
    pub fn set_fov(&mut self, fov: f32) {
        self.fov_target = fov.clamp(FOV_MIN, FOV_MAX);
    }

    /// Q/E: eased 90° yaw snap (also keeps spec-02 occlusion demoing).
    pub fn turn(&mut self, dir: i8, heading: &mut CameraHeading) {
        heading.0 = (heading.0 as i32 + dir as i32).rem_euclid(4) as u8;
        self.snap_from = self.yaw;
        self.snap_to = self.yaw + dir as f32 * 90.0;
        self.snap_t = 0.0;
    }

    /// Mouse-look delta in degrees; also shifts an in-flight snap target.
    pub fn apply_look(&mut self, d_yaw: f32, d_pitch: f32) {
        if self.snap_t < 1.0 {
            self.snap_from += d_yaw;
            self.snap_to += d_yaw;
        } else {
            self.yaw += d_yaw;
        }
        self.pitch = (self.pitch + d_pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// The rig's orientation quaternion (YXZ euler, degrees in).
    pub fn rotation(&self) -> Quat {
        Quat::from_euler(
            EulerRot::YXZ,
            self.yaw.to_radians(),
            self.pitch.to_radians(),
            0.0,
        )
    }

    /// Unit view direction.
    pub fn forward(&self) -> Vec3 {
        self.rotation() * Vec3::NEG_Z
    }
}

/// Optional explicit follow target (entity the camera orbits; the
/// helicopter spawns one when spec 03 lands).
#[derive(Component)]
pub struct RigFollow(pub Entity);

/// Terrain height oracle (metres → metres): the world's own surface
/// under any XZ. Inserted by the world builder for voxel worlds;
/// ABSENT means "flat floor at the box limit" — the archipelago
/// keeps its historic y ≥ 2.5 exactly (deliberate scope: the pivot
/// world gets collision first, nothing regresses on the old one).
/// Round 195: free-fly's flat y ≥ 2.5 floor let the camera plough
/// through a 270 m mountain — spec 03 promised 'never sinks'; this
/// makes the promise real.
#[derive(Resource)]
pub struct TerrainFloor(pub std::sync::Arc<dyn Fn(f32, f32) -> f32 + Send + Sync>);

/// What the rig keeps between skid and surface.
pub const MIN_GROUND_CLEAR: f32 = 2.5;

/// Orbit centre + auto-orbit rate (menu background, world preview).
/// `auto_orbit != 0` selects orbit mode; 0 selects free fly.
#[derive(Resource)]
pub struct RigTarget {
    pub position: Vec3,
    /// Degrees per second added to yaw while orbiting.
    pub auto_orbit: f32,
    /// Some(p): orbit mode locks pitch to `p` degrees and stops the
    /// auto-orbit spin — the Tab top-down map of spec 01. Orbit mode is
    /// selected even when `auto_orbit` is 0 while this is set.
    pub pitch_hold: Option<f32>,
}

impl Default for RigTarget {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            auto_orbit: 0.0,
            pitch_hold: None,
        }
    }
}

/// World rectangle in tile units, refreshed when a world loads.
#[derive(Debug, Resource, Default)]
pub struct WorldBounds {
    pub min: Vec2,
    pub max: Vec2,
}

fn refresh_bounds(world: Res<CurrentWorld>, mut bounds: ResMut<WorldBounds>) {
    if world.is_changed() {
        let (w, h) = world.0.size;
        bounds.min = Vec2::ZERO;
        bounds.max = Vec2::new(w as f32, h as f32);
    }
}

/// Mouse-look, wheel zoom, 90° snaps (run before the fly system).
// 9 small parameters by design: the aim system *is* the fan-in of mouse,
// settings, camera and cursor state; splitting would only scatter it.
#[expect(clippy::too_many_arguments, reason = "fan-in system, see doc")]
fn rig_look(
    mut rigs: Query<&mut CameraRig, With<Camera>>,
    actions: Res<Actions>,
    mut heading: ResMut<CameraHeading>,
    settings: Res<Settings>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    mut motion: MessageReader<MouseMotion>,
    mut wheel: MessageReader<MouseWheel>,
    target: Res<RigTarget>,
    mut cursors: Query<&mut bevy::window::CursorOptions>,
) {
    let mut turn = 0i8;
    if actions.just_pressed(Action::RotateCamLeft) {
        turn -= 1;
    }
    if actions.just_pressed(Action::RotateCamRight) {
        turn += 1;
    }

    // Aim: hold Right Mouse + move. While held in free-fly mode the OS
    // cursor is grabbed (Locked where supported, X11 falls back to
    // Confined) — without this, aiming died at the window edge once the
    // cursor ran out of screen (the classic free-cam complaint).
    let rmb_held = mouse
        .as_deref()
        .is_some_and(|b| b.pressed(MouseButton::Right));
    let grab = rmb_held && target.auto_orbit == 0.0;
    for mut c in &mut cursors {
        c.grab_mode = if grab {
            bevy::window::CursorGrabMode::Locked
        } else {
            bevy::window::CursorGrabMode::None
        };
        c.visible = !grab;
    }
    let mut look = Vec2::ZERO;
    let sens = LOOK_SENS * settings.camera_rotation_speed.clamp(0.1, 4.0);
    for m in motion.read() {
        if rmb_held {
            look += m.delta;
        }
    } // drained always so releasing never queues stale motion
    look *= sens;

    // Wheel: FOV zoom.
    let mut wheel_y = 0.0f32;
    for e in wheel.read() {
        wheel_y += e.y;
    }

    for mut rig in &mut rigs {
        if turn != 0 {
            rig.turn(turn, &mut heading);
        }
        if look != Vec2::ZERO {
            // Mouse right → yaw right (yaw decreases about +Y).
            // MouseMotion.delta.y is positive DOWN (XInput2 screen axis),
            // so negate: mouse up → pitch increases → look up.
            rig.apply_look(-look.x, -look.y);
        }
        if wheel_y != 0.0 {
            let f = rig.fov_target * (-wheel_y * 0.14).exp();
            rig.set_fov(f);
        }
        // Keep the spec-02 heading resource in step with the free yaw.
        heading.0 = ((rig.yaw - 45.0) / 90.0).round().rem_euclid(4.0) as u8;
    }
}

/// WASD/Space/Shift free flight along the view; idle while orbiting.
fn rig_fly(
    time: Res<Time>,
    mut rigs: Query<&mut CameraRig, With<Camera>>,
    actions: Res<Actions>,
    target: Res<RigTarget>,
) {
    if target.auto_orbit != 0.0 {
        return; // orbit mode — menus and preview own the camera
    }
    let dt = time.delta_secs();
    for mut rig in &mut rigs {
        let mut dir = Vec3::ZERO;
        if actions.pressed(Action::Up) {
            dir += rig.forward();
        }
        if actions.pressed(Action::Down) {
            dir -= rig.forward();
        }
        // Strafe stays in the horizontal plane: yaw-only, so looking up
        // or down never drags the camera vertically while sidestepping.
        let yaw_only = Quat::from_rotation_y(rig.yaw.to_radians());
        if actions.pressed(Action::Right) {
            dir += yaw_only * Vec3::X;
        }
        if actions.pressed(Action::Left) {
            dir -= yaw_only * Vec3::X;
        }
        if actions.pressed(Action::Climb) {
            dir += Vec3::Y;
        }
        if actions.pressed(Action::Descend) {
            dir -= Vec3::Y;
        }
        if dir.length_squared() == 0.0 {
            continue;
        }
        let boost = if actions.pressed(Action::Boost) {
            BOOST_X
        } else {
            1.0
        };
        let speed = FLY_BASE * boost * (rig.fov / 45.0);
        rig.position += dir.normalize_or_zero() * speed * dt;
    }
}

fn rig_update(
    time: Res<Time>,
    mut rigs: Query<(&mut CameraRig, &mut Transform, &mut Projection), With<Camera>>,
    follows: Query<&RigFollow>,
    transforms: Query<&GlobalTransform>,
    target: Res<RigTarget>,
    bounds: Res<WorldBounds>,
    floor: Option<Res<TerrainFloor>>,
) {
    let dt = time.delta_secs();
    let k_fast = 1.0 - (-10.0 * dt).exp(); // fov smoothing
    let k_follow = 1.0 - (-6.0 * dt).exp(); // chase-orbit damping
    let max_dim = (bounds.max.x - bounds.min.x).max(bounds.max.y - bounds.min.y);

    for (mut rig, mut transform, mut proj) in &mut rigs {
        // 90° snap easing.
        if rig.snap_t < 1.0 {
            rig.snap_t = (rig.snap_t + dt / SNAP_SECONDS).min(1.0);
            let t = rig.snap_t;
            let ease = t * t * (3.0 - 2.0 * t);
            rig.yaw = rig.snap_from + (rig.snap_to - rig.snap_from) * ease;
        }

        let follow = follows
            .iter()
            .next()
            .and_then(|f| transforms.get(f.0).ok().map(|g| g.translation()));

        if target.auto_orbit != 0.0 || target.pitch_hold.is_some() || follow.is_some() {
            // Orbit mode: script the position around the centre.
            // pitch_hold (top-down view) kills the spin and locks pitch.
            let pitch = target.pitch_hold.unwrap_or(ORBIT_PITCH);
            if target.pitch_hold.is_none() {
                rig.yaw += target.auto_orbit * dt;
            }
            let want_r = if follow.is_some() {
                HOVER_DIST
            } else {
                max_dim * 1.45
            };
            rig.orbit_radius = if rig.orbit_radius <= 0.0 {
                want_r // first frame: no fly-in from radius 0
            } else {
                rig.orbit_radius + (want_r - rig.orbit_radius) * k_follow
            };
            let centre = follow.unwrap_or(target.position);
            let rot =
                Quat::from_euler(EulerRot::YXZ, rig.yaw.to_radians(), pitch.to_radians(), 0.0);
            let want = centre + rot * Vec3::new(0.0, 0.0, rig.orbit_radius);
            rig.position = if follow.is_some() {
                rig.position.lerp(want, k_follow)
            } else {
                want
            };
            rig.pitch = pitch;
        } else {
            // Free fly: generous but absolute bounds so the user cannot
            // get permanently lost (map + 60 units, y 2.5..600).
            let lo = Vec3::new(bounds.min.x - FREE_PAD, 2.5, bounds.min.y - FREE_PAD);
            let hi = Vec3::new(bounds.max.x + FREE_PAD, 600.0, bounds.max.y + FREE_PAD);
            rig.position = rig.position.clamp(lo, hi);
            // Terrain floor: the box is a cage, NOT the ground —
            // mountains live inside it (myth #9: 'never sinks' was
            // prose until this line existed).
            if let Some(f) = &floor {
                let gy = f.0(rig.position.x, rig.position.z);
                rig.position.y = rig.position.y.max(gy + MIN_GROUND_CLEAR);
            }
        }

        rig.fov += (rig.fov_target - rig.fov) * k_fast;

        transform.rotation = rig.rotation();
        transform.translation = rig.position;

        if let Projection::Perspective(persp) = &mut *proj {
            persp.fov = rig.fov.to_radians().min(FRAC_PI_2 * 0.98);
            persp.near = 0.35;
            persp.far = 3000.0;
        }
    }
}

pub struct CameraRigPlugin;

impl Plugin for CameraRigPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CameraHeading>()
            .init_resource::<RigTarget>()
            .init_resource::<WorldBounds>();
        // Only when a world exists (avoids needing the resource at boot).
        app.add_systems(
            Update,
            refresh_bounds
                .run_if(bevy::ecs::schedule::common_conditions::resource_exists::<CurrentWorld>),
        );
        app.add_systems(Update, (rig_look, rig_fly, rig_update).chain());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Myth #9's epitaph: free-fly's box floor (y ≥ 2.5) let the
    /// camera plough THROUGH terrain — 'the rig never sinks' was
    /// spec prose with no code. The TerrainFloor oracle now holds
    /// the skid above the surface, headless-tested: place a rig
    /// inside a flat 100 m plateau, run a frame, it stands at
    /// 102.5 m — never lower, no input needed.
    #[test]
    fn terrain_floor_holds_the_rig_above_the_surface() {
        let mut app = App::new();
        app.add_plugins((
            bevy::MinimalPlugins,
            bevy::input::InputPlugin,
            crate::input::ActionsPlugin,
            CameraRigPlugin,
        ));
        app.insert_resource(crate::settings::Settings::default());
        app.insert_resource(TerrainFloor(std::sync::Arc::new(|_x, _z| 100.0)));
        app.world_mut().spawn((
            Camera::default(),
            Projection::default(),
            CameraRig {
                position: Vec3::new(10.0, 10.0, 10.0),
                ..default()
            },
            Transform::from_xyz(10.0, 10.0, 10.0),
        ));
        app.update();
        let mut q = app.world_mut().query::<(&CameraRig, &Transform)>();
        let (rig, t) = q.single(app.world()).unwrap();
        let _ = rig;
        assert!(
            (t.translation.y - (100.0 + MIN_GROUND_CLEAR)).abs() < 1e-3,
            "pushed out of the mountain, not left inside: {}",
            t.translation.y
        );
    }

    /// No oracle = the historic flat floor, to the millimetre —
    /// the archipelago regression guard for the new branch.
    #[test]
    fn no_orbit_floor_no_change_when_absent() {
        let mut app = App::new();
        app.add_plugins((
            bevy::MinimalPlugins,
            bevy::input::InputPlugin,
            crate::input::ActionsPlugin,
            CameraRigPlugin,
        ));
        app.insert_resource(crate::settings::Settings::default());
        app.world_mut().spawn((
            Camera::default(),
            Projection::default(),
            CameraRig {
                position: Vec3::new(10.0, 10.0, 10.0),
                ..default()
            },
            Transform::from_xyz(10.0, 10.0, 10.0),
        ));
        app.update();
        let mut q = app.world_mut().query::<(&CameraRig, &Transform)>();
        let (_, t) = q.single(app.world()).unwrap();
        assert!(
            t.translation.y > 9.9 && t.translation.y < 10.1,
            "untouched: {}",
            t.translation.y
        );
    }

    /// RMB aim must grab the OS cursor in free-fly mode: without a grab
    /// the cursor hits the window edge, motion deltas stop, and aiming
    /// silently dies — the classic free-camera complaint.
    #[test]
    fn rmb_grabs_cursor_in_free_fly() {
        use bevy::window::{CursorGrabMode, CursorOptions, Window};
        let mut app = App::new();
        app.add_plugins((
            bevy::MinimalPlugins,
            bevy::input::InputPlugin,
            crate::input::ActionsPlugin,
            CameraRigPlugin,
        ));
        app.insert_resource(crate::settings::Settings::default());
        app.world_mut()
            .spawn((Window::default(), CursorOptions::default()));
        app.update();
        fn grab_of(a: &mut App) -> CursorGrabMode {
            let mut q = a.world_mut().query::<&CursorOptions>();
            q.single(a.world()).unwrap().grab_mode
        }
        assert!(matches!(grab_of(&mut app), CursorGrabMode::None));
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .press(MouseButton::Right);
        app.update();
        assert!(
            matches!(grab_of(&mut app), CursorGrabMode::Locked),
            "RMB in free-fly must lock the cursor (X11 downgrades to Confined at runtime)"
        );
        app.world_mut()
            .resource_mut::<ButtonInput<MouseButton>>()
            .release(MouseButton::Right);
        app.update();
        assert!(
            matches!(grab_of(&mut app), CursorGrabMode::None),
            "release must free the cursor"
        );
    }

    #[test]
    fn heading_snaps_track_quadrants() {
        let mut rig = CameraRig::new();
        let mut h = CameraHeading::default();
        rig.turn(1, &mut h);
        assert_eq!(h.0, 1);
        rig.turn(-1, &mut h);
        assert_eq!(h.0, 0);
        // Four lefts return home.
        for _ in 0..4 {
            rig.turn(-1, &mut h);
        }
        assert_eq!(h.0, 0);
    }

    #[test]
    fn pitch_clamps_and_snap_shifts_with_look() {
        let mut rig = CameraRig::new();
        for _ in 0..40 {
            rig.apply_look(0.0, 10.0);
        }
        assert!(rig.pitch <= PITCH_LIMIT + f32::EPSILON);

        let mut h = CameraHeading::default();
        rig.turn(1, &mut h); // snap 0 → 90 begins
        rig.apply_look(30.0, 0.0); // dragging during the snap keeps offset
        assert!(rig.snap_to > 90.0 - 1.0); // did not snap back to 90
    }

    #[test]
    fn forward_is_view_direction() {
        let mut rig = CameraRig::new();
        rig.yaw = 0.0;
        rig.pitch = 0.0;
        assert!(rig.forward().distance(Vec3::NEG_Z) < 1e-5);
        rig.yaw = 90.0;
        assert!(rig.forward().distance(Vec3::new(-1.0, 0.0, 0.0)) < 1e-5);
    }

    #[test]
    fn fov_clamps_to_wheel_range() {
        let mut rig = CameraRig::new();
        rig.set_fov(5.0);
        assert_eq!(rig.fov_target, FOV_MIN);
        rig.set_fov(500.0);
        assert_eq!(rig.fov_target, FOV_MAX);
    }

    /// A/D strafe must stay horizontal even while pitched hard: pre-fix
    /// the strafe vector used full rotation (pitch included) so a
    /// side-step turned into a climb.
    #[test]
    fn strafe_ignores_pitch() {
        let mut app = App::new();
        app.add_plugins((
            bevy::MinimalPlugins,
            bevy::input::InputPlugin,
            crate::input::ActionsPlugin,
            CameraRigPlugin,
        ));
        app.insert_resource(crate::settings::Settings::default());
        let mut rig = CameraRig::new();
        rig.position = Vec3::new(0.0, 50.0, 0.0);
        rig.pitch = 70.0; // looking almost straight up
        app.world_mut().spawn((
            Camera::default(),
            rig,
            Transform::from_xyz(0.0, 50.0, 0.0),
            Projection::Perspective(PerspectiveProjection::default()),
        ));
        app.update();
        app.world_mut()
            .resource_mut::<bevy::input::ButtonInput<bevy::input::keyboard::KeyCode>>()
            .press(bevy::input::keyboard::KeyCode::KeyD);
        // Headless frames are microseconds of wall time; run the fly
        // simulation at 5000× so a 30-frame hold moves a visible distance.
        app.world_mut()
            .resource_mut::<bevy::time::Time<bevy::time::Virtual>>()
            .set_relative_speed(5000.0);
        for _ in 0..30 {
            app.update();
        }
        let mut q = app.world_mut().query::<&Transform>();
        let t = q.single(app.world()).unwrap();
        // moved right (yaw 0 ⇒ +X) and NOT upward despite 70° pitch
        assert!(t.translation.x > 1.0, "strafe x {}", t.translation.x);
        assert!(
            (t.translation.y - 50.0).abs() < 0.01,
            "strafe y {}",
            t.translation.y
        );
    }
}
