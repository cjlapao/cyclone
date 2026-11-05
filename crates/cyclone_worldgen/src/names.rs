//! Deterministic island names (spec 02 A4) so the HUD can say
//! "Crate spotted near Orakey Point".

use rand::Rng;
use rand_chacha::ChaCha8Rng;

const ONSET: &[&str] = &[
    "Kel", "Mar", "Vor", "Tor", "Nuk", "Alo", "Bra", "Sil", "Kar", "Pua", "Drak", "Fen", "Gua",
    "Hia", "Jav", "Kora", "Lum", "Mor", "Neo", "Ora", "Pal", "Quiv", "Rapa", "Sola", "Tar", "Umi",
    "Vai", "Wai", "Xan", "Yel", "Zan",
];

const MIDDLE: &[&str] = &[
    "ora", "vin", "ata", "esi", "oku", "ila", "uru", "ema", "iva", "ano", "oro", "uke", "asi",
    "eva", "ino", "",
];

const TAIL: &[&str] = &[
    "Isle", "Key", "Point", "Reef", "Cay", "Atoll", "Rock", "Shoal", "Bay", "Haven",
];

/// Build one name from the syllable tables using the island's own stream.
pub fn make_name(rng: &mut ChaCha8Rng) -> String {
    let a = ONSET[rng.random_range(0..ONSET.len())];
    let b = MIDDLE[rng.random_range(0..MIDDLE.len())];
    let c = TAIL[rng.random_range(0..TAIL.len())];
    format!("{a}{b} {c}")
}

pub const BASE_ISLAND_NAME: &str = "Base Island";
