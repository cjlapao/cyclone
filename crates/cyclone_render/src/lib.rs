//! cyclone_render — materials, terrain/sea/prop mesh builders, lighting
//! (spec 00 rendering defaults + spec 02 Part B).
//!
//! The only crate besides cyclone_engine allowed to touch render APIs.

pub mod continent_mesh;
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

/// Dev terrain-lab gate: `CYCLONE_CONTINENT=1` makes the *flight*
/// world the voxel-Earth — but only once the pilot is actually
/// flying. The menu backdrop must not pay the 8 s planet build:
/// cyclone_game flips this true on entering InGame.
#[derive(Resource)]
pub struct ContinentLive(pub bool);

/// Single-slot cache so Esc→Enter re-entry respawns meshes instantly
/// (assets survive the WorldRoot despawn; the 8 s build is once per
/// SEED, not once per visit).
#[derive(Resource)]
struct ContinentCache {
    seed: u64,
    sea: Handle<Mesh>,
    entries: Vec<(Vec3, Handle<Mesh>)>,
    extent_m: f32,
    mat: Handle<StandardMaterial>,
}

pub struct WorldBuilderPlugin;

impl Plugin for WorldBuilderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ContinentLive(false));
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

#[expect(
    clippy::too_many_arguments,
    reason = "the world-build system legitimately needs every asset store on hand"
)]
/// Watch `CurrentWorld` and (re)build the scene when it changes (spec 01:
/// reroll replaces the world; preview → InGame keeps it).
fn rebuild_world(
    world: Res<CurrentWorld>,
    cfg: Option<Res<WorldConfig>>,
    live: Option<Res<ContinentLive>>,
    cache: Option<ResMut<ContinentCache>>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    old: Query<Entity, With<WorldRoot>>,
) {
    let continent_env = std::env::var("CYCLONE_CONTINENT").is_ok();
    let live_on = live.as_deref().is_some_and(|c| c.0);
    // The fly-mode toggle can flip without a world change (Enter from a
    // preview of an already-built world): that flip is a trigger too.
    let live_flipped = continent_env && live_on && live.as_ref().is_some_and(|r| r.is_changed());
    if !world.is_changed() && !live_flipped {
        return;
    }
    // Continent flies InGame only: the menu backdrop stays archipelago
    // (no 8 s planet build before New Game is even pressed).
    let continent = continent_env && live_on;
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

    // Dev terrain-lab mode (round-155 pivot): CYCLONE_CONTINENT=1 swaps
    // the archipelago for the voxel-Earth heightfield. Same rig, same
    // materials, same rebuild path — a preview, not a fork.
    let cont_size: u32 = std::env::var("CYCLONE_CONTINENT_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4000)
        .clamp(64, 16_000); // voxels; generate_continent caps at 16384

    let mut deferred_cache: Option<ContinentCache> = None;
    commands.spawn((InWorld, WorldRoot)).with_children(|root| {
        spawn_lights(root);
        if continent {
            let extent_m = cont_size as f32 * 0.25;
            let fresh = cache
                .as_ref()
                .is_none_or(|k| k.seed != map.seed.0 || k.extent_m != extent_m);
            if fresh {
                let t0 = std::time::Instant::now();
                println!("continent mode: generating {}² voxel columns…", cont_size);
                let (c, chunks) = continent_mesh::generate_and_mesh(map.seed.0, cont_size);
                println!(
                    "continent ready: {} chunks, max {:.0} m, ocean {:.0}%, in {} ms",
                    chunks.len(),
                    c.metrics.max_height_m,
                    c.metrics.ocean_ratio * 100.0,
                    t0.elapsed().as_millis()
                );
                let mat = materials.add(StandardMaterial {
                    base_color: Color::WHITE,
                    perceptual_roughness: 0.92,
                    ..default()
                });
                let sea = {
                    let pad = 400.0f32;
                    let e = extent_m + pad;
                    let mut b = QuadBuilder::new();
                    b.quad(
                        [
                            Vec3::new(-pad, 0.0, -pad),
                            Vec3::new(-pad, 0.0, e),
                            Vec3::new(e, 0.0, e),
                            Vec3::new(e, 0.0, -pad),
                        ],
                        Vec3::Y,
                        srgb_to_vec4(1.0, 1.0, 1.0),
                    );
                    meshes.add(b.into_mesh())
                };
                let entries: Vec<(Vec3, Handle<Mesh>)> = chunks
                    .into_iter()
                    .map(|ch| (ch.anchor, meshes.add(ch.builder.into_mesh())))
                    .collect();
                root.spawn((
                    InWorld,
                    Mesh3d(sea.clone()),
                    MeshMaterial3d(sea_mat.clone()),
                ));
                for (a, h) in &entries {
                    root.spawn((
                        InWorld,
                        Mesh3d(h.clone()),
                        MeshMaterial3d(mat.clone()),
                        Transform::from_xyz(a.x, a.y, a.z),
                    ));
                }
                deferred_cache = Some(ContinentCache {
                    seed: map.seed.0,
                    sea,
                    entries,
                    extent_m,
                    mat,
                });
            } else {
                let k = cache.as_deref().unwrap(); // read-only respawn: deref, not move
                root.spawn((
                    InWorld,
                    Mesh3d(k.sea.clone()),
                    MeshMaterial3d(sea_mat.clone()),
                ));
                for (a, h) in &k.entries {
                    root.spawn((
                        InWorld,
                        Mesh3d(h.clone()),
                        MeshMaterial3d(k.mat.clone()),
                        Transform::from_xyz(a.x, a.y, a.z),
                    ));
                }
            }
        } else {
            terrain::spawn_terrain(root, &map, &mut meshes, &mut materials, lh);
            spawn_sea(root, &map, &mut meshes, &sea_mat);
            props::spawn_props(root, &map, &mut meshes, &mut materials, lh);
        }
    });
    if let Some(c) = deferred_cache {
        commands.insert_resource(c);
    }
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
    // Generous skirt: from any flight spawn or wall-bounce the horizon
    // must be sea, never the clear-colour void (intro-screenshot band).
    let pad = 160.0f32;
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
