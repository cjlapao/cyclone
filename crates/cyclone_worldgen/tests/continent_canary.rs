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
    let m = c.metrics;
    let digest = h.finalize().to_string();
    // Re-pinned round 176 (relief rebalance 1100→300 m) on Linux. CI runs this SAME test on
    // macOS + Windows: agreement there is the cross-platform identity
    // proof the island golden gives the map; drift here = platform
    // libm/dial drift, red before any human eye is asked to judge.
    assert_eq!(
        digest,
        "f905b36ec061e31ca9916e4db5cd844be1ed7ae8469c3d9d29a6f781d1d5f2a6"
    );
    // Metrics are ratios of integer counts — a one-column change moves
    // them ~4e-6, so 1e-9 tolerance still sees every single-column
    // drift while surviving decimal round-trip of these literals.
    // (The DIGEST is the bit-exact witness; these are tripwires.)
    assert!(m.ocean_ratio == 0.0);
    assert!((m.flat_ratio - 0.038093566895).abs() < 1e-9);
    assert!((m.high_ratio - 0.684650421143).abs() < 1e-9);
    assert!((m.mountain_ratio - 0.017299652100).abs() < 1e-9);
    assert_eq!(m.max_height_m, 236.25); // voxel multiples: exact in f64
}

fn bytemuck_bytes(v: &[u16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 2);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}
