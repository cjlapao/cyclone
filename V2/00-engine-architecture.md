# 00 — Engine architecture

**Goal:** a Cargo workspace where Bevy supplies the low-level plumbing (window, renderer, ECS, assets, audio, input) and our own engine layer sits on top as a set of plugins. Game code only talks to our layer. That keeps the door open to swap or fork pieces of Bevy later without rewriting the game.

## Why Bevy, and how we "build our own" on it

Bevy is modular by design. Everything, including its own renderer and UI, is a plugin. So "our engine" is simply our own plugins plus a thin facade:

- We add plugins (`CycloneEnginePlugin`, `WorldGenPlugin`, …) that own our systems, resources and states.
- We can **disable** any Bevy default plugin and supply our own (`DefaultPlugins.build().disable::<X>()`).
- We extend rendering through custom `Material` impls with our own WGSL shaders (sea, toon/flat shading, outlines).
- We generate meshes procedurally (`Mesh` with our own vertex buffers) for terrain and sea.
- If we ever outgrow a Bevy crate, we vendor or fork just that crate behind our facade.

**Rule:** game crates never import `bevy::render` internals directly. They use types re-exported from `cyclone_engine`. Only `cyclone_engine` and `cyclone_render` touch Bevy rendering APIs.

## Versions

- Rust: latest stable, pinned in `rust-toolchain.toml`.
- Bevy: the latest released `0.x` at project start (0.19 at the time of writing; check crates.io). Pin it exactly (`bevy = "=0.19.x"`). Upgrades are their own commit with a note in `DECISIONS.md`.
- Every third-party Bevy plugin must have a release matching our Bevy version. If one doesn't, don't use it.

## Workspace layout

```
cyclone/
  Cargo.toml                 # [workspace], shared profile settings
  rust-toolchain.toml
  DECISIONS.md
  docs/specs/
  assets/
    models/                  # .glb files (spec 06)
    shaders/                 # .wgsl
    fonts/
    audio/
  crates/
    cyclone_engine/          # our engine layer: states, camera rig, input, settings, rng, facade
    cyclone_render/          # materials, shaders, terrain/sea mesh builders, lighting presets
    cyclone_worldgen/        # PURE Rust map generator. No Bevy dependency.
    cyclone_ui/              # menu widgets, theme, screen framework (on bevy_ui)
    cyclone_game/            # gameplay systems (helicopter, cyclone, objectives) — later specs
    cyclone/                 # the binary: wires plugins together, nothing else
  tools/
    worldgen-cli/            # generate a map from a seed, dump PNG previews + RON (spec 02)
```

Dependency direction (no cycles, no upward arrows):

```
cyclone (bin)
 ├── cyclone_game ──┐
 ├── cyclone_ui ────┤
 ├── cyclone_render ┼──> cyclone_engine ──> bevy
 └──────────────────┴──> cyclone_worldgen (pure, no bevy)
```

