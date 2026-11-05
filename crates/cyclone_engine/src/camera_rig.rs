//! Isometric orthographic camera rig (spec 00). Four headings with a 0.3 s
//! eased tween, three zoom levels, damped follow, world-bounds clamp.

use bevy::input::mouse::MouseWheel;
use bevy::prelude::*;

use crate::input::Action;
use crate::settings::Settings;
use crate::{Actions, CurrentWorld};

/// Camera heading index 0..=3 (spec 02 occlusion uses it: yaw 45+90·h).
#[derive(Resource, Default)]
pub struct CameraHeading(pub u8);

/// Vertical view size in world units for the three zoom levels.
pub const CAMERA_VIEWS: [f32; 3] = [56.0, 104.0, 176.0];
const PITCH: f32 = 35.264_f32.to_radians();
const CAM_DISTANCE: f32 = 260.0;
const TWEEN_SECONDS: f32 = 0.3;

#[derive(Component)]
pub struct CameraRig {
    pub yaw: f32,
    target_yaw: f32,
    tween: f32,
    from_yaw: f32,
    pub zoom: usize,
    pub view: f32,
}

impl CameraRig {
    pub fn new() -> Self {
        Self {
            yaw: 45.0,
            target_yaw: 45.0,
            tween: 1.0,
            from_yaw: 45.0,
            zoom: 1,
            view: CAMERA_VIEWS[1],
        }
    }

    /// Set the smoothed zoom level directly (preview fit-to-world).
    pub fn set_zoom(&mut self, zoom: usize) {
        self.zoom = zoom.min(CAMERA_VIEWS.len() - 1);
    }

    /// Turn to the next/previous of the four headings.
    pub fn turn(&mut self, dir: i8, heading: &mut CameraHeading) {
        heading.0 = (heading.0 + dir as u8).rem_euclid(4);
        self.from_yaw = self.yaw;
        self.target_yaw += dir as f32 * 90.0;
        self.tween = 0.0;
    }
}

impl Default for CameraRig {
    fn default() -> Self {
        Self::new()
    }
}

/// Optional explicit follow target (entity whose position the rig tracks).
#[derive(Component)]
pub struct RigFollow(pub Entity);

/// Fallback follow position when there is no follow entity (menu orbit…).
#[derive(Resource)]
pub struct RigTarget {
    pub position: Vec3,
    /// Slow auto-orbit (menu background). Degrees per second added to yaw.
    pub auto_orbit: f32,
}

impl Default for RigTarget {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            auto_orbit: 0.0,
        }
    }
}

/// World rectangle in tile units, refreshed when a world loads.
#[derive(Resource, Default)]
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

fn rig_input(
    mut rigs: Query<&mut CameraRig>,
    actions: Res<Actions>,
    mut heading: ResMut<CameraHeading>,
    settings: Res<Settings>,
    mut wheel: MessageReader<MouseWheel>,
) {
    let mut turn = 0i8;
    if actions.just_pressed(Action::RotateCamLeft) {
        turn -= 1;
    }
    if actions.just_pressed(Action::RotateCamRight) {
        turn += 1;
    }
    if turn != 0 {
        for mut rig in &mut rigs {
            rig.turn(turn, &mut heading);
        }
    }
    // Wheel zoom, clamped to the three levels.
    let mut wheel_y = 0.0f32;
    for event in wheel.read() {
        wheel_y += event.y;
    }
    if wheel_y != 0.0 {
        let step = if wheel_y > 0.0 { 1 } else { -1 };
        for mut rig in &mut rigs {
            let next = rig.zoom.saturating_add_signed(step);
            rig.set_zoom(next);
        }
    }
    let _ = settings.camera_rotation_speed;
}

