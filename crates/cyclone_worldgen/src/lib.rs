//! cyclone_worldgen — PURE Rust archipelago generator (spec 02 Part A).
//!
//! Determinism contract (A2): every random draw comes from `ChaCha8Rng`
//! streams derived from the seed; all transcendentals go through `libm`;
//! no hash-map iteration; no `thread_rng`. Same seed ⇒ byte-identical
//! `WorldMap` on every platform.

pub mod config;
pub mod continent;
pub mod error;
pub mod pipeline;
pub use pipeline::tile_tint;
pub mod grid;
pub mod map;
pub mod names;
pub mod noise;
pub mod rng;
pub mod seed;

pub use config::WorldGenConfig;
pub use continent::{
    Continent, ContinentConfig, ContinentMetrics, elevation_m, generate_continent, island_mode,
};
pub use error::GenError;
pub use grid::Grid;
pub use map::{
    Archetype, CrateSpawn, Difficulty, GenResult, Helipad, Island, Prop, PropKind, Rect,
    RefugeeSpawn, Surface, WorldMap, WorldMetrics,
};
pub use seed::Seed;

pub use pipeline::{
    Progress, ProgressNoop, generate, longest_mst_hop, nn_route_len, regenerate_round, validate,
};
