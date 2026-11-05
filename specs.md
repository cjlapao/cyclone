# Cyclone Remake — Agent Build Spec

Nov 5, 2026 · @Car los Lapao

## Mission brief

Build a faithful-in-spirit remake of *Cyclone* (Vortex Software, ZX Spectrum, 1984) as a single native binary for Linux, macOS and Windows. Use Rust with the `macroquad` crate. The player flies a rescue helicopter over an isometric archipelago, recovers five medical supply crates, picks up refugees for bonus points, and returns to Base Island — all while a roaming cyclone, low-flying planes, fuel and a countdown try to stop them.

Ground rules for you, the agent:

- **Recreate the mechanics, not the assets.** Draw every sprite, tile, font and sound yourself, procedurally or as new pixel art. Never download, rip or trace original Spectrum graphics, ROM data or sound.
- **Use a working title**, `Cyclone Rescue`, in the window title and binary name. Keep the name configurable in one constant.
- **Ship playable increments.** Each milestone in the last section must compile, run and be playable before you start the next.
- **Prefer simple over clever.** One crate for windowing, rendering, input and audio. No ECS framework, no engine editor, no asset pipeline beyond `include_bytes!`.
- **Don't stall on gaps.** Where this spec is silent, pick the choice closest to the 1984 original's feel, record it in `DECISIONS.md`, and move on.

The 1984 original's key facts, for reference: 14 islands, 5 crates, unlimited carrying capacity, a north/south view flip, a map screen, refuelling helipads on some islands, a time limit, and a loop that hides crates in harder spots each round ([Wikipedia](<https://en.wikipedia.org/wiki/Cyclone_(video_game)>)).

## Tech stack

Rust plus `macroquad` wins on ease. It builds a single binary with `cargo build --release` on all three platforms, needs no system libraries (no SDL2, no GLFW install), and bundles windowing, 2D drawing, keyboard and gamepad-free input, and simple audio in one crate. C++ would mean wrangling CMake, SDL2 or raylib packaging, and three different toolchains.

| Concern | Choice | Notes |
| --- | --- | --- |
| Language | Rust, stable, edition 2021 | Pin the toolchain in `rust-toolchain.toml` |
| Window, render, input | `macroquad` 0.4.x | Render to a fixed low-res target, scale up with nearest-neighbour |
| Audio | `macroquad::audio` with WAV files generated at build time or by a small `tools/sfx` binary | Fall back to silence if audio init fails |
| RNG | `macroquad::rand` seeded per round | Seed shown on game-over screen for repro |
| Config | `const` values in `src/tuning.rs` | Every gameplay number lives here |
| Tests | `cargo test` on pure logic modules | No rendering in tests |
| Lint | `cargo clippy -- -D warnings`, `cargo fmt --check` | Enforced in CI |

Project layout:

```
cyclone-rescue/
  Cargo.toml
  rust-toolchain.toml
  DECISIONS.md
  README.md
  assets/            # generated sprites and sfx, embedded via include_bytes!
  src/
    main.rs          # window config, main loop, state machine
    tuning.rs        # all gameplay constants
    world.rs         # islands, tiles, heights, buildings, crate and refugee spawns
    worldgen.rs      # deterministic island layout from a seed
    heli.rs          # helicopter physics, fuel, winch
    cyclone.rs       # cyclone movement and force field
    planes.rs        # low-flying aircraft
    iso.rs           # world-to-screen projection, view flip, depth sorting
    render.rs        # drawing the scene, shadows, sprites
    hud.rs           # instrument panel
    map.rs           # map screen
    audio.rs
    state.rs         # Title, Playing, Map, Paused, Crashed, RoundComplete, GameOver
  tests/
    physics.rs
    cyclone.rs
    iso.rs
```

Keep game logic free of `macroquad` calls so you can unit test it. Only `render.rs`, `hud.rs`, `map.rs`, `audio.rs` and `main.rs` may touch the graphics or audio API.

## Game design

One round = find all five crates, then land on Base Island before the clock hits zero. Every number below is a starting value; put it in `tuning.rs` and tune by feel.

### World

- A square sea, roughly 12 × 12 screens, built from 16 × 16 world-unit tiles with an integer height per tile (0 = sea, 1–6 = land and cliffs).
- **14 islands**, generated from a fixed seed so every player sees the same archipelago in round 1. Island 0 is **Base Island**, near the centre.
- Islands carry terrain (beaches, cliffs, hills), buildings (huts, towers, lighthouses) and trees. Tall objects must be able to hide a crate from one viewing direction.
- **Helipads** sit on Base Island and 3–4 others. Landing on one refuels.
- **5 crates** spawn on 5 different islands, never on Base Island. At least 2 per round sit behind a building or cliff that hides them from one view direction.
- **Refugees**: 1–3 per island on 8–10 islands. They wave and stay put.

