//! `generate` benchmark over seeds 1..=20 (spec 02 C4), filtered to
//! the ones that generate at current dials (18 — Exhausted is a
//! supported outcome now). CI logs the numbers; nothing gates on them.
//! Run: cargo bench -p cyclone_worldgen

use criterion::{Criterion, criterion_group, criterion_main};
use cyclone_worldgen::{ProgressNoop, Seed, WorldGenConfig, generate};

criterion_group! {
    name = benches;
    config = Criterion::default().sample_size(10);
    targets = bench_gen
}
criterion_main!(benches);

fn bench_gen(c: &mut Criterion) {
    let cfg = WorldGenConfig::default();
    // Seeded by measurement, not counting: seeds 1..=20 were fine until
    // the taller terrain made Exhausted a supported outcome (~3% of
    // seeds, game rerolls 10-deep) — seeds 12 and 19 exhaust in 8
    // attempts, and the unwrap panicked the CI bench on an honest
    // worldgen result. Filter to generable seeds at build time; the
    // remaining 18 measure the same work per seed.
    let seeds: Vec<Seed> = (1..=20u64)
        .map(Seed)
        .filter(|&s| generate(s, 1, &cfg, &ProgressNoop).is_ok())
        .collect();
    assert!(
        seeds.len() >= 18,
        "bench seed pool degraded: {}/20",
        seeds.len()
    );
    c.bench_function("generate 160x160 round 1", move |b| {
        b.iter(|| {
            for &s in &seeds {
                let map = std::hint::black_box(generate(s, 1, &cfg, &ProgressNoop).unwrap());
                std::hint::black_box(map.metrics.difficulty);
            }
        })
    });
}
