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

/// The terrain-lab mode as ONE read of the env dials: Some((size in
/// voxels, extent in metres)). rebuild, the bounds guard, and the game
/// all ask this — no scattered env reads that can drift apart.
pub fn continent_mode() -> Option<(u32, f32)> {
    // Value semantics (round 228): strictly '1', like island_mode and
    // the PREVIEW/SCALE gates. Mere presence used to mean ON, so the
    // friendly typo 'CYCLONE_CONTINENT=0' turned the world ON — the
    // one dial whose 'off' read as 'on'. Every doc, test (8/8), and
    // CI site already spells '=1', so the truth table only changes
    // for values nobody ever used. One reader made this a one-line
    // change: round 227's consolidation paid rent the next day.
    if !std::env::var("CYCLONE_CONTINENT").is_ok_and(|v| v == "1") {
        return None;
    }
    let size: u32 = std::env::var("CYCLONE_CONTINENT_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(4000)
        .clamp(64, 16_000); // voxels; generate_continent caps at 16384
    Some((size, size as f32 * 0.25))
}

/// Dev terrain-lab gate: `CYCLONE_CONTINENT=1` makes the *flight*
/// world the voxel-Earth — but only once the pilot is actually
/// flying. The menu backdrop must not pay the 8 s planet build:
/// cyclone_game flips this true on entering InGame.
#[derive(Resource)]
pub struct ContinentLive(pub bool);

/// Single-slot cache so Esc→Enter re-entry respawns meshes instantly
/// Ownership for the R8 shoreline texture: it belongs to no entity
/// (it is baked for a future WGSL material), so the rebuild sweep
/// could not find it by walking despawned children — it lives here
/// instead, and exactly one instance ever exists.
#[derive(Resource)]
struct ShoreOwn(bevy::asset::Handle<bevy::image::Image>);

/// Rebuilt-planets cache, keyed by SEED: terrain chunks + sea mesh
/// ride a reroll-free Esc→Enter→Esc→Enter cycle intact
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

/// A planet build in flight on the task pool: the 8 s
/// `generate_and_mesh` (round 201's documented freeze) runs behind
/// whatever is on screen — the archipelago preview, the Generating
/// screen, even the previous planet — and the world swap happens
/// whole when it lands. One per key (seed, size); stale keys get
/// replaced mid-flight (the wasted task costs a core, not a frame).
#[derive(Resource)]
struct ContinentPrewarm {
    key: (u64, u32),
    /// Slot so the poll can MOVE the finished task out of ResMut
    /// (you cannot move out of `Res`; `now_or_never` needs owned).
    task: Option<
        bevy::tasks::Task<(
            cyclone_worldgen::continent::Continent,
            Vec<continent_mesh::ContinentChunk>,
        )>,
    >,
}

/// The finished payload a waiting `rebuild_world` claims. `Option`
/// so the claim MOVES the data out of the resource (a 1 km planet
/// does not clone); a key mismatch marks it stale — never built.
#[derive(Resource)]
struct ContinentBuilt {
    key: (u64, u32),
    payload: Option<(
        cyclone_worldgen::continent::Continent,
        Vec<continent_mesh::ContinentChunk>,
    )>,
}

pub struct WorldBuilderPlugin;

impl Plugin for WorldBuilderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ContinentLive(false));
        // The guard writes bounds; init so the plugin stands alone
        // (the engine app already has it — Default is a no-op there).
        app.init_resource::<cyclone_engine::WorldBounds>();
        app.add_systems(Update, continent_bounds_guard);
        // Prewarm BEFORE rebuild (kick→poll→rebuild, same beat):
        // rebuild must see the task running the frame the world
        // lands, else it would sync-build and the freeze is back.
        app.add_systems(
            Update,
            (continent_prewarm_kick, continent_prewarm_poll).before(rebuild_world),
        );
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

/// The preview camera auto-frames from WorldBounds, and the engine's
/// refresh computes it from the archipelago's tile size (200) — while
/// a live planet spans 1000 m, the Esc-menu orbited a tiny square of a
/// world far bigger (pilot: "in the map same small square", round 176).
/// The guard overwrites the framing while the planet flies; it writes
/// only when the value is wrong, so island mode is untouched.
fn continent_bounds_guard(
    live: Option<Res<ContinentLive>>,
    mut bounds: ResMut<cyclone_engine::WorldBounds>,
) {
    let Some((_size, extent)) = continent_mode() else {
        return;
    };
    if !live.is_some_and(|c| c.0) || bounds.max.x == extent {
        return;
    }
    bounds.min = Vec2::ZERO;
    bounds.max = Vec2::new(extent, extent);
}

