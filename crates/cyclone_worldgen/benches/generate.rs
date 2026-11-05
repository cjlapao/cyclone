//! `generate` benchmark on 20 fixed seeds (spec 02 C4). CI logs the
//! numbers; nothing gates on them.
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
    let seeds: Vec<Seed> = (1..=20u64).map(Seed).collect();
    c.bench_function("generate 160x160 round 1", move |b| {
        b.iter(|| {
            for &s in &seeds {
                let map = std::hint::black_box(generate(s, 1, &cfg, &ProgressNoop).unwrap());
                std::hint::black_box(map.metrics.difficulty);
            }
        })
    });
}
