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
| 9 | InGame start pitch | `screens.rs` `rig.pitch = -18.0` | −18° | first-frame angle when Fly! begins (−25° pre-feel-pass; −18° frames 10 of 14 islands at spawn — see DECISIONS round 101) |
| 10 | preview Tab pitch | `screens.rs` `Some(-89.5)` | −89.5° | the flat top-down toggle angle |
| 11 | Base Island tint | `cyclone_render/src/terrain.rs` `base_island_tint` | ×1.10/1.02/0.82 RGB | the warm gold cast on the starting island |
| 12 | Q/E snap | fixed | 90° | heading snaps (drives crate occlusion per quadrant) |
| 13 | min-show progress | `screens.rs` | 300 ms | Generating screen floor before it can hand off |

## Debug profile is honest (measured, not assumed)

The tour says plain `cargo run` (dev profile). Measured on the
current code and config (160×160, 14 islands): `cargo bench` release
198 ms per 18-seed sample = ~11 ms/world (re-measured round 116,
after the bench pool became "generable seeds from 1..=20" = 18 — the
faster number vs the 20-seed era's 262 ms/13 ms partly *is* that
filter: the exhausted seeds' full 8-attempt burn no longer lands in
the sample); a single debug CLI
generate 13 ms — dev ≈ release, as designed: the profile compiles
all deps at opt-level 3 (root Cargo.toml), so `cargo run` gives
release-speed gameplay with fast incremental rebuilds. The `worldgen
benchmark` CI job logs the same bench every push (its runner is
~2× slower; the bench times 18 seeds per criterion sample — divide
by 18 before comparing). Worst reroll frame measured
in-app (dev, ignored `reroll_hitch` test): 3.8 ms — under a quarter
of the 16.6 ms 60 Hz budget. If a feel complaint ever smells like
stutter, the first question is "dev or release?" — and the answer
should not matter.

## What the generator rejects (measured tuning signal)

`worldgen-cli batch` prints `batch rejects: {…}` — every validator
verdict that cost a re-roll, live. Keys are normalised (numbers →
`#`, island names collapsed), so families group honestly:
`island out of fuel reach`, `fuel graph disconnected`, `no room for
the base hangar`, `pair closer than min_gap`, `only # of # island
sites fit`. Observed mix since the size spread (5..18) landed: the
fuel family leads, hangar-room next — both point at `island_radius`
spread or `fuel_range`.
A `fuel reach` rejection means the route solver couldn't link pads
under `fuel_range`. Note: in-game, machine-rolled seeds quietly
reroll past an Exhausted budget ("Rolling a luckier seed…"), so what
you can see is the seed you typed getting rejected honestly. If one dominates your runs,
the dial it points at (island_count, island_radius, min_gap,
fuel_range) is the one to move — the test
`rejections_match_attempts` guarantees attempts = 1 + reasons, so
the histogram is trustworthy, not folklore.

## Terracing: the number behind "the terrain is flat"

Every generated map reports `terracing` (`stats.txt`, batch
`summary.csv`, dev-HUD): the area-weighted, per-island share of land
NOT in the island's dominant height band — i.e. how stepped the
islands look from above. Measured eras on the same frozen seeds:
flat era 0.48–0.52, e-curve era 0.62–0.64 but with mesa-big-islands
(one level holding half an island — the slab screenshot), current
radial terraces + size-lift **0.53–0.70, median 0.65**, big islands
0.66–0.76. The `invariants_hold_over_many_seeds` test floors it at
0.50. If a world reads flat to your eye, `stats.txt` decides: low
terracing → the shape is the culprit (peak table + size-lift in
terrain.rs, capped by `max_level`); high terracing → the world just
looks smooth because steps are short — RAISE `level_height` (world
units per level) to exaggerate every step in 3D.

### Why the previous number lied

`relief_share` (share of land ≥ level 3) read a proud 0.76 on the
very world whose screenshot is a flat slab: height above sea and
*shape* are different properties. A metric picked without checking
it against the failure it is meant to detect will certify nonsense.
`terracing` was calibrated on both worlds before any floor was set.

## Feedback loop

Say what you felt ("cruise feels slow over water", "gold too strong",
"look drags") — each row maps to one commit; tests cover behaviour,
values are free. After three feel passes we promote the settled values
into defaults and prune this file.
