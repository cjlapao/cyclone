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
| `W A S D` | pan the view (view-relative) |
| `Q` / `E` | rotate 90° (tweened) |
| wheel | zoom (island / squad / archipelago) |
| `Enter` | confirm / start flying |
| `R` | reroll a new world (preview) |
| `C` | copy seed to clipboard (preview) |
| `Esc` | back |

Seed formats accepted in “Play Seed…”: 16 hex digits or the 13-char dashed
form shown in-game. Worldgen knobs: `assets/worldgen/default.ron`;
keys/display/vsync/fullscreen: `~/.local/share/cyclone/settings.ron`
(applies live, see `DECISIONS.md`).

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