/// Start (or restart) the planet task whenever a world exists whose
/// key has neither a fresh cache nor a task nor a built payload —
/// REGARDLESS of `ContinentLive`: the preview shows the archipelago
/// while Enter is still coming, and that's exactly the time to
/// carve the planet nobody is looking at yet. Headless-safe pool
/// init mirrors screens.rs (tests carry no TaskPoolPlugin).
fn continent_prewarm_kick(
    world: Option<Res<CurrentWorld>>,
    cache: Option<Res<ContinentCache>>,
    prewarm: Option<Res<ContinentPrewarm>>,
    built: Option<Res<ContinentBuilt>>,
    mut commands: Commands,
) {
    let Some((size, _extent_m)) = continent_mode() else {
        return;
    };
    let Some(world) = world else { return };
    let key = (world.0.seed.0, size);
    let fresh = cache
        .as_ref()
        .is_some_and(|k| k.seed == world.0.seed.0 && k.extent_m == size as f32 * 0.25);
    let running = prewarm
        .as_ref()
        .is_some_and(|p| p.key == key && p.task.is_some());
    let waiting = built
        .as_ref()
        .is_some_and(|b| b.payload.is_some() && b.key == key);
    if fresh || running || waiting {
        return;
    }
    if prewarm.is_some() {
        // A stale-key task: drop the handle (the worker finishes
        // into nothing — a wasted core for seconds, never a frame).
        commands.remove_resource::<ContinentPrewarm>();
    }
    if built.is_some_and(|b| b.key != key) {
        // Symmetric shedding: a landed payload for a DEAD seed
        // pins its heightfield + chunk bytes with no consumer in
        // sight (Esc-to-menu after a pre-Enter landing, reroll
        // mid-landing). The next landing overwrites it anyway —
        // holding it that long is a leak with a polite name.
        commands.remove_resource::<ContinentBuilt>();
    }
    let seed = world.0.seed.0;
    let task = bevy::tasks::AsyncComputeTaskPool::get_or_init(bevy::tasks::TaskPool::default)
        .spawn(async move { continent_mesh::generate_and_mesh(seed, size) });
    commands.insert_resource(ContinentPrewarm {
        key,
        task: Some(task),
    });
}

