//! `Seed` with the Crockford Base32 display form (spec 02 A1).

use std::fmt;
use std::str::FromStr;

use crate::error::GenError;

const ALPHABET: [u8; 32] = *b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";

/// Value of `Seed(u64)`; display form is 13 Crockford Base32 chars,
/// grouped `XXXX-XXXX-XXXXX`.
#[derive(
    Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct Seed(pub u64);

impl Seed {
    /// Draw a fresh seed from OS entropy.
    pub fn random() -> Self {
        let mut buf = [0u8; 8];
        if getrandom::fill(&mut buf).is_err() {
            // Effectively impossible; fall back to a fixed seed rather than
            // crash so headless odd environments still produce a game.
            return Seed(0xC071C1ED);
        }
        Seed(u64::from_le_bytes(buf))
    }
}

impl FromStr for Seed {
    type Err = GenError;

    /// Case-insensitive; ignores dashes/spaces; `O→0`, `I/L→1` per
    /// Crockford. Accepts 1–13 characters (left-padded with zeros).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut v: u64 = 0;
        let mut digits = 0usize;
        for ch in s.chars() {
            if ch == '-' || ch == ' ' || ch == '\t' {
                continue;
            }
            let c = match ch {
                'O' | 'o' => '0',
                'I' | 'i' | 'L' | 'l' => '1',
                c => c.to_ascii_uppercase(),
            };
            let d = ALPHABET
                .iter()
                .position(|&a| a as char == c)
                .ok_or_else(|| GenError::Seed(format!("invalid seed character {ch:?}")))?;
            digits += 1;
            if digits > 13 {
                return Err(GenError::Seed("seed too long (max 13 digits)".into()));
            }
            v = v
                .checked_mul(32)
                .and_then(|x| x.checked_add(d as u64))
                .ok_or_else(|| GenError::Seed("seed overflows 64 bits".into()))?;
        }
        if digits == 0 {
            return Err(GenError::Seed("empty seed".into()));
        }
        Ok(Seed(v))
    }
}

impl fmt::Display for Seed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 13 base-32 digits big-endian; the most significant digit only
        // holds 4 bits of a u64.
        let mut out = [0u8; 13];
        for (i, slot) in out.iter_mut().enumerate() {
            let group = 12 - i; // power of 32 for this position
            let digit = if group == 12 {
                (self.0 >> 60) as usize & 0xF
            } else {
                ((self.0 >> (group * 5)) as usize) & 0x1F
            };
            *slot = ALPHABET[digit];
        }
        let s = core::str::from_utf8(&out).expect("alphabet is ascii");
        // Group XXXX-XXXX-XXXXX
        f.write_str(&s[..4])?;
        f.write_str("-")?;
        f.write_str(&s[4..8])?;
        f.write_str("-")?;
        f.write_str(&s[8..])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_known_values() {
        for s in [
            0u64,
            1,
            31,
            32,
            u32::MAX as u64,
            u64::MAX,
            0x1234_5678_9abc_def0,
        ] {
            let text = Seed(s).to_string();
            let back: Seed = text.parse().unwrap();
            assert_eq!(back.0, s, "round trip failed for {s} ({text})");
        }
    }

    #[test]
    fn parse_forgives_layout_and_lookalikes() {
        let a: Seed = "47Q2-9XMB-4TRFW".parse().unwrap();
        let b: Seed = "47Q2 9XMB 4TRFW".parse().unwrap();
        assert_eq!(a, b);
        // O→0 and I/L→1 lookalikes
        let c: Seed = "O1IL".parse().unwrap();
        assert_eq!(c, "0111".parse::<Seed>().unwrap());
    }

    #[test]
    fn rejects_garbage() {
        assert!("".parse::<Seed>().is_err());
        assert!("U".parse::<Seed>().is_err()); // U is not in Crockford
        assert!("AAAAAAAAAAAAA0".parse::<Seed>().is_err()); // 14 digits
    }
}
