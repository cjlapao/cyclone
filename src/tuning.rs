//! Every gameplay number lives here (spec: Tech stack → Config).

// ---------------------------------------------------------------- identity
/// Working title — configurable in this one constant (window title + binary name is `cyclone-rescue`).
/// World layout seed shown on Game Over (spec: acceptance).
pub const BASE_SEED: u64 = 1984;

pub const GAME_NAME: &str = "Cyclone Rescue";

// ---------------------------------------------------------------- display
pub const SCREEN_W: f32 = 320.0;
pub const SCREEN_H: f32 = 240.0;
/// HUD right panel width (inside SCREEN_W).
pub const PANEL_W: f32 = 72.0;
/// HUD bottom bar height (inside SCREEN_H).
pub const BAR_H: f32 = 28.0;
/// Fixed physics step, Hz.
pub const PHYSICS_HZ: f32 = 60.0;
pub const PHYSICS_DT: f32 = 1.0 / PHYSICS_HZ;

// ---------------------------------------------------------------- world
/// Tiles per world edge (square sea). ~12 screens wide with these sizes.
pub const WORLD_TILES: u32 = 96;
/// Edge length of a tile in world units.
pub const TILE: f32 = 16.0;
pub const WORLD_UNITS: f32 = WORLD_TILES as f32 * TILE;
/// Max tile height on land (spec: 1..=6; 0 = sea).
pub const MAX_LAND_HEIGHT: u8 = 6;
/// Pixels per altitude unit in the iso projection (the `k`).
pub const HEIGHT_PX: f32 = 4.0;
pub const NUM_ISLANDS: u32 = 14;
pub const NUM_CRATES: u32 = 5;
pub const MIN_HIDDEN_CRATES: u32 = 2;

// ---------------------------------------------------------------- heli
pub const HELI_MAX_SPEED: f32 = 3.0 * TILE; // 3 tiles/s
pub const HELI_ACCEL: f32 = 6.0 * TILE; // units/s^2
pub const HELI_DRAG: f32 = 2.2; // exponential-ish damping factor /s
pub const CLIMB_RATE: f32 = 6.0; // altitude units/s
pub const MAX_ALT: f32 = 8.0;
/// Ground speed above this (units/s) makes touchdown a crash.
pub const LAND_MAX_SPEED: f32 = 0.9 * TILE;
/// Winch radius in tiles and max altitude for winching.
pub const WINCH_RADIUS: f32 = 0.5 * TILE;
pub const WINCH_MAX_ALT: f32 = 2.0;
/// Seconds a crate/refugee must stay in winch range before it lifts off.
pub const WINCH_LIFT_TIME: f32 = 0.5;
pub const FUEL_SECONDS: f32 = 70.0; // full tank, hover consumption
pub const FUEL_SPEED_FACTOR: f32 = 0.9; // extra drain proportional to speed fraction
pub const REFUEL_SECONDS: f32 = 2.0;
pub const START_LIVES: u32 = 3;

// ---------------------------------------------------------------- cyclone
pub const CYCLONE_RADIUS: f32 = 6.0 * TILE; // outer force zone
pub const CYCLONE_INNER: f32 = 1.5 * TILE; // instant crash
pub const CYCLONE_BASE_SPEED: f32 = HELI_MAX_SPEED * 0.5;
/// Fraction of cyclone force felt while landed.
pub const CYCLONE_LAND_FACTOR: f32 = 0.5;
pub const CYCLONE_TRACK_SECS: f32 = 30.0;

// ---------------------------------------------------------------- planes
pub const PLANE_MIN_GAP: f32 = 15.0;
pub const PLANE_MAX_GAP: f32 = 30.0;
pub const PLANE_ALT_MIN: f32 = 3.0;
pub const PLANE_ALT_MAX: f32 = 5.0;
pub const PLANE_SPEED: f32 = 6.0 * TILE;
pub const PLANE_WARN_TIME: f32 = 2.0;
/// A plane hits you if you are within this |dz| of its altitude band.
pub const PLANE_ALT_TOL: f32 = 1.0;

// ---------------------------------------------------------------- rounds & score
pub const ROUND_TIME: f32 = 300.0; // 5 minutes
pub const ROUND_TIME_STEP: f32 = 20.0;
pub const MIN_ROUND_TIME: f32 = 180.0;
pub const CYCLONE_SPEEDUP: f32 = 1.10;
pub const SCORE_CRATE: u32 = 1000;
pub const SCORE_REFUGEE: u32 = 250;
pub const SCORE_PER_SECOND: u32 = 10;
pub const HIGH_SCORES: usize = 10;
/// View flip fade duration, seconds.
pub const FLIP_FADE: f32 = 0.25;
/// Explosion pause on the Crashed state, seconds.
pub const CRASH_PAUSE: f32 = 2.0;
