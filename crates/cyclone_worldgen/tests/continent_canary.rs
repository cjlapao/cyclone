//! Cross-platform identity pin for the terrain core (the continent
//! analogue of the map golden): height bytes + metrics digest, run on
//! every OS in CI — if libm or a dial ever drifts platform-wise, this
//! goes red BEFORE the pilot's eyeballs can be blamed.
//!
//! Contract note (round 117 doctrine): the seed list is frozen; a
//! dial change REDs this test *by design*. Regenerating the constant
//! is a deliberate act: rerun locally, diff the hillshade, then edit.

use cyclone_worldgen::{ContinentConfig, generate_continent};

#[test]
fn continent_seed2_512_digest() {
    let c = generate_continent(
        2,
        &ContinentConfig {
            size: 512,
            ..Default::default()
        },
    )
    .unwrap();
    let mut h = blake3::Hasher::new();
    h.update(&bytemuck_bytes(&c.heights));
    // Rivers are terrain truth now: the water channel rides the
    // same digest — a platform that carves differently drifts red.
    h.update(&bytemuck_bytes(&c.water));
    let m = c.metrics;
    let digest = h.finalize().to_string();
    // Re-pinned round 177 (rivers: water channel carved) on Linux. CI runs this SAME test on
    // macOS + Windows: agreement there is the cross-platform identity
    // proof the island golden gives the map; drift here = platform
    // libm/dial drift, red before any human eye is asked to judge.
    assert_eq!(
        digest,
        "1f9ee0daca6b1f5a2e2ab1ab6201b5d8d1c4e35aa070ef05d47927d03b49c04d"
    );
    // Metrics are ratios of integer counts — a one-column change moves
    // them ~4e-6, so 1e-9 tolerance still sees every single-column
    // drift while surviving decimal round-trip of these literals.
    // (The DIGEST is the bit-exact witness; these are tripwires.)
    // River carving nudges flat (valleys steepen) and adds the
    // river channel itself; the rest hold — the carve is local.
    assert!(m.ocean_ratio == 0.0);
    assert!((m.flat_ratio - 0.037986755371).abs() < 1e-9);
    assert!((m.high_ratio - 0.684650421143).abs() < 1e-9);
    assert!((m.mountain_ratio - 0.017299652100).abs() < 1e-9);
    assert!((m.river_ratio - 0.002746582031).abs() < 1e-9);
    assert_eq!(m.max_height_m, 236.25); // voxel multiples: exact in f64
}

fn bytemuck_bytes(v: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 2);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}
