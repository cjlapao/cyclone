//! The generation driver + pipeline stages (spec 02 A5).
//!
//! Each stage is `fn stage(ctx) -> Result<(), GenError>`. A stage may
//! return `GenError::Retry(reason)`; the driver then restarts the whole
//! world with attempt n+1 (fresh streams per A2.2). After
//! `max_world_attempts`, `GenError::Exhausted` with all reasons.

mod archetypes;
mod cleanup;
mod common;
pub use common::tile_tint;
pub use metrics::{longest_mst_hop, nn_route_len, validate};
mod crates;
mod helipads;
mod metrics;
mod props;
mod refugees;
mod sites;
mod surface;
mod terrain;

use std::time::Instant;

use rand_chacha::ChaCha8Rng;

use crate::config::WorldGenConfig;
use crate::error::GenError;
use crate::map::{Surface, WorldMap, WorldMetrics};
use crate::rng::{self, stage_stream_id};
use crate::seed::Seed;

/// Progress callback (spec 02 A6).
pub trait Progress: Send + Sync {
    fn report(&self, stage: &'static str, fraction: f32);
}

pub struct ProgressNoop;
impl Progress for ProgressNoop {
    fn report(&self, _stage: &'static str, _fraction: f32) {}
}

/// One island site from stage 1.
#[derive(Clone, Copy, Debug)]
pub struct Site {
    pub centre: (u32, u32),
    /// Base radius in tiles.
    pub radius: f64,
    pub archetype: crate::map::Archetype,
}

/// The partially built world plus everything the stages need.
pub struct GenCtx<'a> {
    pub seed: Seed,
    pub round: u32,
    pub attempt: u32,
    pub cfg: WorldGenConfig,
    pub map: WorldMap,
    pub sites: Vec<Site>,
    /// Occupancy mask: props, helipads(+ring), crates, refugees may not overlap.
    pub occupied: Vec<bool>,
    pub progress: &'a dyn Progress,
}

impl GenCtx<'_> {
    pub fn w(&self) -> u32 {
        self.map.size.0
    }
    pub fn h(&self) -> u32 {
        self.map.size.1
    }

    /// Fresh RNG for a stage under this attempt's stream (deterministic).
    pub fn stage_rng(&self, stage: u32) -> ChaCha8Rng {
        rng::stream(self.seed.0, stage_stream_id(self.attempt, stage))
    }

    /// A sub-stream for one island of a stage (spec A7 "using the island's
    /// own stream").
    pub fn island_rng(&self, stage: u32, island: u8) -> ChaCha8Rng {
        let base = stage_stream_id(self.attempt, stage) * 1000 + u64::from(island) + 1;
        rng::stream(self.seed.0, base)
    }

    /// Round-salted stream for stages that re-roll per round (8–10).
    pub fn round_rng(&self, stage: u32, sub: u64) -> ChaCha8Rng {
        let base = stage_stream_id(self.attempt, stage) * 1000 + sub;
        rng::stream(self.seed.0, rng::salt_for_round(base, self.round))
    }

    pub fn report(&self, stage: &'static str, fraction: f32) {
        self.progress.report(stage, fraction);
    }

    pub fn in_bounds(&self, x: u32, y: u32) -> bool {
        x < self.w() && y < self.h()
    }

    pub fn is_occupied(&self, x: u32, y: u32) -> bool {
        self.occupied[(y * self.w() + x) as usize]
    }

    pub fn occupy(&mut self, x: u32, y: u32) {
        let i = (y * self.map.size.0 + x) as usize;
        self.occupied[i] = true;
    }

    /// Mark the `footprint` rect anchored at `tile` (min corner) occupied.
    pub fn occupy_footprint(&mut self, tile: (u32, u32), fp: (u8, u8)) {
        for dy in 0..fp.1 as u32 {
            for dx in 0..fp.0 as u32 {
                let (x, y) = (tile.0 + dx, tile.1 + dy);
                if self.in_bounds(x, y) {
                    self.occupy(x, y);
                }
            }
        }
    }

    /// Standing ground used by several stages: land, no cliff, free.
    pub fn walkable(&self, x: u32, y: u32) -> bool {
        if !self.in_bounds(x, y) || self.is_occupied(x, y) {
            return false;
        }
        self.map.height_at(x, y) >= 1
            && self.map.cliff.raw(x, y) == 0
            && !matches!(self.map.surface.raw(x, y), Surface::Sea | Surface::Shallow)
    }
}