*(Round-253 note: the tree above is the target layout as specced; the
implemented layout differs exactly where the pilot chose — the specs
themselves live in `V2/`, not `docs/specs/`; `assets/models` +
`shaders` arrive with spec 06 as planned, and `docs/` today carries
generated samples (contact-sheet.png re-regenerated TODAY,
2026-11-08 — round-253 label aged out within hours; and classic bytes are
digest-pinned at 1f9ee0da in tests/continent_canary.rs, CI-green to the
tip, so any sheet's classic content is current BY DIGEST, not by age).)*

`cyclone_worldgen` stays Bevy-free so it compiles fast, tests fast, and runs in the CLI tool.

## Build profiles

In the workspace `Cargo.toml`:

```toml
[profile.dev]
opt-level = 1

[profile.dev.package."*"]
opt-level = 3        # optimise Bevy and deps even in dev; our code stays fast to compile

[profile.release]
lto = "thin"
codegen-units = 1
strip = true
```

Add a `dev` cargo feature on the binary that enables `bevy/dynamic_linking` for faster local iteration. Never ship with it on.

## App states

One top-level state enum in `cyclone_engine`, driven with Bevy `States`:

```rust
#[derive(States, Default, Clone, Eq, PartialEq, Hash, Debug)]
pub enum AppState {
    #[default]
    Boot,            // load settings, fonts, minimal assets
    MainMenu,        // spec 01
    WorldPreview,    // spec 01 + 02: generate, look, regenerate, start
    Generating,      // background worldgen with progress bar
    InGame,          // later specs
}
```

Gameplay sub-states (`Playing`, `Paused`, `MapView`, `Crashed`, `RoundComplete`, `GameOver`) come later as a `SubStates` under `InGame`.

Every screen spawns its entities with a `DespawnOnExit(AppState::X)` marker (or the current Bevy equivalent) so state changes clean up automatically. No manual despawn lists.

## Engine plugins (in `cyclone_engine`)

| Plugin | Owns |
|--------|------|
| `CoreStatePlugin` | `AppState`, transitions, despawn-on-exit |
| `SettingsPlugin` | `Settings` resource, load/save RON in the OS config dir (`directories` crate), apply on change |
| `InputPlugin` | Action map: `Action::{Up, Down, Left, Right, Confirm, Back, RotateCamLeft, RotateCamRight, Climb, Descend, Map, Pause}` bound to keys and gamepad. Game code reads actions, never raw keys |
| `CameraRigPlugin` | Isometric orthographic camera rig (below) |
| `RngPlugin` | `GameSeed` resource and seeded RNG streams (see spec 02) |
| `DiagnosticsPlugin` | FPS overlay and debug toggles behind the `dev` feature |

### Camera rig

- Orthographic projection, scaling mode fixed vertical, so the view size doesn't change with window aspect.
- Pitch about 35° down (true isometric is 35.264°). Yaw starts at 45°.
- `RotateCamLeft` / `RotateCamRight` (Q / E) turn yaw by 90° with a 0.3 s eased tween. Exactly four headings: 45°, 135°, 225°, 315°.
- Follows a target entity with critically damped smoothing. Clamped to world bounds.
- Zoom: three fixed levels on the mouse wheel. Default the middle one.
- Expose `CameraHeading` as a resource (0–3). Spec 02 uses it for crate occlusion checks.

*(Annotation 2026-11-08: the shipped camera drifted from this section
while the pilot was choosing worlds, not silently — spec 03's
"where the code stands" records it: free mouse-look with WASD+Space/
Shift/Ctrl fly (`camera_rig.rs`, ~600 lines), perspective, not the
orthographic iso rig specced above; Q/E still snap yaw and
`CameraHeading` keeps its four-quadrant occlusion role (re-pointed
witness, round 222). The zoom-levels line is dev-tool reality on the
free cam (wheel = continuous FOV zoom, eased toward a target; the
three-fixed-levels spec never shipped). Which camera survives gameplay is
spec 03's OPEN Q2 — until the pilot answers, this section is
prescription in review, not status.)*

### Input bindings (defaults)

| Action | Keyboard | Gamepad |
|--------|----------|---------|
| Move | Arrows / WASD | Left stick / D-pad |
| Confirm | Enter / Space | South (A) |
| Back | Esc | East (B) |
| Rotate camera | Q / E | LB / RB |
| Climb / Descend | Space / Left Shift (in game only) | RT / LT |
| Map | M | Select |
| Pause | P / Esc | Start |

Store bindings in `Settings` so remapping is a later UI job, not a code change.

## Rendering defaults (in `cyclone_render`)

- Look: flat-shaded low-poly, vertex colours from a shared palette, soft directional sun with shadows, mild ambient light, light fog toward the edges.
- One custom `SeaMaterial` (WGSL): gentle vertex wave, depth-tinted colour, foam where the sea meets land (from a shoreline distance texture baked by worldgen).
- MSAA 4×. Target 60 fps at 1080p on an M1 MacBook Air and a GTX 1060-class PC.

## Cross-platform and CI

GitHub Actions matrix: `ubuntu-latest`, `macos-latest`, `windows-latest`.

- Every push: fmt, clippy, test, `cargo build --release -p cyclone`.
- Linux runners install: `libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev`.
- Cache `~/.cargo` and `target/` with `Swatinem/rust-cache`.
- Tag `v*`: build and attach Linux x86_64, Windows x86_64, and a universal macOS binary (`lipo` of arm64 + x86_64), each zipped with the `assets/` folder.

## Acceptance checks

- [ ] `cargo run -p cyclone` opens a window on Linux, macOS and Windows, shows a placeholder Boot screen, then MainMenu. *(Boot→MainMenu hold is tested headless (`flow.rs`); the on-screen open across OSes awaits the player.)*
- [x] Removing any game crate from the binary still compiles the engine crates (proves the dependency direction). *(CI guard step: engine tree greps empty for game-side crates, with two must-fire positive controls — `990e899`.)*
- [x] `cyclone_worldgen` has no `bevy` in `cargo tree -p cyclone_worldgen`. *(Same guard step, every CI run on 3 OSes.)*
- [x] Camera rig rotates through all four headings and returns to the start after four presses. *(Witness re-pointed round 222: `heading_snaps_track_quadrants` (camera_rig.rs) asserts all four headings AND return-to-start after four presses; the game-crate occlusion tests exercise it through the real rig. Both run on all CI OSes.)*
- [x] Settings survive a restart. Deleting the settings file restores defaults without a crash. *(Disk round-trip + fallback tests in `cyclone_engine`.)*
- [x] CI is green on all three OSes. *(Proven on `24459f8` — the last failure was ron's platform-default CRLF in golden text, root-caused via the in-test fingerprint table, DECISIONS round 39. Fresh proof of current-era state: `3e3d5a6` green ×3 verified round 217 via the Actions pages; freshest proof round 235: `29b2697` (a BEHAVIOR change — strict-'1' dials) completed successfully ×3 OSes, closing a green ×6 that spans the whole dial-ownership era. Islands-v2 era proof: `ede9f17`+`81d2712` completed success on all three OSes, the island canary digest ASSERTED and AGREED on every runner — libm law intact. See the weekly probe.)*
