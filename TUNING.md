# Tuning map

Every dial we're holding an opinion on until your flight says otherwise.
Change a value → `cargo run -p cyclone` (warm incremental ≈ seconds).
Numbers marked *(setting)* are already user-adjustable in the Settings
overlay; the rest are one-line code edits.

All camera knobs live in
[`crates/cyclone_engine/src/camera_rig.rs`](crates/cyclone_engine/src/camera_rig.rs)
unless noted.

| # | Dial | Line | Now | Feel it as |
|---|------|------|-----|-----------|
| 1 | `FLY_BASE` | :37 | 38 u/s | cruise speed at fov 45; scales with fov (zoom in ⇒ slower) |
| 2 | `BOOST_X` | :38 | 4.5 | Shift-held multiplier |
| 3 | `LOOK_SENS` | :39 | 0.16 °/px | mouse feel; × `camera_rotation_speed` *(setting)* |
| 4 | `FOV_MIN` / `FOV_MAX` | :31-32 | 25 / 80 | wheel-zoom ends: 25 = sniper, 80 = wide |
| 5 | `ORBIT_PITCH` | :35 | −32° | menu/preview tilt (fly-mode look is free) |
| 6 | `PITCH_LIMIT` | :36 | 85° | how far up/down look can go before lock |
| 7 | orbit radius factor | :304 | max_dim × 1.45 | how far the menu/preview camera sits above the island |
| 8 | fly bounds | :324-325 | map + 60 u, y ∈ [2.5, 600] | soft walls of free flight (ceiling/floor) |
| 9 | InGame start pitch | `screens.rs` :1201 | −25° | first-frame angle when Fly! begins |
| 10 | preview Tab pitch | `screens.rs` | −89.5° | the flat top-down toggle angle |
| 11 | Base Island tint | `cyclone_render/src/terrain.rs` `base_island_tint` | ×1.10/1.02/0.82 RGB | the warm gold cast on the starting island |
| 12 | Q/E snap | fixed | 90° | heading snaps (drives crate occlusion per quadrant) |
| 13 | min-show progress | `screens.rs` | 300 ms | Generating screen floor before it can hand off |

## Feedback loop

Say what you felt ("cruise feels slow over water", "gold too strong",
"look drags") — each row maps to one commit; tests cover behaviour,
values are free. After three feel passes we promote the settled values
into defaults and prune this file.
