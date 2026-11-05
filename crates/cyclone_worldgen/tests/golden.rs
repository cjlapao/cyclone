//! Golden determinism test (spec 02 C3): regenerate the fixed seeds and
//! compare BLAKE3 of `WorldMap::to_ron()`. If a deliberate generator change
//! breaks this, regenerate with
//! `cargo run -p worldgen-cli -- golden --update` in the same commit and
//! note the change in DECISIONS.md.

use cyclone_worldgen::{ProgressNoop, Seed, WorldGenConfig, generate};

fn expected_rows() -> Vec<(String, String)> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/golden.ron");
    let text = std::fs::read_to_string(path).expect("tests/golden.ron committed");
    text.lines()
        .filter(|l| l.contains("seed:"))
        .map(|l| {
            let seed = l.split('"').nth(1).expect("seed string").to_string();
            let hash = l.split('"').nth(3).expect("hash string").to_string();
            (seed, hash)
        })
        .collect()
}

#[test]
fn golden_worlds_reproduce_exactly() {
    let cfg = WorldGenConfig::default();
    for (i, (seed, want)) in expected_rows().iter().enumerate() {
        let seed_parsed: Seed = seed.parse().expect("golden seed parses");
        let map = generate(seed_parsed, 1, &cfg, &ProgressNoop)
            .unwrap_or_else(|e| panic!("golden row {i} ({seed}) failed: {e}"));
        let got = blake3::hash(map.to_ron().as_bytes()).to_hex().to_string();
        assert_eq!(got, *want, "golden drift on seed {seed}");
    }
}
