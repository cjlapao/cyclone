//! Embedded chiptune SFX + looping rotor hum. All WAVs are synthesised by
//! tools/gen_sfx.rs (original material) and baked in with include_bytes!.

use crate::game::Sfx;
use macroquad::audio::{
    load_sound_from_bytes, play_sound, play_sound_once, stop_sound, PlaySoundParams, Sound,
};

static WAVS: &[(&str, &[u8])] = &[
    ("pickup", include_bytes!("../assets/pickup.wav")),
    ("deliver", include_bytes!("../assets/deliver.wav")),
    ("crash", include_bytes!("../assets/crash.wav")),
    ("warn", include_bytes!("../assets/warn.wav")),
    ("land", include_bytes!("../assets/land.wav")),
    ("refuel", include_bytes!("../assets/refuel.wav")),
    ("flip", include_bytes!("../assets/flip.wav")),
    ("round", include_bytes!("../assets/round.wav")),
    ("over", include_bytes!("../assets/over.wav")),
    ("rotor1", include_bytes!("../assets/rotor1.wav")),
    ("rotor2", include_bytes!("../assets/rotor2.wav")),
    ("rotor3", include_bytes!("../assets/rotor3.wav")),
];

pub struct Audio {
    one: Vec<Sound>,
    rotors: Vec<Sound>,
    cur_rotor: isize,
    muted: bool,
}

impl Audio {
    pub async fn load() -> Self {
        let mut one = Vec::new();
        let mut rotors = Vec::new();
        for (name, bytes) in WAVS {
            let snd = load_sound_from_bytes(bytes)
                .await
                .unwrap_or_else(|e| panic!("bad embedded wav {name}: {e}"));
            if name.starts_with("rotor") {
                rotors.push(snd);
            } else {
                one.push(snd);
            }
        }
        Self {
            one,
            rotors,
            cur_rotor: -1,
            muted: false,
        }
    }

    fn sfx_idx(ev: Sfx) -> usize {
        match ev {
            Sfx::Pickup => 0,
            Sfx::Deliver => 1,
            Sfx::Crash => 2,
            Sfx::Warn => 3,
            Sfx::Land => 4,
            Sfx::Refuel => 5,
            Sfx::Flip => 6,
            Sfx::Round => 7,
            Sfx::Over => 8,
        }
    }

    pub fn play(&self, ev: Sfx) {
        if self.muted {
            return;
        }
        play_sound_once(&self.one[Self::sfx_idx(ev)]);
    }

    /// Keep the rotor hum running; bucket by ground speed. On the ground the
    /// rotors "stop" (silence) like the original's parked heli.
    pub fn rotor(&mut self, flying: bool, speed01: f32) {
        let want = if flying && !self.muted {
            ((speed01.clamp(0.0, 1.0) * 2.999) as usize).min(2)
        } else {
            99
        };
        if want as isize == self.cur_rotor {
            return;
        }
        if self.cur_rotor >= 0 {
            stop_sound(&self.rotors[self.cur_rotor as usize]);
        }
        self.cur_rotor = if want < 3 {
            play_sound(
                &self.rotors[want],
                PlaySoundParams {
                    looped: true,
                    volume: 0.30,
                },
            );
            want as isize
        } else {
            -1
        };
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
        if self.muted && self.cur_rotor >= 0 {
            stop_sound(&self.rotors[self.cur_rotor as usize]);
            self.cur_rotor = -1;
        }
    }
}
