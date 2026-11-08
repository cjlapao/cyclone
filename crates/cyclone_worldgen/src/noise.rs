//! Deterministic value noise + fBm (spec 02 A2.6: our own noise, libm maths).
//!
//! Everything here is a pure function of integer coordinates and a u64 seed,
//! using only operations that are bit-identical across platforms: integer
//! hashing, exact power-of-two float divisions, and `libm` for transcendentals.

/// splitmix64 finaliser — strong integer avalanche, no std deps.
#[inline]
pub fn mix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E3779B97F4A7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
    z ^ (z >> 31)
}

/// Combine two u64s deterministically (for stream ids, tile hashes, …).
#[inline]
pub fn hash_pair(a: u64, b: u64) -> u64 {
    mix64(a.wrapping_mul(0x2545_F491_4F6C_DD1D) ^ b.rotate_left(23))
}

/// Hash of a tile coordinate for cosmetic tinting (spec 02 B1 — not an RNG
/// stream, just a pure function of (x, y, seed)).
#[inline]
pub fn tile_hash(x: u32, y: u32, seed: u64) -> u64 {
    hash_pair(seed, (x as u64) << 32 | y as u64)
}

#[inline]
fn lattice(x: i64, y: i64, seed: u64) -> f64 {
    // Map hash to [0,1) using a 53-bit mantissa-safe slice, exact division.
    let h = hash_pair(seed, (x as u64) << 32 ^ (y as u64).wrapping_mul(0x9E3779B1));
    ((h >> 11) as f64) * (1.0 / (1u64 << 53) as f64)
}

#[inline]
fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

/// Classic 2-D value noise in `[0,1]` at world scale (1 unit = 1 tile).
pub fn value_noise(x: f64, y: f64, seed: u64) -> f64 {
    let ix = libm::floor(x);
    let iy = libm::floor(y);
    let fx = x - ix;
    let fy = y - iy;
    let x0 = ix as i64;
    let y0 = iy as i64;
    let v00 = lattice(x0, y0, seed);
    let v10 = lattice(x0 + 1, y0, seed);
    let v01 = lattice(x0, y0 + 1, seed);
    let v11 = lattice(x0 + 1, y0 + 1, seed);
    let sx = smooth(fx);
    let sy = smooth(fy);
    let a = v00 + (v10 - v00) * sx;
    let b = v01 + (v11 - v01) * sx;
    a + (b - a) * sy
}

/// Fractal Brownian motion of `value_noise`, normalised to ~`[0,1]`.
/// `freq` is cycles-per-tile at octave 0.
pub fn fbm(x: f64, y: f64, freq: f64, octaves: usize, seed: u64) -> f64 {
    let mut sum = 0.0f64;
    let mut amp = 1.0f64;
    let mut norm = 0.0f64;
    let mut f = freq;
    for o in 0..octaves {
        sum += value_noise(x * f, y * f, seed.wrapping_add(o as u64)) * amp;
        norm += amp;
        amp *= 0.5; // exact halving: bit-stable
        f *= 2.0;
    }
    sum / norm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_deterministic_and_bounded() {
        let mut min: f64 = 2.0;
        let mut max: f64 = -1.0;
        for i in 0..5000 {
            let x = i as f64 * 0.137;
            let y = (i % 97) as f64 * 0.311;
            let v = fbm(x, y, 0.08, 4, 42);
            assert!(v.is_finite());
            min = min.min(v);
            max = max.max(v);
        }
        assert!(min >= 0.0 && max <= 1.0, "range {min}..{max}");
        // Same inputs, same outputs.
        assert_eq!(fbm(3.7, -2.1, 0.1, 3, 9), fbm(3.7, -2.1, 0.1, 3, 9));
    }
}
