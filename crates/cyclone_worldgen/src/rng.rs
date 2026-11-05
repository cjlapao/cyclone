//! Seeded RNG streams (spec 02 A2.1–2). `ChaCha8Rng` only — its stream
//! counters give us "adding a draw in one stage never shifts another".

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

/// The one and only RNG constructor in the generator.
pub fn stream(seed: u64, stream_id: u64) -> ChaCha8Rng {
    let mut r = ChaCha8Rng::seed_from_u64(seed);
    r.set_stream(stream_id);
    r
}

/// Stream id for (world attempt, stage). Attempts must change streams
/// (A2.2 via the driver note in spec 02 A5).
pub fn stage_stream_id(attempt: u32, stage: u32) -> u64 {
    u64::from(attempt + 1) * 1000 + u64::from(stage)
}

/// Intra-stage retry stream (spec 02 A2.2: `stage_id * 1000 + attempt`).
pub fn retry_stream_id(stage_id: u64, retry: u32) -> u64 {
    stage_id * 1000 + u64::from(retry)
}

/// A round-salted variant of a stream id — used by `regenerate_round` so
/// later rounds re-roll crates/refugees from `stream(round)` without
/// disturbing the shared terrain streams.
pub fn salt_for_round(stream_id: u64, round: u32) -> u64 {
    crate::noise::hash_pair(
        stream_id,
        u64::from(round).wrapping_mul(0x9E37_79B9_7F4A_7C15),
    )
}
