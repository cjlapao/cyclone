//! `WorldMap` and its data types — the generator output (spec 02 A4).
//!
//! Timing fields are `#[serde(skip)]`: RON output must hash identically
//! across runs and platforms (golden test C3), so wall-clock numbers are
//! carried out-of-band. See DECISIONS.md.

use serde::{Deserialize, Serialize};

use crate::grid::Grid;
use crate::seed::Seed;

/// Serialisation result alias so callers don't need `ron` in scope.
pub type GenResult<T> = Result<T, crate::GenError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Surface {
    Sea,
    Shallow,
    Sand,
    Grass,
    Rock,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Archetype {
    Flat,
    Hilly,
    Plateau,
    Peak,
    Atoll,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x0: u32,
    pub y0: u32,
    pub x1: u32,
    pub y1: u32,
}

impl Rect {
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x0 && x <= self.x1 && y >= self.y0 && y <= self.y1
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Island {
    pub name: String,
    pub archetype: Archetype,
    pub centre: (u32, u32),
    pub bbox: Rect,
    pub area: u32,
    pub peak_level: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PropKind {
    House,
    Hut,
    PalmTree,
    PineTree,
    Rock,
    Lighthouse,
    RadioTower,
    Hangar,
    Jetty,
}

impl PropKind {
    /// (footprint_w, footprint_d, height_levels) — the fixed table (spec A4).
    pub const fn dims(self) -> (u8, u8, u8) {
        match self {
            PropKind::House => (2, 2, 2),
            PropKind::Hut => (1, 1, 2),
            PropKind::PalmTree => (1, 1, 3),
            PropKind::PineTree => (1, 1, 4),
            PropKind::Rock => (1, 1, 1),
            PropKind::Lighthouse => (1, 1, 8),
            PropKind::RadioTower => (1, 1, 6),
            PropKind::Hangar => (2, 2, 2),
            PropKind::Jetty => (1, 1, 0),
        }
    }
}

/// `facing` 0..3 = north/east/south/west.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Prop {
    pub kind: PropKind,
    pub tile: (u32, u32),
    pub footprint: (u8, u8),
    pub facing: u8,
    pub height_levels: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Helipad {
    pub tile: (u32, u32), // centre of the 3×3 pad
    pub island: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrateSpawn {
    pub tile: (u32, u32),
    pub island: u8,
    /// Occlusion per camera heading 0..3 (45°, 135°, 225°, 315°).
    pub hidden_from: [bool; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefugeeSpawn {
    pub tile: (u32, u32),
    pub island: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldMetrics {
    /// Nearest-neighbour tour Base → all crates → Base, in tiles.
    pub route_len: f64,
    /// Longest edge of the helipad MST, in tiles.
    pub longest_hop: f64,
    pub hidden_crates: u32,
    pub land_ratio: f64,
    /// 1 − (biggest single height-band share of land). The metric that
    /// actually tracks "does this island look terraced": the user's flat
    /// mesa world scored 0.58 with a level-3 band covering 42% of ALL
    /// land, while the old flat era scored ~0.25. relief_share (share of
    /// land ≥ level 3) read 0.76 on the SAME mesa — it measured height
    /// above sea, not shape, and hid the bug. Round 102 lesson: a metric
    /// you pick yourself can certify nonsense. Tune relief dials against
    /// THIS one; >0.6 = visibly stepped islands.
    pub terracing: f64,
    pub difficulty: Difficulty,
    pub difficulty_reason: String,
    /// Wall-clock fields: kept out of the serialised form so golden hashes
    /// stay stable; still filled for stats.txt.
    #[serde(skip, default)]
    pub gen_ms: u64,
    #[serde(skip, default)]
    pub attempts: u32,
    #[serde(skip, default)]
    pub stage_ms: Vec<(String, u64)>,
    /// Why earlier attempts were rejected (`validate` verdicts), e.g.
    /// `"a0: route 41.2 > fuel_range"`. Empty on one-shot worlds — the
    /// histogram of these is the tuning signal (TUNING.md).
    #[serde(skip, default)]
    pub rejections: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WorldMap {
    pub seed: Seed,
    pub round: u32,
    pub size: (u32, u32),
    /// 0..=max_level.
    pub height: Grid<u8>,
    pub surface: Grid<Surface>,
    /// Bitmask per tile: bit0 N, bit1 E, bit2 S, bit3 W — neighbour ≥2 lower.
    pub cliff: Grid<u8>,
    /// Island index per tile; 255 = sea.
    pub island_of: Grid<u8>,
    /// Tiles to nearest coast, capped at 15.
    pub shore_dist: Grid<u8>,
    /// Index 0 is Base Island.
    pub islands: Vec<Island>,
    pub props: Vec<Prop>,
    pub helipads: Vec<Helipad>,
    /// len == config.crates.
    pub crates: Vec<CrateSpawn>,
    pub refugees: Vec<RefugeeSpawn>,
    pub metrics: WorldMetrics,
}

impl WorldMap {
    /// Sea/shore cliff bit helpers.
    #[inline]
    pub fn height_at(&self, x: u32, y: u32) -> u8 {
        self.height.raw(x, y)
    }

    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new()
            .struct_names(true)
            .new_line("\n");
        ron::ser::to_string_pretty(self, pretty).expect("map always serialises")
    }

    pub fn from_ron(text: &str) -> GenResult<Self> {
        Ok(ron::from_str(text)?)
    }
}

/// Cliff bit order.
pub const CLIFF_N: u8 = 1;
pub const CLIFF_E: u8 = 2;
pub const CLIFF_S: u8 = 4;
pub const CLIFF_W: u8 = 8;

/// `island_of` sentinel for open sea.
pub const NO_ISLAND: u8 = 255;