/// Land the finished payload for rebuild to claim next beat. The
/// task is only polled when PROVEN finished, so `now_or_never`
/// resolves immediately — this system structurally cannot block
/// the main thread (the whole point of the exercise).
fn continent_prewarm_poll(prewarm: Option<ResMut<ContinentPrewarm>>, mut commands: Commands) {
    use std::task::{Context, Poll};
    let mut prewarm = match prewarm {
        Some(p) => p,
        None => return,
    };
    if prewarm.task.as_ref().is_none_or(|t| !t.is_finished()) {
        return;
    }
    // SAFETY of the move: task proven finished; taking the Option
    // out of the slot leaves a visibly-drained resource that
    // rebuild's claim (or next frame) removes.
    let Some(mut task) = prewarm.task.take() else {
        return;
    };
    // Std-only join: a proven-finished future resolves on the
    // no-op waker's single poll — no futures-lite dependency, no
    // blocking primitive. (Panicked worker => Ready never arrives;
    // the drain below already prevents a re-poll.)
    let waker = std::task::Waker::noop();
    let cx = &mut Context::from_waker(waker);
    let (c, chunks) = match std::future::Future::poll(std::pin::Pin::new(&mut task), cx) {
        Poll::Ready(v) => v,
        Poll::Pending => return,
    };
    let key = prewarm.key;
    commands.remove_resource::<ContinentPrewarm>();
    commands.insert_resource(ContinentBuilt {
        key,
        payload: Some((c, chunks)),
    });
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

    mut wb: ResMut<cyclone_engine::WorldBounds>,
    mut built: Option<ResMut<ContinentBuilt>>,
    old_assets: Query<(&Mesh3d, &MeshMaterial3d<StandardMaterial>), With<InWorld>>,
    shore_prev: Option<ResMut<ShoreOwn>>,
) {
    let continent_env = continent_mode().is_some();
    // Dev knob (NOT a shipped product dial): `CYCLONE_CONTINENT_
    // PREVIEW=1` (legacy alias: CYCLONE_CONTINENT_SCALE=1) skips the
    // menu gate — the planet goes live at boot instead of at Enter.
    // It does NOTHING else — round 183 proved the 0.2× "toy-scale
    // overview" ever existed only in comments (no mesh anchor,
    // transform, spawn height or WorldBounds ever multiplied by it);
    // the myth text died, the flag stays under honest names because
    // "boot straight into flying" is genuinely useful. A true
    // overview is a raised camera: Ctrl-boost to ~300 m and a whole
    // island fits the screen at TRUE scale.
    let skip_gate = std::env::var("CYCLONE_CONTINENT_PREVIEW").is_ok_and(|v| v == "1")
        || std::env::var("CYCLONE_CONTINENT_SCALE").is_ok_and(|v| v == "1");
    let live_on = live.as_deref().is_some_and(|c| c.0) || (continent_env && skip_gate);
    // The fly-mode toggle can flip without a world change (Enter from a
    // preview of an already-built world): that flip is a trigger too.
    let live_flipped = continent_env && live_on && live.as_ref().is_some_and(|r| r.is_changed());
    // A landed planet payload re-triggers the rebuild that waited.
    let payload_landed = built
        .as_ref()
        .is_some_and(|b| b.payload.is_some() && b.key.0 == world.0.seed.0);
    if !world.is_changed() && !live_flipped && !payload_landed {
        return;
    }
    // The flip is made REAL here (round 184): a local `live_on` never
    // reached WorldBounds or the game's own env readers — PREVIEW
    // half-skipped the gate (planet built) while the camera framed a
    // default-bounds void, caught the day the flag got its first
    // test. One insert, every reader agrees — no second source of
    // "is the planet flying".
    if continent_env && skip_gate && !live.as_deref().is_some_and(|c| c.0) {
        commands.insert_resource(ContinentLive(true));
    }
    // Continent flies InGame only: the menu backdrop stays archipelago
    // (no 8 s planet build before New Game is even pressed).
    let continent = continent_env && live_on;
    let map = world.0.clone();
    let cont_size = if continent {
        continent_mode().map(|(s, _)| s).unwrap_or(4000)
    } else {
        0
    };

    // The round-201 freeze, deleted: a continent with neither a
    // fresh cache nor a landed payload WAITS — no despawn, no
    // build, no main-thread work. What the player sees while the
    // worker carves (archipelago preview / Generating / the
    // previous planet) simply keeps repainting; the swap happens
    // whole the frame the payload lands. The kick system (ordered
    // before this one) owns task existence.
    let extent_now = cont_size as f32 * 0.25;
    let cache_fresh_now = continent
        && cache
            .as_ref()
            .is_some_and(|k| k.seed == map.seed.0 && k.extent_m == extent_now);
    let payload_here = continent
        && built
            .as_ref()
            .is_some_and(|b| b.payload.is_some() && b.key == (map.seed.0, cont_size));
    if continent && !cache_fresh_now && !payload_here {
        return;
    }
    // Past the wait: the old root goes; the new world (claimed
    // payload or cache respawn or archipelago) takes its place.

    for e in &old {
        commands.entity(e).despawn();
    }
    // THE ASSET SWEEP (round 216: measured +236 meshes over five
    // rerolls, ~46/world, zero remove-sites in the repo — Bevy
    // never GCs the asset server, and a pilot hour of Esc/R is a
    // hundred rebuilds). Every mesh+material on the despawning
    // world's children is superseded the instant the new build is
    // chosen — EXCEPT what a FRESH cache will respawn (the Esc→
    // Enter case: same handles must survive the menu beat). When
    // the cache is stale, its own handles join the doomed set:
    // the claim replaces them this very run, so keeping them was
    // the leak hiding in 'we reuse via cache'.
    {
        use std::collections::HashSet; // AssetId<Mesh> paths below via prelude
        let keep: HashSet<(AssetId<Mesh>, bevy::asset::AssetId<StandardMaterial>)> =
            if cache_fresh_now {
                if let Some(k) = cache.as_ref() {
                    std::iter::once((k.sea.id(), k.mat.id()))
                        .chain(k.entries.iter().map(|(_, h)| (h.id(), k.mat.id())))
                        .collect()
                } else {
                    HashSet::new()
                }
            } else {
                HashSet::new()
            };
        let mut doom_m: Vec<AssetId<Mesh>> = Vec::new();
        let mut doom_t: Vec<bevy::asset::AssetId<StandardMaterial>> = Vec::new();
        for (m, t) in &old_assets {
            let pair = (m.id(), t.id());
            if !keep.contains(&pair) {
                doom_m.push(m.id());
                doom_t.push(t.id());
            }
        }
        // Stale cache's planet: unreferenced the moment the claim
        // lands (this run), so it is doomed NOW.
        if !cache_fresh_now && let Some(k) = cache.as_ref() {
            doom_m.push(k.sea.id());
            for (_, h) in &k.entries {
                doom_m.push(h.id());
            }
            doom_t.push(k.mat.id());
        }
        let uniq_m: std::collections::HashSet<_> = doom_m.into_iter().collect();
        let uniq_t: std::collections::HashSet<_> = doom_t.into_iter().collect();
        for id in uniq_m {
            if !keep.iter().any(|(m, _)| *m == id) {
                meshes.remove(id);
            }
        }
        for id in uniq_t {
            if !keep.iter().any(|(_, t)| *t == id) {
                materials.remove(id);
            }
        }
    }

    // Claim OUT HERE — inside the spawn closure `commands` is
    // already borrowed (round 195's lesson, applied preemptively).
    let mut claimed: Option<(
        cyclone_worldgen::continent::Continent,
        Vec<continent_mesh::ContinentChunk>,
    )> = None;
    if payload_here {
        if let Some(b) = built.as_mut() {
            claimed = b.payload.take();
        }
        commands.remove_resource::<ContinentBuilt>();
    }

    let lh = cfg.map_or(LEVEL_HEIGHT, |c| c.0.level_height);
    // White base: the ocean's colour lives in VERTEX COLOURS — the
    // continent ramp (build_sea) and the classic foam/shallows
    // (build_sea_classic) paint it. A navy base used to live here
    // and MULTIPLIED the careful turquoise into near-black: a
    // design palette the pilot never saw because the material ate
    // it (found round 212, same audit that found the missing foam).
    let sea_mat = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.15,
        metallic: 0.1,
        ..default()
    });

    // Shoreline R8 texture for the future sea WGSL material (spec 02
    // B2). Owned now (ShoreOwn): referenced by no entity, an
    // un-owned one leaks one texture per rebuild forever (the
    // round-216 sweep). Exactly one exists; the last frame's is
    // freed the frame the next is born.
    if let Some(prev) = shore_prev {
        images.remove(&prev.0);
    }
    let shore = shore_texture(&map, &mut images);
    commands.insert_resource(ShoreOwn(shore));
    // Dev terrain-lab mode (round-155 pivot): CYCLONE_CONTINENT=1 swaps
    // the archipelago for the voxel-Earth heightfield. Same rig, same
    // materials, same rebuild path — a preview, not a fork.
    let mut deferred_cache: Option<ContinentCache> = None;
    // WorldRoot carries Visibility AND Transform. Bevy validates two
    // inherited components per parent-child pair (B0004): children get
    // Visibility from the render plugin and Transform from the mesh
    // bundles — if the PARENT lacks either, every child insertion
    // warns ("This will cause inconsistent behaviors!"). The
    // Visibility half drowned the terminal once (pilot, round 175);
    // the Transform half has been quieter but is the SAME bug —
    // witnessed headless this round: exactly one Transform-less
    // parent, this root, one line per mesh chunk per rebuild.
    // Terrain floor for the rig: the camera cannot sink under this
    // world's own surface (myth #9's fix; the engine clamp + unit
    // test own the promise, this is the wiring). Per BUILD, so a
    // reroll re-arms the oracle with its own seed; the archipelago
    // keeps its historic flat y ≥ 2.5 floor untouched (no oracle,
    // no regression — deliberate scope).
    if continent {
        // Bounds BEFORE any rig can see them: the Update-schedule
        // guard below stays as a backstop, but a stale archipelago
        // box (160 m) clamping this planet's flagship spawn (860 m
        // out) would PERSIST — rig position is stateful, so the
        // 700 m teleport would stick. Set here, synchronously,
        // three statements before the root (the rig's future
        // home) exists as an entity.
        let extent_m = cont_size as f32 * 0.25;
        wb.min = Vec2::ZERO;
        wb.max = Vec2::new(extent_m, extent_m);
        let seed = map.seed.0;
        let size = cont_size;
        commands.insert_resource(cyclone_engine::TerrainFloor(std::sync::Arc::new(
            move |x: f32, z: f32| {
                continent_mesh::surface_height_m(x as f64, z as f64, seed, size) as f32
            },
        )));
    } else {
        commands.remove_resource::<cyclone_engine::TerrainFloor>();
    }

    commands
        .spawn((
            InWorld,
            WorldRoot,
            Visibility::Visible,
            Transform::default(),
        ))
        .with_children(|root| {
            spawn_lights(root);
            if continent {
                let extent_m = cont_size as f32 * 0.25;
                let fresh = cache
                    .as_ref()
                    .is_none_or(|k| k.seed != map.seed.0 || k.extent_m != extent_m);
                if fresh {
                    let t0 = std::time::Instant::now();
                    println!("continent mode: planet carved on a worker…");
                    let Some((c, chunks)) = claimed.take() else {
                        return; // payload vanished (stale-removed): next trigger rebuilds
                    };
                    let nchunks = chunks.len(); // chunks is moved by the builder below
                    // Round 271: the 'ready' line used to print HERE, before any
                    // meshing — 'in 0 ms' it said, truthfully timing only the
                    // payload take, misleadingly for a pilot who just watched a
                    // seven-second spinner. It now prints after the last spawn,
                    // naming its real unit (meshed). Console text only; no
                    // geometry or logic moves.
                    let mat = materials.add(StandardMaterial {
                        base_color: Color::WHITE,
                        perceptual_roughness: 0.92,
                        ..default()
                    });
                    // The sea is terrain too: bathymetry quads (shore
                    // cyan → basin navy), then one safety plane under
                    // it for the void beyond a small map's buckets.
                    let sea = {
                        let e = extent_m;
                        let mut b = continent_mesh::build_sea(&c).unwrap_or_else(|| {
                            let mut b = QuadBuilder::new();
                            b.quad(
                                [
                                    Vec3::new(0.0, -0.02, 0.0),
                                    Vec3::new(0.0, -0.02, e),
                                    Vec3::new(e, -0.02, e),
                                    Vec3::new(e, -0.02, 0.0),
                                ],
                                Vec3::Y,
                                srgb_to_vec4(0.02, 0.10, 0.24),
                            );
                            b
                        });
                        // Safety plane 2 cm below: any bucket grid
                        // rounding leaves no blue-black gap at the rim.
                        b.quad(
                            [
                                Vec3::new(0.0, -0.05, 0.0),
                                Vec3::new(0.0, -0.05, e),
                                Vec3::new(e, -0.05, e),
                                Vec3::new(e, -0.05, 0.0),
                            ],
                            Vec3::Y,
                            srgb_to_vec4(0.02, 0.10, 0.24),
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
                    println!(
                        "continent ready: {nchunks} chunks, max {:.0} m, ocean {:.0}%, meshed in {} ms",
                        c.metrics.max_height_m,
                        c.metrics.ocean_ratio * 100.0,
                        t0.elapsed().as_millis()
                    );
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
    let (ex, ez) = (w as f32, h as f32);
    // Generous skirt DEEPER than the tiles: from any flight spawn or
    // wall-bounce the horizon must be sea, never the clear-colour
    // void (intro-screenshot band). Its colour is a vertex colour
    // now — the material is white and stays out of the business.
    let pad = 160.0f32;
    let mut skirt = QuadBuilder::new();
    skirt.quad(
        [
            Vec3::new(-pad, -0.05, -pad),
            Vec3::new(-pad, -0.05, ez + pad),
            Vec3::new(ex + pad, -0.05, ez + pad),
            Vec3::new(ex + pad, -0.05, -pad),
        ],
        Vec3::Y,
        srgb_to_vec4(0.05, 0.28, 0.47),
    );
    root.spawn((
        InWorld,
        Mesh3d(meshes.add(skirt.into_mesh())),
        MeshMaterial3d(sea_mat.clone()),
    ));
    // The map itself gets a real shaded sea: foam where shore_dist
    // ≤ 1, turquoise shallows to navy by 15 tiles (spec 02 B2's
    // promise, finally consumed by the surface it names). The data
    // grid has been baked by worldgen since the pipeline's first
    // day — this is the consumer it never had.
    if let Some(b) = continent_mesh::build_sea_classic(map) {
        root.spawn((
            InWorld,
            Mesh3d(meshes.add(b.into_mesh())),
            MeshMaterial3d(sea_mat.clone()),
        ));
    }
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
