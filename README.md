# Cyclone Rescue

A faithful-in-spirit remake of the 1984 rescue-helicopter classic: fly over a
procedural isometric archipelago, recover five medical crates, pick up
refugees, and beat the clock back to Base Island while a cyclone roams the
map. All sprites, tiles, fonts and sounds are original work.

Built in Rust with [macroquad](https://github.com/not-fl3/macroquad); ships as
a single binary for Linux, macOS and Windows.

## Build & run

```sh
cargo run            # debug
cargo build --release && ./target/release/cyclone-rescue
```

Linux build dependencies (Debian/Ubuntu names): `libasound2-dev libx11-dev
libxi-dev libgl1-mesa-dev` — only needed for the windowing/audio layer.

macOS: the release binary is unsigned and un-notarised; right-click → Open the
first time you run it.

## Controls

| Action | Primary | Alternate |
| --- | --- | --- |
| Move | Arrows | W A S D |
| Climb | Space | Q |
| Descend | Left Shift | Z |
| Winch (lift cargo / people when low, land on pads) | E | Enter |
| Flip view (N/S) | V | Tab |
| Map | M | |
| Mute sound | K | |
| Pause | P | Esc |
| Quit to title | Esc (while paused) | |
| Fullscreen | F11 | Alt+Enter |

Movement is *screen-relative*: Up always means "away from the camera", so
after flipping to the South view the stick inverts with the world, exactly as
in the original.

## Sound

All effects and the rotor hum are synthesised from scratch by
`tools/gen_sfx.rs`; the resulting WAVs in `assets/` are embedded in the
binary with `include_bytes!` — there is no runtime data folder.

Audio is the `audio` cargo feature (on by default). On machines without the
ALSA development headers, build without it:

```sh
cargo build --no-default-features   # silent but otherwise identical game
```

## Development

```sh
cargo test --no-default-features          # logic + acceptance tests
cargo clippy --no-default-features --all-targets -- -D warnings
cargo fmt --check
rustc -O tools/gen_sfx.rs -o target/gen_sfx && ./target/gen_sfx   # regen SFX
```

The crate is a lib + thin bin split, so `tests/` drives the exact same
fixed-step simulation the binary runs (`tests/physics.rs` proves identical
results at 30/60/144 fps).

Gameplay constants live in `src/tuning.rs`. Design deviations are recorded in
`DECISIONS.md`.
