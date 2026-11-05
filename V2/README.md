# Cyclone Rescue — Specs

These specs drive the build. Hand them to the coding agent in order. Each one is self-contained, but later specs assume earlier ones are done.

| # | Spec | Status | What it delivers |
|---|------|--------|------------------|
| 00 | [Engine architecture](00-engine-architecture.md) | Ready | Cargo workspace, Bevy foundation, our own engine layer as plugins, state machine, camera rig, input, settings |
| 01 | [Main menu](01-main-menu.md) | Ready | Title screen, menu navigation, settings, seed entry, world preview screen |
| 02 | [Map generation](02-map-generation.md) | Ready | Random archipelago every game, deterministic from a seed, validated, meshed in 3D |
| 03 | Flight and helicopter | Planned | — |
| 04 | Objectives, cyclone, planes | Planned | — |
| 05 | HUD, map screen, audio | Planned | — |
| 06 | Assets (Blender-scripted glTF) | Planned | — |

## What changed from the first build spec

The earlier "Cyclone Remake — Agent Build Spec" doc is still the source for **gameplay rules** (crates, refugees, fuel, cyclone zones, planes, scoring). These specs replace its **technical** parts:

- **Engine**: Bevy instead of macroquad, because the game is now 3D with rotatable, low-poly glTF models.
- **View**: a free-rotating isometric camera (4 × 90° steps) replaces the north/south flip.
- **Rendering**: no 320 × 240 render target or hand-rolled depth sort. Bevy's 3D renderer handles depth.
- **World**: random every game, not a fixed seed for round 1. The seed is still shown and can be typed in to replay a map.

## Ground rules for the agent

- Recreate mechanics, never the original's assets. Every model, texture and sound is new.
- Working title `Cyclone Rescue`, set in one constant.
- Each spec ends with a playable or runnable build. Don't start the next spec until the current one's acceptance checks pass.
- Where a spec is silent, choose, write the choice in `DECISIONS.md`, and keep going.
- `cargo fmt --check`, `cargo clippy --workspace -- -D warnings` and `cargo test --workspace` must pass before every commit.
