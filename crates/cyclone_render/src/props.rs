//! Placeholder props (spec 02 B3): shared unit-box mesh + transforms.
//! Real low-poly models arrive with spec 06; only the geometry source
//! changes, the spawn mapping below stays.

use bevy::prelude::*;
use cyclone_engine::InWorld;
use cyclone_worldgen::{PropKind, WorldMap};

use crate::srgb_to_vec4;
use crate::terrain::QuadBuilder;

/// Placeholder prop height per level (world units) — until spec 06 models.
const PROP_UNIT: f32 = 1.15;

fn kind_color(kind: PropKind) -> (f32, f32, f32) {
    match kind {
        PropKind::House => (0.82, 0.74, 0.62),
        PropKind::Hut => (0.66, 0.52, 0.38),
        PropKind::PalmTree => (0.55, 0.35, 0.20),
        PropKind::PineTree => (0.18, 0.38, 0.22),
        PropKind::Rock => (0.50, 0.50, 0.53),
        PropKind::Lighthouse => (0.92, 0.92, 0.90),
        PropKind::RadioTower => (0.65, 0.67, 0.70),
        PropKind::Hangar => (0.45, 0.50, 0.42),
        PropKind::Jetty => (0.48, 0.36, 0.24),
    }
}

/// Unit box anchored at y 0..1, x/z −0.5..0.5.
fn unit_box() -> Mesh {
    let mut b = QuadBuilder::new();
    let c = srgb_to_vec4(1.0, 1.0, 1.0);
    let (x0, x1, z0, z1, t) = (-0.5f32, 0.5f32, -0.5f32, 0.5f32, 1.0f32);
    b.quad(
        [
            Vec3::new(x0, t, z0),
            Vec3::new(x0, t, z1),
            Vec3::new(x1, t, z1),
            Vec3::new(x1, t, z0),
        ],
        Vec3::Y,
        c,
    );
    for (ring, n) in [
        (
            [
                Vec3::new(x0, 0.0, z0),
                Vec3::new(x0, t, z0),
                Vec3::new(x1, t, z0),
                Vec3::new(x1, 0.0, z0),
            ],
            Vec3::new(0.0, 0.0, -1.0),
        ),
        (
            [
                Vec3::new(x1, 0.0, z0),
                Vec3::new(x1, t, z0),
                Vec3::new(x1, t, z1),
                Vec3::new(x1, 0.0, z1),
            ],
            Vec3::X,
        ),
        (
            [
                Vec3::new(x1, 0.0, z1),
                Vec3::new(x1, t, z1),
                Vec3::new(x0, t, z1),
                Vec3::new(x0, 0.0, z1),
            ],
            Vec3::new(0.0, 0.0, 1.0),
        ),
        (
            [
                Vec3::new(x0, 0.0, z1),
                Vec3::new(x0, t, z1),
                Vec3::new(x0, t, z0),
                Vec3::new(x0, 0.0, z0),
            ],
            Vec3::NEG_X,
        ),
    ] {
        b.quad(ring, n, c);
    }
    b.into_mesh()
}

/// Flat 1×1 quad on XZ.
fn unit_quad() -> Mesh {
    let mut b = QuadBuilder::new();
    let c = srgb_to_vec4(1.0, 1.0, 1.0);
    b.quad(
        [
            Vec3::new(-0.5, 0.0, -0.5),
            Vec3::new(-0.5, 0.0, 0.5),
            Vec3::new(0.5, 0.0, 0.5),
            Vec3::new(0.5, 0.0, -0.5),
        ],
        Vec3::Y,
        c,
    );
    b.into_mesh()
}

