//! Offline SFX generator: `rustc -O tools/gen_sfx.rs -o target/gen_sfx && ./target/gen_sfx`
//! Writes original 8-bit-style WAV effects into assets/ (checked in so the
//! game builds with include_bytes! without running this tool).
//! Everything here is synthesised from scratch — no samples, no assets to copy.

use std::f32::consts::TAU;
use std::fs;
use std::path::Path;

const SR: u32 = 22_050;

struct Wav {
    s: Vec<f32>,
}

impl Wav {
    fn new() -> Self {
        Wav { s: Vec::new() }
    }
    fn at(&mut self, t: f32, v: f32) {
        let i = (t * SR as f32) as usize;
        if i >= self.s.len() {
            self.s.resize(i + 1, 0.0);
        }
        self.s[i] = (self.s[i] + v).clamp(-1.0, 1.0);
    }
    /// sine with exponential decay, hard-clipped for the chiptune bite
    fn tone(&mut self, t0: f32, dur: f32, f0: f32, f1: f32, vol: f32, decay: f32) {
        let n = (dur * SR as f32) as usize;
        for i in 0..n {
            let t = i as f32 / SR as f32;
            let f = f0 + (f1 - f0) * (t / dur);
            let e = (1.0 - t / dur).powf(decay);
            let v = (t * f * TAU).sin() * vol * e;
            self.at(t0 + t, v.signf() * v.abs().powf(0.6));
        }
    }
    fn noise(&mut self, t0: f32, dur: f32, vol: f32, decay: f32) {
        let n = (dur * SR as f32) as usize;
        let mut x = 0x9E37_79B9u32;
        for i in 0..n {
            let t = i as f32 / SR as f32;
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            let r = (x as f32 / u32::MAX as f32) * 2.0 - 1.0;
            let e = (1.0 - t / dur).powf(decay);
            self.at(t0 + t, r * vol * e);
        }
    }
    /// seamless loop: integer number of blade passes in dur
    fn rotor(&mut self, dur: f32, blade_hz: f32, vol: f32) {
        let n = (dur * SR as f32) as usize;
        for i in 0..n {
            let t = i as f32 / SR as f32;
            let p = (t * blade_hz) % 1.0;
            // chop: sharp pulse per blade + low thump harmonic
            let chop = if p < 0.12 { (0.12 - p) / 0.12 } else { 0.0 };
            let v = chop * vol + (t * blade_hz * TAU).sin() * vol * 0.45;
            self.at(t, v);
        }
    }
    fn save(&self, path: &str) {
        let mut out: Vec<u8> = Vec::new();
        let data = (self.s.len() * 2) as u32;
        out.extend_from_slice(b"RIFF");
        out.extend_from_slice(&(36 + data).to_le_bytes());
        out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes()); // PCM
        out.extend_from_slice(&1u16.to_le_bytes()); // mono
        out.extend_from_slice(&SR.to_le_bytes());
        out.extend_from_slice(&(SR * 2).to_le_bytes());
        out.extend_from_slice(&2u16.to_le_bytes());
        out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data");
        out.extend_from_slice(&data.to_le_bytes());
        for &v in &self.s {
            out.extend_from_slice(&((v * 32000.0) as i16).to_le_bytes());
        }
        fs::write(path, out).expect("write wav");
        println!("wrote {path} ({} samples)", self.s.len());
    }
}

trait Signf {
    fn signf(&self) -> f32;
}
impl Signf for f32 {
    fn signf(&self) -> f32 {
        if *self < 0.0 {
            -1.0
        } else {
            1.0
        }
    }
}

fn main() {
    fs::create_dir_all(Path::new("assets")).unwrap();

    let mut w = Wav::new();
    w.tone(0.0, 0.18, 420.0, 880.0, 0.7, 1.4);
    w.save("assets/pickup.wav");

    let mut w = Wav::new();
    w.tone(0.0, 0.12, 523.0, 523.0, 0.6, 0.6);
    w.tone(0.12, 0.12, 659.0, 659.0, 0.6, 0.6);
    w.tone(0.24, 0.22, 784.0, 784.0, 0.7, 1.0);
    w.save("assets/deliver.wav");

    let mut w = Wav::new();
    w.noise(0.0, 0.55, 0.9, 1.6);
    w.tone(0.0, 0.4, 180.0, 40.0, 0.8, 1.2);
    w.save("assets/crash.wav");

    let mut w = Wav::new();
    for k in 0..3 {
        w.tone(k as f32 * 0.22, 0.09, 660.0, 660.0, 0.55, 0.2);
        w.tone(k as f32 * 0.22 + 0.11, 0.09, 440.0, 440.0, 0.55, 0.2);
    }
    w.save("assets/warn.wav");

    let mut w = Wav::new();
    w.tone(0.0, 0.1, 130.0, 90.0, 0.8, 2.2);
    w.noise(0.0, 0.05, 0.25, 2.0);
    w.save("assets/land.wav");

    let mut w = Wav::new();
    w.tone(0.0, 0.16, 880.0, 1320.0, 0.4, 1.0);
    w.save("assets/refuel.wav");

    let mut w = Wav::new();
    w.noise(0.0, 0.14, 0.4, 1.0);
    w.tone(0.0, 0.14, 900.0, 200.0, 0.3, 1.6);
    w.save("assets/flip.wav");

    let mut w = Wav::new();
    w.tone(0.0, 0.12, 523.0, 523.0, 0.6, 0.4);
    w.tone(0.12, 0.12, 659.0, 659.0, 0.6, 0.4);
    w.tone(0.24, 0.12, 784.0, 784.0, 0.6, 0.4);
    w.tone(0.36, 0.4, 1047.0, 1047.0, 0.7, 1.2);
    w.save("assets/round.wav");

    let mut w = Wav::new();
    w.tone(0.0, 0.35, 392.0, 330.0, 0.6, 0.5);
    w.tone(0.35, 0.35, 330.0, 262.0, 0.6, 0.5);
    w.tone(0.7, 0.7, 262.0, 98.0, 0.7, 1.2);
    w.save("assets/over.wav");

    for (i, hz) in [40.0f32, 56.0, 72.0].iter().enumerate() {
        let mut w = Wav::new();
        w.rotor(0.62, *hz, 0.5);
        w.save(&format!("assets/rotor{}.wav", i + 1));
    }
    println!("done");
}
