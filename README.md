# Cyclone Rescue (V2)

Search-and-rescue over a generated archipelago. Bevy **0.20.1** (exact pin).
V1 lives in [`v1/`](v1/); the V2 specs are in [`V2/`](V2/).

## Run

```sh
cargo run -p cyclone        # toolchain: ~/.rustup/toolchains/stable-*/bin
```

Menu → **New Game** → generating screen (live progress) → world preview
(orbiting, info panel) → **Enter** to fly around the world freely.

| Key | Action |
| --- | --- |
| `W A S D` | fly along the view (free camera) |
| `Space` / `Shift` | climb / descend |
| `Ctrl` | fly boost |
| **hold right mouse** + move | aim the camera |
| wheel | zoom (FOV) |
| `Q` / `E` | snap-turn 90° (crates re-hide as you turn) |
| `Enter` | confirm / start flying |
| `R` | reroll a new world (preview) |
| `C` | copy seed to clipboard (preview) |
| `Tab` | preview: flat top-down map toggle |
| `F3` | **dev builds only** (`--features dev`): reveal crates on preview — rotate with Q/E to watch them hide per heading |
| — | the **Base Island is warm-tinted** gold in every 3D view |
| `Esc` | back |

**Gamepad**: menus and flying are fully mappable to a pad (D-pad/South
etc.). Device I/O is optional: build with `--features gamepad` (on Linux
first `sudo apt install pkg-config libudev-dev`; macOS/Windows need
nothing). Without it everything still works by keyboard; the mapping
itself is event-tested in CI either way.

Seed formats accepted in “Play Seed…”: 16 hex digits or the 13-char dashed
form shown in-game. Worldgen knobs: `assets/worldgen/default.ron`;
keys/display/vsync/fullscreen: `~/.config/rescue/settings.ron` on Linux
(`%APPDATA%\cyclone\rescue\` on Windows, `~/Library/Application Support/com.cyclone.rescue/`
on macOS — applies live, see `DECISIONS.md`; key remaps live there too).
Display/input options also live in-menu: **Settings** row opens an overlay
(↑↓ rows · ←→ or Enter adjust · **click a row** to adjust · Esc close) — fullscreen, vsync, window
size, camera speed; edits apply the same frame and persist. Volume rows
arrive with the audio engine (spec 05).

Flying feel off? Every dial (speed, mouse sensitivity, zoom ends,
orbit framing, tints…) is mapped to a file:line in
**[TUNING.md](TUNING.md)** — one-line edits, warm rebuilds.

## Judging the generator (spec 02 tooling)

```sh
cargo run -p worldgen-cli --release -- generate --seed 0x1 --out out/
cargo run -p worldgen-cli --release -- batch --count 1000 --out out/batch/
cargo run -p worldgen-cli -- golden          # determinism check (CI runs it)
cargo bench -p cyclone_worldgen              # ~26 ms/world mean, ~53 ms p95 (release; stricter validate gates mean ~22% of seeds regenerate once)
```

`generate` writes a labelled top-down `map.png` (props, `H` helipads, `X`
crates — red = hidden from some heading, refugee dots, island names),
`world.ron` and `stats.txt`. `batch` writes `summary.csv` plus a
`contact-sheet.png` of the first 64 worlds.


## Workspace

| crate | role |
| --- | --- |
| `crates/cyclone` | binary — wires plugins, nothing else |
| `crates/cyclone_engine` | states, settings, input, camera rig, RNG |
| `crates/cyclone_render` | world mesh building (terrain/sea/props) |
| `crates/cyclone_ui` | theme, menu buttons, focus navigation |
| `crates/cyclone_game` | screens (spec 01), flight/cyclone (later specs) |
| `crates/cyclone_worldgen` | **Bevy-free** deterministic map generation |
| `tools/worldgen-cli` | headless map dumping/inspection |

## Quality gates (required before any commit — see `V2/README.md`)

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Determinism: same seed → same map, byte-identical, on every run;
`cargo test -p cyclone_worldgen` enforces it.