### Helicopter

- State: position (x, y), altitude z, velocity, heading, fuel, cargo counts.
- Controls move it on x/y with inertia: thrust accelerates, drag slows. Max ground speed about 3 tiles per second.
- Altitude rises and falls at a fixed rate, clamped between 0 and 8 height units. Ground contact = landed if over flat land at low speed, crash if over a slope, building, tree or sea, or if moving too fast.
- **Winch**: hovering within half a tile of a crate or refugee and below altitude 2 auto-lifts it after 0.5 seconds. No capacity limit.
- **Fuel** drains with time airborne, faster at high speed. Landing on a helipad refills over 2 seconds. Zero fuel = engine cuts, helicopter descends; reaching the sea or rough ground = crash.
- A crash costs a life (start with 3) and respawns you on Base Island with collected cargo kept.

### Cyclone

- A moving eye with a radius of influence. It wanders on a slow, smooth path (sum of two sine waves plus a gentle drift toward the player's quadrant), speed about half the helicopter's.
- **Outer zone** (up to \~6 tiles from the eye): applies a swirling force tangent to the eye plus random jitter, so controls feel erratic. Strength scales with closeness.
- **Inner zone** (under \~1.5 tiles): instant crash unless landed on a helipad.
- The HUD shows a proximity warning; the map shows its position and recent track.
- Landing anywhere safe while it passes is a valid tactic — being landed halves the force on you.

### Planes

- Every 15–30 seconds while you're over open sea, a plane crosses the screen in a straight line at a fixed low altitude band (3–5).
- Collision in x, y and z = crash. Climbing above or dropping below its band avoids it.
- HUD warns 2 seconds before it enters the screen and shows the side it comes from.

### Time, score and rounds

- Round timer starts at 5 minutes. HUD shows minutes and seconds.
- Score: 1,000 per crate, 250 per refugee delivered, plus 10 per second left when you land on Base Island with all five crates.
- Refugees count only once you land on Base Island with them aboard.
- Clearing a round loops the game: new seed for crate and refugee spots, more crates hidden behind objects, cyclone 10% faster, timer 20 seconds shorter (minimum 3 minutes).
- Game over when lives or time run out. Keep a local high-score table of 10 entries in the platform's data directory.

## Rendering

Draw everything into a fixed 320 × 240 render target, then scale it to the window by the largest whole number that fits, letterboxed, with nearest-neighbour filtering. That keeps pixels crisp on any monitor.

### Isometric projection

- Use a 2:1 dimetric projection. For a world point (x, y, z): `sx = (x - y) * 1.0`, `sy = (x + y) * 0.5 - z * k`, with `k` the height scale in `tuning.rs`.
- The camera follows the helicopter's ground position with a little lag, clamped to the world edges.
- Draw order: sea, then tiles and objects sorted back to front by `(x + y)` and then by base height, then the helicopter's **shadow** on the ground directly below it, then the helicopter at `z`, then planes, then cyclone effects.
- The helicopter must be visible even behind a tall object: draw a 1-pixel outline of it on top when it's occluded.

### North/South view flip

- One key toggles between the north view and the south view. The south view rotates the world 180° (`x → W - x`, `y → H - y`) before projecting.
- Controls stay screen-relative after a flip: up still moves the helicopter up the screen.
- The HUD shows which view is active. A flip takes 0.25 seconds with a quick fade.

### Look and palette

- Pick a limited palette of about 15 colours inspired by 8-bit home computers (bright cyan sea, yellow sand, green land, white buildings, black outlines). Don't copy the Spectrum's attribute clash; aim for clean modern pixel art.
- The sea animates with a 2–3 frame wave shimmer. Rotor blades spin with 2 frames.
- The cyclone shows as a rotating spiral of dark cloud sprites plus screen shake that grows as it nears.

### HUD

The HUD frames the play area as a border panel. It shows:

| Item | Form |
| --- | --- |
| Speed | Bar gauge |
| Altitude | Vertical bar with tick marks |
| Fuel | Bar gauge, flashes under 20% |
| Time left | mm:ss digits |
| Crates | 5 slots, filled as collected |
| Refugees aboard | Number |
| View | N or S |
| Wind force | 0–9 digit from cyclone proximity |
| Warnings | Flashing CYCLONE and PLANE lamps |
| Score, lives | Digits and helicopter icons |

### Map screen

- A key opens a full-screen top-down map and pauses the game.
- It shows island outlines with numbers, Base Island, helipads, the helicopter, the cyclone eye with its outer ring and a dotted trail of its last 30 seconds.
- Don't show crate positions. Finding them is the game.

## Controls, audio and states

### Controls

| Action | Primary | Alternate |
| --- | --- | --- |
| Move | Arrow keys | W A S D |
| Climb | Space | Q |
| Descend | Left Shift | Z |
| Flip view | V | Tab |
| Map | M |  |
| Pause | P | Esc |
| Quit to title | Esc from pause |  |
| Toggle fullscreen | F11 | Alt+Enter |

Keep bindings in one table in code so remapping later is a one-file change.

### Audio

- Generate every sound yourself: a looping rotor chop whose pitch follows speed, a winch whirr, a pickup chime, a low-fuel beep, a plane whoosh, wind noise that swells with cyclone proximity, a crash burst, and a short round-complete jingle.
- A small `tools/sfx` binary can synthesise these into WAV files with square, noise and envelope generators. Commit the outputs into `assets/`.
- Master volume and mute live on the pause screen.

### Game states

The main loop runs one state at a time:

1. **Title**: name, high scores, press any key.
2. **Playing**: the game. M opens Map, P opens Paused.
3. **Map**: pauses play, any key returns.
4. **Paused**: volume, resume, quit.
5. **Crashed**: 2-second explosion, then respawn on Base Island, or GameOver if no lives remain.
6. **RoundComplete**: score tally, then Playing with the next round's setup.
7. **GameOver**: final score, seed, name entry if it's a high score, then Title.

Run physics on a fixed 60 Hz step with an accumulator so behaviour is identical on any frame rate.

## Milestones, testing and CI

Work through these in order. Each ends with a playable build and a short note in `DECISIONS.md`.

1. **Skeleton**: window opens on all three OSes, 320 × 240 target scales cleanly, state machine switches Title → Playing → Paused. CI green.
2. **Isometric world**: seeded generation of 14 islands, tile heights, buildings, correct depth sorting, camera follow, north/south flip.
3. **Flight**: helicopter movement with inertia, altitude, shadow, landing and crash rules, fuel and helipad refuelling.
4. **Objective**: crates and refugees, winch pickup, Base Island delivery, timer, score, lives, round complete.
5. **Hazards**: cyclone movement and both zones, planes with warnings.
6. **HUD and map**: every HUD item from the table, map screen with cyclone trail.
7. **Sound and polish**: generated SFX, screen shake, sea shimmer, title screen, high scores.
8. **Release**: tagged builds for all platforms attached to a GitHub release.

### Acceptance checks

- [ ] Same seed produces the same island layout and crate spots, verified by a unit test.
- [ ] At least 2 crates per round are invisible from one view direction and visible from the other.
- [ ] Landing on a slope, building or sea always crashes; landing slowly on flat land never does.
- [ ] The inner cyclone zone always crashes an airborne helicopter.
- [ ] Running out of fuel over the sea always crashes.
- [ ] Physics gives identical results at 30, 60 and 144 fps for a recorded input sequence.
- [ ] A full round is completable in under 4 minutes by a skilled player on round 1.
- [ ] Release binary starts in under 2 seconds and holds 60 fps on a 2018-era laptop.
- [ ] No asset is loaded from disk at runtime; the binary runs from any folder.

### Tests

Unit test the pure modules: projection and view flip round-trips in `iso.rs`, landing and crash rules in `heli.rs`, cyclone force falloff in `cyclone.rs`, world generation determinism in `worldgen.rs`. Add a headless replay test that feeds a scripted input list into the simulation and checks the final state.

### CI

Use GitHub Actions with a matrix of `ubuntu-latest`, `macos-latest` and `windows-latest`:

- On every push: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `cargo build --release`.
- On a `v*` tag: build release binaries for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, and a universal macOS binary (`aarch64-apple-darwin` and `x86_64-apple-darwin` merged with `lipo`), zip each, and attach them to the release.
- Linux runners need `libasound2-dev`, `libx11-dev`, `libxi-dev` and `libgl1-mesa-dev` installed for macroquad to build.
- Note in the README that macOS users must right-click → Open the first time, since the binary isn't notarised.
