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
    // Same contract as the golden CLI: a generous attempt budget keeps
    // the FROZEN seed list honest under current dials (one golden seed
    // needs >8 attempts since the taller terrain). The budget is part
    // of the reproduction — a seed succeeding on attempt 12 hashes the
    // attempt-12 streams, so the test must attempt exactly as deep.
    let cfg = WorldGenConfig {
        max_world_attempts: 24,
        ..WorldGenConfig::default()
    };
    for (i, (seed, want)) in expected_rows().iter().enumerate() {
        let seed_parsed: Seed = seed.parse().expect("golden seed parses");
        let map = generate(seed_parsed, 1, &cfg, &ProgressNoop)
            .unwrap_or_else(|e| panic!("golden row {i} ({seed}) failed: {e}"));
        let ron_text = map.to_ron();
        let got = blake3::hash(ron_text.as_bytes()).to_hex().to_string();
        if got != *want {
            // Windows CI once drifted with no other signal: report a
            // fingerprint table so the annotation itself localises the
            // differing region (compare with local values for same seed).
            let lines: Vec<&str> = ron_text.lines().collect();
            let n = lines.len();
            let chunk = |a: usize, b: usize| -> String {
                let joined = lines[a.min(n)..b.min(n)].join("\n");
                blake3::hash(joined.as_bytes()).to_hex()[..8].to_string()
            };
            panic!(
                "golden drift seed {seed}: len={} lines={} h8={} c0={} c1={} c2={} c3={}",
                ron_text.len(),
                n,
                &blake3::hash(ron_text.as_bytes()).to_hex()[..8],
                chunk(0, 200),
                chunk(200, 2000),
                chunk(2000, 20000),
                chunk(20000, n),
            );
        }
    }
}

/// Diagnostic companion: prints the SAME fingerprint table the drift
/// panic emits, so a Windows annotation can be diffed against local
/// (reference) values region by region. Run with --nocapture.
#[test]
#[ignore = "manual diagnostic; prints the golden fingerprint table"]
fn golden_fingerprints() {
    let cfg = WorldGenConfig::default();
    for (seed, want) in expected_rows() {
        let seed_parsed: Seed = seed.parse().unwrap();
        let map = generate(seed_parsed, 1, &cfg, &ProgressNoop).unwrap();
        let ron_text = map.to_ron();
        let lines: Vec<&str> = ron_text.lines().collect();
        let n = lines.len();
        let chunk = |a: usize, b: usize| -> String {
            let joined = lines[a.min(n)..b.min(n)].join("\n");
            blake3::hash(joined.as_bytes()).to_hex()[..8].to_string()
        };
        println!(
            "seed {seed} want={want} len={} lines={} h8={} c0={} c1={} c2={} c3={}",
            ron_text.len(),
            n,
            &blake3::hash(ron_text.as_bytes()).to_hex()[..8],
            chunk(0, 200),
            chunk(200, 2000),
            chunk(2000, 20000),
            chunk(20000, n),
        );
    }
}
