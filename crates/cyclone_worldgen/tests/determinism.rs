//! Spec 02 Part C — determinism + invariant tests (C1–C2 level coverage).

use cyclone_worldgen::*;
use proptest::prelude::*;

fn gen_world(seed: Seed, round: u32) -> WorldMap {
    let cfg = WorldGenConfig::default();
    generate(seed, round, &cfg, &ProgressNoop).expect("generation must succeed")
}

fn snapshot(m: &WorldMap) -> String {
    // Everything except wall-clock fields (already skipped in serde).
    ron::ser::to_string_pretty(m, ron::ser::PrettyConfig::new().depth_limit(2))
        .expect("WorldMap must serialise")
}

#[test]
fn same_seed_same_world_across_runs() {
    for s in [7u64, 1234, 999_999, u64::MAX - 5] {
        let a = snapshot(&gen_world(Seed(s), 1));
        let b = snapshot(&gen_world(Seed(s), 1));
        assert_eq!(a, b, "world for seed {s} drifted between runs");
    }
}

#[test]
fn different_seeds_different_worlds() {
    let a = snapshot(&gen_world(Seed(1), 1));
    let b = snapshot(&gen_world(Seed(2), 1));
    assert_ne!(a, b);
}

#[test]
fn invariants_hold_over_many_seeds() {
    let cfg = WorldGenConfig::default();
    for s in 0..300u64 {
        let m = gen_world(Seed(0xA5A5_0000 + s), 1);
        let issues = validate(&m, &cfg);
        assert!(issues.is_empty(), "seed {s}: {}", issues.join("; "));

        assert_eq!(m.islands.len(), cfg.island_count as usize);
        assert_eq!(m.crates.len(), cfg.crates as usize);
        assert!(!m.refugees.is_empty());
        assert!(
            m.helipads.len() >= 3,
            "seed {s}: only {} pads",
            m.helipads.len()
        );
        let hidden = m
            .crates
            .iter()
            .filter(|c| c.hidden_from.iter().any(|b| *b))
            .count();
        assert!(
            hidden >= cfg.hidden_crates_min_for_round(1) as usize,
            "seed {s}: {hidden} hidden crates"
        );
        for (i, isl) in m.islands.iter().enumerate() {
            assert!(isl.area >= 8, "seed {s}: island {i} too small");
        }
    }
}

#[test]
fn regenerate_round_keeps_terrain_changes_spawns() {
    let cfg = WorldGenConfig::default();
    let seed = Seed(31337);
    let m1 = gen_world(seed, 1);
    let m2 = regenerate_round(&m1, 2, &cfg, &ProgressNoop).expect("regen must succeed");

    assert_eq!(m2.round, 2);
    assert_eq!(m1.height, m2.height);
    assert_eq!(m1.islands, m2.islands);
    assert_eq!(m1.helipads, m2.helipads);
    assert_eq!(m1.props.len(), m2.props.len());
    assert_ne!(
        m1.crates.iter().map(|c| c.tile).collect::<Vec<_>>(),
        m2.crates.iter().map(|c| c.tile).collect::<Vec<_>>()
    );
    let hidden = m2
        .crates
        .iter()
        .filter(|c| c.hidden_from.iter().any(|b| *b))
        .count();
    assert!(hidden >= cfg.hidden_crates_min_for_round(2) as usize);
    let issues = validate(&m2, &cfg);
    assert!(issues.is_empty(), "regen: {}", issues.join("; "));
}

#[test]
fn regeneration_is_deterministic_too() {
    let cfg = WorldGenConfig::default();
    let m1 = gen_world(Seed(8), 1);
    let a = snapshot(&regenerate_round(&m1, 2, &cfg, &ProgressNoop).unwrap());
    let b = snapshot(&regenerate_round(&m1, 2, &cfg, &ProgressNoop).unwrap());
    assert_eq!(a, b);
}

#[test]
fn custom_configs_respect_bounds() {
    let cfg = WorldGenConfig {
        island_count: 8,
        size: (96, 96),
        crates: 5,
        ..WorldGenConfig::default()
    };
    let m = generate(Seed(42), 1, &cfg, &ProgressNoop).unwrap();
    assert_eq!(m.size, (96, 96));
    assert_eq!(m.islands.len(), 8);
    assert_eq!(m.crates.len(), 5);
}

proptest! {
    #[test]
    fn seed_text_round_trips(s in any::<u64>()) {
        let seed = Seed(s);
        let text = seed.to_string();
        let back: Seed = text.parse().unwrap();
        prop_assert_eq!(back, seed);
    }

    #[test]
    fn empty_seed_rejects(s in "[^0-9A-Za-z -]+") {
        prop_assert!("".parse::<Seed>().is_err());
        prop_assert!(s.parse::<Seed>().is_err());
    }
}
