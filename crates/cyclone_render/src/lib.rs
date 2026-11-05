//! cyclone_render — materials, terrain/sea/prop mesh builders, lighting
//! (spec 00 rendering defaults + spec 02 Part B).
//!
//! The only crate besides cyclone_engine allowed to touch render APIs.

mod props;
mod terrain;

pub use props::{Crate, crate_occlusion, gen_heading_of};
pub use terrain::{LEVEL_HEIGHT, QuadBuilder};

use bevy::light::DirectionalLight;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use cyclone_engine::{CurrentWorld, InWorld, WorldConfig};
use cyclone_worldgen::WorldMap;

/// Root entity of the built world; children ride along on despawn.
#[derive(Component)]
pub struct WorldRoot;

pub struct WorldBuilderPlugin;

impl Plugin for WorldBuilderPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            rebuild_world
                .run_if(bevy::ecs::schedule::common_conditions::resource_exists::<CurrentWorld>),
        );
        // Spec 02 B4: crates hidden from the camera's heading blink out
        // as the rig turns (free yaw keeps CameraHeading in sync).
        app.add_systems(
            Update,
            props::crate_occlusion.run_if(
                bevy::ecs::schedule::common_conditions::resource_exists::<
                    cyclone_engine::CameraHeading,
                >,
            ),
        );
    }
}

/// Watch `CurrentWorld` and (re)build the scene when it changes (spec 01:
/// reroll replaces the world; preview → InGame keeps it).
fn rebuild_world(
    world: Res<CurrentWorld>,
    cfg: Option<Res<WorldConfig>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    old: Query<Entity, With<WorldRoot>>,
) {
    if !world.is_changed() {
        return;
    }
    for e in &old {
        commands.entity(e).despawn();
    }
    let map = world.0.clone();

    let lh = cfg.map_or(LEVEL_HEIGHT, |c| c.0.level_height);
    let sea_mat = materials.add(StandardMaterial {
        base_color: Color::srgb(0.05, 0.28, 0.47),
        perceptual_roughness: 0.15,
        metallic: 0.1,
        ..default()
    });

    // Shoreline R8 texture for the future sea WGSL material (spec 02 B2).
    let _shore = shore_texture(&map, &mut images);

    commands.spawn((InWorld, WorldRoot)).with_children(|root| {
        spawn_lights(root);
        terrain::spawn_terrain(root, &map, &mut meshes, &mut materials, lh);
        spawn_sea(root, &map, &mut meshes, &sea_mat);
        props::spawn_props(root, &map, &mut meshes, &mut materials, lh);
    });
}

/// The shoreline-distance texture the sea shader samples (spec 02 B2):
/// one R8 texel per tile, scaled to full range at the 15-tile cap.
fn shore_texture(map: &WorldMap, images: &mut ResMut<Assets<Image>>) -> Handle<Image> {
    let (w, h) = map.size;
    let mut data = vec![0u8; (w as usize) * (h as usize)];
    for y in 0..h {
        for x in 0..w {
            data[y as usize * w as usize + x as usize] = map.shore_dist.raw(x, y).min(15) * 17;
        }
    }
    images.add(Image::new(
        Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        data,
        TextureFormat::R8Unorm,
        Default::default(),
    ))
}

fn spawn_lights(root: &mut ChildSpawnerCommands) {
    // Soft sun + mild ambient (spec 00 look). Shadows land with the look
    // polish pass; the 0.20 shadow pipeline API churned (DECISIONS.md).
    root.spawn((
        InWorld,
        DirectionalLight {
            illuminance: 22_000.0,
            color: Color::srgb(1.0, 0.96, 0.88),
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.9, -0.85, 0.0)),
    ));
}

fn spawn_sea(
    root: &mut ChildSpawnerCommands,
    map: &WorldMap,
    meshes: &mut ResMut<Assets<Mesh>>,
    sea_mat: &Handle<StandardMaterial>,
) {
    let (w, h) = map.size;
    let pad = 40.0f32;
    let (ex, ez) = (w as f32, h as f32);
    let mut b = QuadBuilder::new();
    let col = srgb_to_vec4(1.0, 1.0, 1.0);
    b.quad(
        [
            Vec3::new(-pad, 0.0, -pad),
            Vec3::new(-pad, 0.0, ez + pad),
            Vec3::new(ex + pad, 0.0, ez + pad),
            Vec3::new(ex + pad, 0.0, -pad),
        ],
        Vec3::Y,
        col,
    );
    root.spawn((
        InWorld,
        Mesh3d(meshes.add(b.into_mesh())),
        MeshMaterial3d(sea_mat.clone()),
    ));
}

/// sRGB → linear helper for vertex colours.
pub(crate) fn srgb_to_vec4(r: f32, g: f32, b: f32) -> Vec4 {
    let f = |v: f32| {
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    Vec4::new(f(r), f(g), f(b), 1.0)
}
