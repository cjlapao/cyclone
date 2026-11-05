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
| Flip view (N/S) | V | Tab |
| Map | M | |
| Pause | P | Esc |
| Quit to title | Esc (while paused) | |
| Fullscreen | F11 | Alt+Enter |

## Development

```sh
cargo test          # pure-logic unit tests (no rendering)
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Gameplay constants live in `src/tuning.rs`. Design deviations are recorded in
`DECISIONS.md`.
