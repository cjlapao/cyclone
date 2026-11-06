# Tuning map

Every dial we're holding an opinion on until your flight says otherwise.
Change a value → `cargo run -p cyclone` (warm incremental ≈ seconds).
Numbers marked *(setting)* are already user-adjustable in the Settings
overlay; the rest are one-line code edits.

All camera knobs live in
[`crates/cyclone_engine/src/camera_rig.rs`](crates/cyclone_engine/src/camera_rig.rs)
unless noted. The Line column is a **search anchor, not a line number** —
`grep -n "FLY_BASE" crates/cyclone_engine/src/camera_rig.rs` — because raw
line numbers rotted twice already; identifiers don't.

## Controls (defaults; remap by hand-editing the settings file — no remap UI exists yet)

The overlay edits only Fullscreen / VSync / Window size / Camera speed.
Keys live in the `keys:` block of the settings file (edit it with the
game closed; a bad file falls back to defaults):
Linux `~/.config/rescue/settings.ron`, Windows
`%APPDATA%\cyclone\rescue\settings.ron`, macOS
`~/Library/Application Support/com.cyclone.rescue/settings.ron`
(paths of `directories::ProjectDirs` — verified by running it).
Binding names = field names below (`up`, `climb`, `boost`, …), each a
list of key names as in the table (e.g. `boost: ["ControlLeft"]`).

| Key | Does |
|-----|------|
| `W`/`↑` `S`/`↓` `A`/`←` `D`/`→` | fly forward/back/left/right |
| `Space` / `Shift` | climb / descend |
| `Ctrl` | boost (×4.5 — dial 2) |
| hold `RMB` + mouse | look (`LOOK_SENS` dial 3) |
| wheel | FOV zoom (25°–80°, dials 4–5) |
| `Q` / `E` | snap view 90° left/right |
| `Tab` | top-down (preview) |
| `R` / `C` | reroll / copy seed (preview) |
| `Esc` | menu/back | `P` pause `M` map |

*Correction note: earlier chat notes said "Shift boost" — wrong; Shift
descends, Ctrl boosts. The in-game HUD and this table match the code
(`settings.rs` `KeyBindings::default`).*

| # | Dial | Line | Now | Feel it as |
|---|------|------|-----|-----------|
| 1 | `FLY_BASE` | `const FLY_BASE` | 38 u/s | cruise speed at fov 45; scales with fov (zoom in ⇒ slower) |
| 2 | `BOOST_X` | `const BOOST_X` | 4.5 | Ctrl-held multiplier |
| 3 | `LOOK_SENS` | `const LOOK_SENS` | 0.16 °/px | mouse feel; × `camera_rotation_speed` *(setting)* |
| 4 | `FOV_MIN` / `FOV_MAX` | `const FOV_MIN` | 25 / 80 | wheel-zoom ends: 25 = sniper, 80 = wide |
| 5 | `ORBIT_PITCH` | `const ORBIT_PITCH` | −32° | menu/preview tilt (fly-mode look is free) |
| 6 | `PITCH_LIMIT` | `const PITCH_LIMIT` | 85° | how far up/down look can go before lock |
| 7 | orbit radius factor | `max_dim * 1.45` | max_dim × 1.45 | how far the menu/preview camera sits above the island |
| 8 | fly bounds | `let lo = Vec3::new` | map + 60 u, y ∈ [2.5, 600] | soft walls of free flight (ceiling/floor) |
| 9 | InGame start pitch | `screens.rs` `rig.pitch = -25.0` | −25° | first-frame angle when Fly! begins |
| 10 | preview Tab pitch | `screens.rs` `Some(-89.5)` | −89.5° | the flat top-down toggle angle |
| 11 | Base Island tint | `cyclone_render/src/terrain.rs` `base_island_tint` | ×1.10/1.02/0.82 RGB | the warm gold cast on the starting island |
| 12 | Q/E snap | fixed | 90° | heading snaps (drives crate occlusion per quadrant) |
| 13 | min-show progress | `screens.rs` | 300 ms | Generating screen floor before it can hand off |

## Debug profile is honest (measured, not assumed)

The tour says plain `cargo run` (dev profile). Dev builds here measure
(ignored flow test `pipeline_wall`, `-- --ignored --nocapture`):
boot+plugins ~10 ms, generate ~1.7 ms, scene build ~0.3 ms/frame —
within noise of release (10.1 / 1.5 / 0.36 ms). The profile already
compiles all dependencies at opt-level 3, so `cargo run` gives you
release-speed gameplay with fast incremental rebuilds. If a feel
complaint ever smells like stutter, we first ask "dev or release?"
and re-run this probe.

## Feedback loop

Say what you felt ("cruise feels slow over water", "gold too strong",
"look drags") — each row maps to one commit; tests cover behaviour,
values are free. After three feel passes we promote the settled values
into defaults and prune this file.