/// Pan the fallback target with move actions, relative to the current
/// heading (spec 00: "Move … view-relative"). No-op while an explicit
/// follow entity exists — the helicopter controller owns movement then.
fn pan_target(
    time: Res<Time>,
    actions: Res<Actions>,
    rigs: Query<&CameraRig, With<Camera>>,
    mut fallback: ResMut<RigTarget>,
    follows: Query<&RigFollow>,
) {
    if !follows.is_empty() {
        return;
    }
    let Ok(rig) = rigs.single() else { return };
    let yaw = rig.yaw.to_radians();
    let fwd = Vec3::new(-yaw.sin(), 0.0, -yaw.cos());
    let right = Vec3::new(yaw.cos(), 0.0, -yaw.sin());
    let mut dir = Vec3::ZERO;
    if actions.pressed(Action::Up) {
        dir += fwd;
    }
    if actions.pressed(Action::Down) {
        dir -= fwd;
    }
    if actions.pressed(Action::Right) {
        dir += right;
    }
    if actions.pressed(Action::Left) {
        dir -= right;
    }
    if dir.length_squared() == 0.0 {
        return;
    }
    // Pan speed tracks zoom so screen-space speed feels constant.
    let speed = 34.0 * rig.view / CAMERA_VIEWS[1];
    fallback.position += dir.normalize_or_zero() * speed * time.delta_secs();
}

fn rig_update(
    time: Res<Time>,
    settings: Res<Settings>,
    mut rigs: Query<(&mut CameraRig, &mut Transform, &mut Projection), With<Camera>>,
    follows: Query<&RigFollow>,
    transforms: Query<&GlobalTransform>,
    fallback: Res<RigTarget>,
    bounds: Res<WorldBounds>,
) {
    let dt = time.delta_secs();
    let speed = settings.camera_rotation_speed.clamp(0.1, 4.0);
    for (mut rig, mut transform, mut proj) in &mut rigs {
        // Heading tween (ease in-out over 0.3 s, scaled by setting).
        if rig.tween < 1.0 {
            rig.tween = (rig.tween + dt / TWEEN_SECONDS * speed).min(1.0);
            let t = rig.tween;
            let ease = t * t * (3.0 - 2.0 * t);
            rig.yaw = rig.from_yaw + (rig.target_yaw - rig.from_yaw) * ease;
        }
        if fallback.auto_orbit != 0.0 {
            // Continuous menu/preview orbit: advance the target; the tween
            // (or a settled rig) follows, so Q/E turns still work on top.
            rig.target_yaw += fallback.auto_orbit * dt;
            if rig.tween >= 1.0 {
                rig.yaw = rig.target_yaw;
            }
        }
        // Zoom smoothing (frame-rate independent exponential).
        let target_view = CAMERA_VIEWS[rig.zoom];
        let k = 1.0 - (-10.0 * dt).exp();
        rig.view += (target_view - rig.view) * k;

        // Follow position.
        let mut pos = fallback.position;
        if let Some(f) = follows.iter().next()
            && let Ok(gt) = transforms.get(f.0)
        {
            pos = gt.translation();
        }
        // Keep the view over the world (spec: clamp bounds).
        let half = rig.view * 0.5;
        let (bmin, bmax) = (bounds.min, bounds.max);
        if bmax.x > bmin.x {
            pos.x = pos.x.clamp(bmin.x + half * 0.4, bmax.x - half * 0.4);
            pos.y = pos.y.clamp(bmin.y + half * 0.7, bmax.y - half * 0.7);
        }

        let rot = Quat::from_euler(EulerRot::YXZ, rig.yaw.to_radians(), -PITCH, 0.0);
        transform.rotation = rot;
        transform.translation = pos + rot * Vec3::new(0.0, 0.0, CAM_DISTANCE);

        if let Projection::Orthographic(ortho) = &mut *proj {
            // 0.20 computes aspect from the viewport; we own scale/near/far.
            ortho.scale = half;
            ortho.near = 1.0;
            ortho.far = CAM_DISTANCE * 3.0;
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
        app.add_systems(Update, (rig_input, pan_target, rig_update).chain());
    }
}
