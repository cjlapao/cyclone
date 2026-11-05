# DECISIONS

Choices made where the spec is silent, newest first (spec ground rule: pick the
option closest to the 1984 original's feel, record it, move on).

## M1 — Skeleton

- **macroquad 0.4.16 API drift.** The spec says 0.4.x; the resolved crate
  (0.4.16) renamed/changed several items (`Conf` nests `miniquad_conf`, `Rect`
  uses `x/y`, no `hex!` macro, no `was_any_key_pressed`, `RenderTarget` not
  `RenderTarget2D`). Code targets the actual resolved API.
- **`game.rs` and `font.rs` added to the spec layout.** `game.rs` (pure,
  macroquad-free) will own the simulation so the headless replay test can step
  it; `font.rs` is the hand-drawn pixel font used by the render layer. Both
  respect the rule that only render-side files touch graphics.
- **Font is a hand-built 5×7 bitmap atlas**, generated at startup from packed
  bits in `font.rs` — original glyphs, no TTFs, satisfies "draw every font
  yourself".
- **Palette lives in `render.rs`** as a 16-entry table (`color_u8!`), bright
  cyan sea / sand / grass / white buildings / black outlines per spec.
- **`get_keys_pressed()` for "press any key"** on the Title state, gated by a
  0.4 s delay so the key that unpaused/started doesn't double-fire.
- **Fixed-step loop**: physics runs in 1/60 s ticks via an accumulator; input
  edge triggers are read inside those ticks so replays are frame-rate
  independent (acceptance check #6).
- **Full-screen toggle** tracks its own bool and calls `set_fullscreen`, since
  0.4.16 has no `toggle_fullscreen`.
- **crate-level `#![allow(dead_code)]`** while milestones land; to be removed
  in the M7 polish pass so CI's `clippy -D warnings` stays meaningful.

## M2–M5 (world, flight, objective, hazards)

- Terrain smoothing added to `gen_world`: 9 neighbour-average passes + slope
  limiter (|Δh| ≤ 1 between touching land tiles). Without it, 3×3 flat
  landing sites essentially never existed on a noisy heightmap.
- Base helipad is claimed **before** buildings/trees so objects can't crowd
  it out; extra pads use the real `landing_ok` predicate, not an ad-hoc one.
- `landing_ok` object rule = heli footprint (centre tile) overlap only. A
  tree one tile away doesn't block the 3×3 pad; landing ON the tree crashes
  via `hits_object` / footprint overlap. (Spec says "on a tree".)
- `Heli::step` takes the `View` explicitly (a thread-local hack was removed —
  purity of the sim matters for the replay tests).
- Storm drift is applied as a *position* drift each tick (not accumulated
  velocity), so the heli stops cleanly when it leaves the force zone.
  Landed factor 0.5 can drag a parked heli into the sea → crash.
- Planes fly only +x/+y lanes over proven-sea lines (original had fixed
  lanes; seeded random sea lanes keep the spirit and stay deterministic).
- Scoring: cargo scores when DELIVERED to the base pad (1000/250), ground
  bonus +10/s accrues while landed/refuelling, countdown is per round
  (−20 s, floor 180 s), cyclone speed ×1.10^(round−1).
- macroquad 0.4.16 drift: `draw_ellipse(x,y,w,h,rotation,color)` (no segment
  count), `draw_line` is 6 scalars, no `Color::darken`, new clippy wants
  `u32::is_multiple_of`. All original art is drawn with these primitives.

## M6–M7 (HUD, map, sound, polish)

- HUD is drawn from the shell side (`hud.rs`/`map.rs`) reading `&Game`; the
  simulation never touches it. Wind compass shows the screen-space bearing to
  the cyclone eye so a flip rotates the arrow with the world (it reads the
  same way the player looks).
- Chart screen (`M`) shows *all* crates including hidden ones — the planning
  map is honest even when the side view hides a crate behind a hill.
- Crate delivery is scored on landing on the base pad, never mid-air.
- SFX: `game.step` only pushes into a `fx: Vec<Sfx>` queue (keeps the sim
  macroquad-free); the shell drains it each frame. The cyclone/planes
  warning sound is edge-triggered on `Warning` entry, matching the 2 s rule.
- Rotor "pitch" is three pre-baked loops (slow/med/fast chop, seamless
  0.62 s WAVs) cross-faded by stopping/starting on bucket change —
  macroquad's PlaySoundParams has no pitch field, and switching samples is
  both cheaper and truer to 8-bit machines.
- High scores persist to `~/.cyclone_rescue_highscores` (HOME/USERPROFILE/
  APPDATA fallback "."): lines `score|round|seed`, top 10, shown on title.
- lib/bin split: `src/lib.rs` re-exports all modules so `tests/` integration
  tests import `cyclone_rescue::*`; `main.rs` is now just window + loop +
  input mapping. `#![allow(dead_code)]` is gone; tree is warning-free.
- Audio is the default `audio` cargo feature. Without libasound2-dev the
  linker fails, so `--no-default-features` compiles the same game with
  macroquad's silent dummy audio — CI/release builds keep sound.
- Acceptance `tests/physics.rs` equalises *physics step count* (not wall
  time) across 30/60/144 fps before comparing bit patterns.
- Screen shake (crash only) offsets the projected scene centre by a decaying
  ~2 px sine; the tile loop already draws a tile beyond the viewport, so no
  black edges appear.

## Linking ALSA without sudo (Linux dev boxes)

- quad-snd links `-lasound`, which needs the dev symlink `libasound.so`;
  Ubuntu's runtime package ships only `libasound.so.2`. `make setup`
  symlinks the runtime lib into `vendor/alsa/` (git-ignored) and
  `.cargo/config.toml` adds `-L native=vendor/alsa` for the linux target.
  Machines with the real dev package ignore the missing dir; macOS/Windows
  configs untouched. Full sound, no root.

## V2 — Engine & renderer (specs 00–02)

### Bevy pinned exactly to 0.20.1
Spec 00 requires an exact pin; `=`0.20.1 with `default-features = false` and
an explicit feature list. `x11` is on (no wayland on this dev box); audio,
gamepad-backends and wayland are off — Gilrs would pull C deps we don't need
v1 has no sound.

### `First` is a struct, not an enum variant (Bevy 0.20)
`Systems::First` no longer exists; schedules take the `First` marker struct
directly (`add_systems(First, poll_actions)`). Input polling runs there so
`Actions` is fresh before every Update system.

### KeyboardInput/MouseWheel are Messages in 0.20
`Event` → `Message`; readers are `MessageReader<T>`. The seed-text field
consumes `MessageReader<KeyboardInput>` — no `bevy_ui` TextInput, whose
pointer-event API is too new to rely on yet.

### Custom MenuButton + Interaction (deprecated) instead of bevy_ui_widgets
The 0.20 `widget::Button` needs `Pointer*` messages the focus system can't
share; classic `Button`/`Interaction` still works and is only deprecated.
`#[expect(deprecated)]` is confined to cyclone_ui.

### Text is component-tuples now (0.20 `Text` widget)
`TextBundle` is gone. Theme helpers return `(Text, TextFont, TextColor)`;
font size is `FontSize::Px`, font handle goes through `FontSource`.

### One Camera3d renders world + UI
Bevy 0.20 renders UI for 3D cameras; a second `Camera2d` doubles the render
graph for nothing. If UI turns out invisible at runtime, re-add Camera2d.

### Orthographic rig in engine, target-relative
`CameraRig` (yaw tween 0.3 s, 3 zoom stops 56/104/176 tiles, fixed 35.264°
pitch) lives in cyclone_engine with `RigTarget`/`RigFollow`/`WorldBounds`
so menus, preview and gameplay all share one camera model (spec 00).

### Level height comes from WorldConfig, not a constant
`level_height` defaults to 0.5 world units (≤ 3.5-unit peaks). The renderer
reads `Res<WorldConfig>` and scales terrain by it; props use their own
`PROP_UNIT = 1.15` so they stay readable at any level height.

### Terrain: chunked quads + vertex colors, no textures v1
16×16-tile meshes, `RENDER_WORLD` usage, baked hash-tint vertex colors;
one white StandardMaterial per chunk group. Shore foam texture is authored
(R8 image) but unused until a shader exists. No shadows, no fog, no MSAA
(spec 00 perf budget).

### Mesh type is `Mesh3d` (+ `MeshMaterial3d`)
0.20 renamed the world-mesh component; `MeshMesh3d` doesn't exist. Caught
against the 0.20.1 registry sources, not from memory.

### Generation off-thread via Arc<Mutex<Option<..>>> + atomics
Worldgen is sync; a detached AsyncComputeTaskPool task writes progress into
Arc<AtomicU32> (permille) + Arc<Mutex<&'static str>> (stage) and the result
into a slot. `Generating` polls, floors at 300 ms so it never flashes
(spec 01).

### Settings path via directories::ProjectDirs, RON, forgiving load
`com.cyclone.rescue` → `settings.ron`; any parse error falls back to
defaults (spec 00: never hard-fail on user files).

### Clipboard through bevy_clipboard (C-copy on preview)
`ResMut<Clipboard>` + `set_text`; headless/clipboard-less setups just log
a warning instead of failing the key press.

### Fonts: Liberation Sans bundled
`assets/fonts/LiberationSans-{Regular,Bold}.ttf` (Liberation fonts are
SIL OFL / LPPL; metric-compatible with Arial). Avoids Bevy's built-in
default-font pipeline dependency.

### cargo not on PATH
Dev box has no rustup shims dir; toolchain is
`~/.rustup/toolchains/stable-x86_64-unknown-linux-gnu/bin`. Makefile/CI
must prepend it or rely on a login shell.

## Round 3–4 — startup panics, Bevy 0.20 landings, generator tooling

### Bevy 0.20 API landings (from the headless flow test)
- `add_asset` is `init_asset` in 0.20; tests that need assets call it
  explicitly with `RenderAssetUsages::MAIN_WORLD`.
- Message types must be registered (`app.add_message::<T>()`) **before** any
  system takes a `MessageReader`/`MessageWriter` — Bevy 0.20 errors at run
  time otherwise, not compile time. `KeyboardInput`/`MouseWheel` come from
  `bevy::input::InputPlugin`; engine `ActionsPlugin` now adds it when absent
  (our own plugin used to shadow it and skipped registration).
- Injecting a world message from tests: `world.write_message(msg)`.
- `MinimalPlugins` contains no StatesPlugin — `CycloneEnginePlugin` adds
  `bevy::state::app::StatesPlugin` itself so headless runs get state
  transitions.
- Without the `gilrs` feature there is no `ButtonInput<GamepadButton>`
  resource; the actions system reads `Option<Res<…>>`.
- Plugins are **not** deduped: adding `CycloneEnginePlugin` from two places
  panics. `CycloneGamePlugin` owns it; `crates/cyclone/src/main.rs` doesn't.

### Difficulty bands recalibrated (golden hashes refreshed)
The spec-02 classifier thresholds predated the final helipad placement:
every world scored Hard, so the label carried zero signal. Bands are now a
weighted score `0.55·hop/fuel + 0.45·route/crate/fuel` with cut points taken
from a 1000-seed sweep (p25/p65 → Easy/Normal/Hard ≈ 25/40/35 %).
`crates/cyclone_worldgen/tests/golden.ron` was regenerated with
`worldgen-cli golden --update` in the same commit, as the spec requires.

### worldgen-cli ships spec 02 C1–C4
`generate` (labelled PNG + world.ron + stats.txt), `batch` (summary.csv +
64-map contact sheet, non-zero exit on any failure), `golden` (+ cargo-test
`golden.rs`), criterion bench (~14 ms/world release; CI logs, never gates).
`pipeline::validate` (A5.10 invariants) is public so the batch checker and
game-side tests share one definition.

### Island visuals are uniform — known improvement
Contact sheets show correct placement but every island reads as a green
disc with a sand ring; archetypes don't read visually yet. First thing to
iterate on once navigation is confirmed (affects 3D too, not just PNGs).