pub fn spawn_props(
    root: &mut ChildSpawnerCommands,
    map: &WorldMap,
    meshes: &mut ResMut<Assets<Mesh>>,
    materials: &mut ResMut<Assets<StandardMaterial>>,
    lh: f32,
) {
    let box_h = meshes.add(unit_box());
    let quad_h = meshes.add(unit_quad());

    // One coloured material per prop kind + special marker colour class.
    let mat_of = |mats: &mut ResMut<Assets<StandardMaterial>>, rgb: (f32, f32, f32), rough| {
        mats.add(StandardMaterial {
            base_color: Color::srgb(rgb.0, rgb.1, rgb.2),
            perceptual_roughness: rough,
            ..default()
        })
    };

    // Props.
    for p in &map.props {
        let (fw, fd, levels) = p.kind.dims();
        let h = if p.kind == PropKind::Jetty {
            0.12
        } else {
            levels.max(1) as f32 * PROP_UNIT
        };
        let ground = match p.kind {
            PropKind::Jetty => 0.05,
            _ => map.height.raw(p.tile.0, p.tile.1) as f32 * lh,
        };
        let (cx, cz) = (
            p.tile.0 as f32 + fw as f32 * 0.5,
            p.tile.1 as f32 + fd as f32 * 0.5,
        );
        root.spawn((
            InWorld,
            Mesh3d(box_h.clone()),
            MeshMaterial3d(mat_of(materials, kind_color(p.kind), 0.85)),
            Transform::from_xyz(cx, ground, cz)
                .with_scale(Vec3::new(fw as f32 * 0.8, h, fd as f32 * 0.8))
                .with_rotation(Quat::from_rotation_y(
                    p.facing as f32 * std::f32::consts::FRAC_PI_2,
                )),
        ));
    }

    // Helipads: flat quads, base pad yellow, others pale grey.
    for (i, pad) in map.helipads.iter().enumerate() {
        let ground = map.height.raw(pad.tile.0, pad.tile.1) as f32 * lh;
        let rgb = if i == 0 {
            (0.87, 0.72, 0.20)
        } else {
            (0.72, 0.72, 0.75)
        };
        root.spawn((
            InWorld,
            Mesh3d(quad_h.clone()),
            MeshMaterial3d(mat_of(materials, rgb, 0.7)),
            Transform::from_xyz(
                pad.tile.0 as f32 + 0.5,
                ground + 0.04,
                pad.tile.1 as f32 + 0.5,
            )
            .with_scale(Vec3::new(2.9, 1.0, 2.9)),
        ));
    }

    // Crates (spec 02 B4): carry `hidden_from` as a component; a system
    // hides them against the camera's current heading (see below).
    for c in &map.crates {
        let ground = map.height.raw(c.tile.0, c.tile.1) as f32 * lh;
        root.spawn((
            InWorld,
            Crate {
                hidden_from: c.hidden_from,
            },
            Visibility::default(),
            Mesh3d(box_h.clone()),
            MeshMaterial3d(mat_of(materials, (0.83, 0.44, 0.14), 0.8)),
            Transform::from_xyz(c.tile.0 as f32 + 0.5, ground, c.tile.1 as f32 + 0.5)
                .with_scale(Vec3::splat(0.7)),
        ));
    }

    // Refugees: small blue figures (placeholder until spec 06).
    for r in &map.refugees {
        let ground = map.height.raw(r.tile.0, r.tile.1) as f32 * lh;
        root.spawn((
            InWorld,
            Mesh3d(box_h.clone()),
            MeshMaterial3d(mat_of(materials, (0.20, 0.35, 0.72), 0.9)),
            Transform::from_xyz(r.tile.0 as f32 + 0.5, ground, r.tile.1 as f32 + 0.5)
                .with_scale(Vec3::new(0.5, 0.9, 0.5)),
        ));
    }
}

/// Crate marker + its per-heading invisibility (spec 02 B4). The camera
/// rig keeps `CameraHeading` synced to the free yaw; when it changes,
/// crates hidden for that heading flip to `Visibility::Hidden`, so a
/// crate behind a cliff visibly disappears as the player turns (Q/E).
#[derive(Component)]
pub struct Crate {
    pub hidden_from: [bool; 4],
}

/// Map a camera heading (rig convention: canonical yaw = 45° + 90°·h,
/// forward = rot(yaw)·-Z, so the viewpoint sits at −forward from the
/// crate) onto worldgen's `hidden_from` index (gen rays run from the
/// crate toward the viewpoint, CAM_DIRS[0] = (+x,+y)).
/// Derivation: cam 0 ⇒ viewpoint offset (−S,+S) = CAM_DIRS[3]; each
/// further cam step walks the offsets backwards ⇒ h_gen = (3 − cam) % 4.
pub fn gen_heading_of(cam: u8) -> usize {
    (3 - cam % 4) as usize % 4
}

/// Spec 02 B4 demo: hide crates whose sight line is blocked from the
/// camera's current heading (acceptance C6: Q/E turn, watch them blink).
pub fn crate_occlusion(
    mut crates: Query<(&Crate, &mut Visibility)>,
    heading: Res<cyclone_engine::CameraHeading>,
) {
    if !heading.is_changed() {
        return;
    }
    let h = gen_heading_of(heading.0);
    for (c, mut v) in &mut crates {
        *v = if c.hidden_from[h] {
            Visibility::Hidden
        } else {
            Visibility::Visible
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_space_maps_rig_to_gen() {
        // Derivation check: cam 0 (yaw 45°, viewpoint (+z,−x)) is gen idx 3.
        assert_eq!(gen_heading_of(0), 3);
        assert_eq!(gen_heading_of(1), 2);
        assert_eq!(gen_heading_of(2), 1);
        assert_eq!(gen_heading_of(3), 0);
        // Wraps like the rig does.
        assert_eq!(gen_heading_of(4), 3);
        let all: Vec<_> = (0..4).map(gen_heading_of).collect();
        assert_eq!(
            all.len(),
            all.iter().collect::<std::collections::HashSet<_>>().len()
        );
    }
}