pub(crate) fn blank_map(seed: Seed, round: u32, cfg: &WorldGenConfig) -> WorldMap {
    let (w, h) = cfg.size;
    WorldMap {
        seed,
        round,
        size: cfg.size,
        height: crate::grid::Grid::new(w, h, 0),
        surface: crate::grid::Grid::new(w, h, Surface::Sea),
        cliff: crate::grid::Grid::new(w, h, 0),
        island_of: crate::grid::Grid::new(w, h, crate::map::NO_ISLAND),
        shore_dist: crate::grid::Grid::new(w, h, 0),
        islands: Vec::new(),
        props: Vec::new(),
        helipads: Vec::new(),
        crates: Vec::new(),
        refugees: Vec::new(),
        metrics: WorldMetrics {
            route_len: 0.0,
            longest_hop: 0.0,
            hidden_crates: 0,
            land_ratio: 0.0,
            terracing: 0.0,
            difficulty: crate::map::Difficulty::Normal,
            difficulty_reason: String::new(),
            gen_ms: 0,
            attempts: 0,
            rejections: Vec::new(),
            stage_ms: Vec::new(),
        },
    }
}

/// Run one full attempt. `round_only` = regenerate crates/refugees on an
/// existing map (spec A6) instead of running stages 1–7.
fn attempt_once(
    seed: Seed,
    round: u32,
    attempt: u32,
    cfg: &WorldGenConfig,
    progress: &dyn Progress,
    round_only: Option<&WorldMap>,
) -> Result<WorldMap, GenError> {
    let regen = round_only.is_some();
    let mut ctx = if let Some(base) = round_only {
        let mut map = base.clone();
        map.round = round;
        map.crates.clear();
        map.refugees.clear();
        let mut occupied = vec![false; (map.size.0 * map.size.1) as usize];
        common::mark_props_and_pads(&map, &mut occupied);
        GenCtx {
            seed,
            round,
            attempt,
            cfg: cfg.clone(),
            map,
            sites: common::sites_from_islands(base),
            occupied,
            progress,
        }
    } else {
        GenCtx {
            seed,
            round,
            attempt,
            cfg: cfg.clone(),
            map: blank_map(seed, round, cfg),
            sites: Vec::new(),
            occupied: vec![false; (cfg.size.0 * cfg.size.1) as usize],
            progress,
        }
    };

    let t0 = Instant::now();
    let mut stage_ms: Vec<(String, u64)> = Vec::new();

    macro_rules! stage {
        ($name:literal, $frac:literal, $f:expr) => {{
            let ts = Instant::now();
            ctx.report($name, $frac);
            ($f)(&mut ctx)?;
            stage_ms.push((String::from($name), ts.elapsed().as_millis() as u64));
        }};
    }

    if regen {
        stage!("Hiding supplies", 0.84, |c| crates::stage(c, true));
        stage!("Finding survivors", 0.92, |c| refugees::stage(c, true));
        stage!("Final checks", 0.98, metrics::stage);
    } else {
        stage!("Charting the sea", 0.02, sites::stage);
        stage!("Charting the sea", 0.08, archetypes::stage);
        stage!("Raising islands", 0.15, terrain::stage);
        stage!("Raising islands", 0.35, cleanup::stage);
        stage!("Painting coastlines", 0.50, surface::stage);
        stage!("Building helipads", 0.60, helipads::stage);
        stage!("Building villages", 0.72, props::stage);
        stage!("Hiding supplies", 0.84, |c| crates::stage(c, false));
        stage!("Finding survivors", 0.92, |c| refugees::stage(c, false));
        stage!("Final checks", 0.98, metrics::stage);
    }

    let mut map = ctx.map;
    map.metrics.gen_ms = t0.elapsed().as_millis() as u64;
    map.metrics.attempts = attempt + 1;
    map.metrics.stage_ms = stage_ms;
    Ok(map)
}

/// Generate a world (spec 02 A6).
pub fn generate(
    seed: Seed,
    round: u32,
    cfg: &WorldGenConfig,
    progress: &dyn Progress,
) -> Result<WorldMap, GenError> {
    cfg.validate()?;
    let mut reasons = Vec::new();
    for attempt in 0..cfg.max_world_attempts {
        match attempt_once(seed, round.max(1), attempt, cfg, progress, None) {
            Ok(mut map) => {
                map.metrics.rejections = reasons;
                return Ok(map);
            }
            Err(GenError::Retry(r)) => reasons.push(format!("a{attempt}: {r}")),
            Err(e) => return Err(e),
        }
    }
    Err(GenError::Exhausted {
        attempts: cfg.max_world_attempts,
        reasons,
    })
}

/// Keep terrain, redo crates/refugees/metrics from `stream(round)` (A6).
pub fn regenerate_round(
    base: &WorldMap,
    round: u32,
    cfg: &WorldGenConfig,
    progress: &dyn Progress,
) -> Result<WorldMap, GenError> {
    cfg.validate()?;
    let mut reasons = Vec::new();
    for attempt in 0..cfg.max_world_attempts {
        match attempt_once(base.seed, round.max(1), attempt, cfg, progress, Some(base)) {
            Ok(mut map) => {
                map.metrics.rejections = reasons;
                return Ok(map);
            }
            Err(GenError::Retry(r)) => reasons.push(format!("a{attempt}: {r}")),
            Err(e) => return Err(e),
        }
    }
    Err(GenError::Exhausted {
        attempts: cfg.max_world_attempts,
        reasons,
    })
}
