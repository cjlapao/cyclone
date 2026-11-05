//! Quick reliability check: seed N worlds, report failure stats + timing.
use cyclone_worldgen::*;
use std::time::Instant;

fn main() {
    let n: u64 = std::env::args()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1000);
    let cfg = WorldGenConfig::default();
    let mut fails = 0;
    let t = Instant::now();
    let mut reasons = std::collections::BTreeMap::new();
    for i in 0..n {
        let seed = Seed(i.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xC0FFEE);
        match generate(seed, 1, &cfg, &ProgressNoop) {
            Ok(_) => {}
            Err(e) => {
                fails += 1;
                if let GenError::Exhausted { reasons: rs, .. } = &e
                    && let Some(r) = rs.last()
                {
                    *reasons
                        .entry(r.split(':').next().unwrap_or("?").to_string())
                        .or_insert(0u32) += 1;
                }
            }
        }
        if i % 100 == 99 {
            println!(
                "{:>5} done {:>3} fails {:.1}ms/world",
                i + 1,
                fails,
                t.elapsed().as_micros() as f64 / 1000.0 / (i + 1) as f64
            );
        }
    }
    println!(
        "TOTAL {n} fails {fails} ({:.2}%)",
        fails as f64 * 100.0 / n as f64
    );
    for (k, v) in reasons {
        println!("  {v:4}  {k}");
    }
}
