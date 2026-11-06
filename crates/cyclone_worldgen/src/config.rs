//! `WorldGenConfig` — tunable generator parameters (spec 02 A3).

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WorldGenConfig {
    /// World size in tiles (1 tile = 1 world unit).
    pub size: (u32, u32),
    /// Open sea border kept clear of land.
    pub margin: u32,
    /// Island count including Base Island.
    pub island_count: u32,
    /// Per-island base radius range.
    pub island_radius: (u32, u32),
    /// Base Island radius range.
    pub base_radius: (u32, u32),
    /// Minimum open water between island coastlines.
    pub min_gap: u32,
    /// Height levels: 0 sea, 1 beach, 2..=max_level land.
    pub max_level: u8,
    /// World height per level.
    pub level_height: f32,
    /// Full-tank flight distance in tiles (shared with spec 03 tuning).
    pub fuel_range: u32,
    /// Helipads beyond Base Island.
    pub helipads_extra: (u32, u32),
    /// Crate count.
    pub crates: u32,
    /// Crates hidden from at least one camera heading (floor; round raises it).
    pub hidden_crates_min: u32,
    /// Islands that get refugees.
    pub refugee_islands: (u32, u32),
    /// Refugees per refugee island.
    pub refugees_per_island: (u32, u32),
    /// Whole-world retries before giving up.
    pub max_world_attempts: u32,
}

impl Default for WorldGenConfig {
    fn default() -> Self {
        Self {
            size: (160, 160),
            margin: 12,
            island_count: 14,
            island_radius: (6, 16),
            base_radius: (10, 12),
            min_gap: 5,
            max_level: 7,
            level_height: 0.5,
            fuel_range: 70,
            helipads_extra: (3, 4),
            crates: 5,
            hidden_crates_min: 2,
            refugee_islands: (8, 10),
            refugees_per_island: (1, 3),
            max_world_attempts: 8,
        }
    }
}

impl WorldGenConfig {
    /// Load from RON text (`assets/worldgen/default.ron`).
    pub fn from_ron(text: &str) -> Result<Self, crate::GenError> {
        Ok(ron::from_str(text)?)
    }

    pub fn to_ron(&self) -> String {
        let pretty = ron::ser::PrettyConfig::new()
            .struct_names(true)
            .new_line("\n");
        ron::ser::to_string_pretty(self, pretty).expect("config always serialises")
    }

    /// Round 3+ hides 3 crates, round 5+ hides 4 (spec 02 A3).
    pub fn hidden_crates_min_for_round(&self, round: u32) -> u32 {
        let bump = match round {
            0..=2 => 0,
            3..=4 => 3,
            _ => 4,
        };
        self.hidden_crates_min.max(bump).min(self.crates)
    }

    /// Reject nonsense early so stages can assume sane ranges.
    pub fn validate(&self) -> Result<(), crate::GenError> {
        let bad = self.size.0 < 40
            || self.size.1 < 40
            || self.margin * 3 >= self.size.0.min(self.size.1)
            || self.island_count < 3
            || self.island_radius.0 > self.island_radius.1
            || self.island_radius.0 == 0
            || self.base_radius.0 > self.base_radius.1
            || self.max_level < 3
            || self.level_height <= 0.0
            || self.fuel_range < 10
            || self.crates + 1 > self.island_count
            || self.helipads_extra.1 + 1 >= self.island_count
            // refugee_islands is clamped to the available islands by stage 9
            || self.refugees_per_island.0 > self.refugees_per_island.1
            || self.refugees_per_island.0 == 0
            || self.max_world_attempts == 0
            || self.min_gap == 0;
        if bad {
            return Err(crate::GenError::Config(format!(
                "config out of range: {self:?}"
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid_and_ron_round_trips() {
        let c = WorldGenConfig::default();
        c.validate().unwrap();
        let text = c.to_ron();
        let back = WorldGenConfig::from_ron(&text).unwrap();
        assert_eq!(back, c);
    }

    #[test]
    fn hidden_min_rises_with_round() {
        let c = WorldGenConfig::default();
        assert_eq!(c.hidden_crates_min_for_round(1), 2);
        assert_eq!(c.hidden_crates_min_for_round(3), 3);
        assert_eq!(c.hidden_crates_min_for_round(6), 4);
    }
}
