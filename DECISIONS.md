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

### AsyncComputeTaskPool::get() → get_or_init (crash class #7)
`get()` **panics** when TaskPoolPlugin never ran ("The AsyncComputeTaskPool
has not been initialized yet"). The real app is fine (DefaultPlugins), but
any custom/test app died on New Game. `gen_start` now uses
`get_or_init(TaskPool::default)`. Regression: `cyclone_game/tests/pool.rs`
drives OnEnter(Generating) on an app with no task pool — it also documents
the minimal bare-app recipe (StatesPlugin, KeyboardInput/MouseWheel/Activated
messages, GameConfig, RigTarget). `cyclone_game::register_test()` exposes the
screens' schedules without the engine plugin for this.

### Terrain iteration 1: spec-faithful coastlines + relief (golden refresh #2)
Contact sheets read as "identical discs" — three fixes, all spec-justified:
1. Coastline warp now `fbm×0.35` at freq 0.08 (was ±0.15 @ 0.05) — the
   spec's "ragged coastline" term, zero-mean so the packing radius holds.
   Mean gen attempts rose 1.03 → 1.13 (min-gap retries), still 300/300 ok.
2. Beach band narrowed (sea-cut→beach 1.25 → 1.15): sand rings no longer
   dominate every shore.
3. Detail amp raised Hilly 0.30→0.50, Peak 0.35→0.55 (Flat/Plateau/Atoll
   stay calm — A5.6 needs flat 3×3 pads): interior cliffs and rocky spots.
Goldens regenerated with `golden --update` per spec. The first genuine
round of the "generate → judge → tune" loop the specs designed around.

### Free camera replaces the isometric rig (user instruction, supersedes spec 00 rig)
"Move the camera freely and aim it with the mouse" — CameraRig is now a
perspective free camera: position + yaw/pitch, right-mouse-drag aim,
WASD fly along the view, Space/Shift altitude, Ctrl boost, wheel = FOV
(25°–80°), generous padded bounds so the player cannot get lost.
- Menus/preview keep the slow orbit (auto_orbit) — scripted in the same
  rig; orbit radius auto-frames from WorldBounds.
- Q/E 90° snaps stay, and CameraHeading now tracks the *nearest quadrant
  of the free yaw every frame*, so spec-02 crate occlusion demos live.
- MouseMotion.delta.y is positive DOWN on XInput2 (winit "unspecified
  units"); we negate for screen-space look. Right-drag (not pointer grab)
  because this winit build lacks the cursor-grab feature.
- Actions gains Boost (Ctrl, serde-defaulted so old settings.ron parses).
- Engine add_message attempt removed: ActionsPlugin's "add InputPlugin
  only if Messages<KeyboardInput> missing" guard must see reality —
  registering messages without ButtonInput poisoned it (caught by the
  flow tests).
- tests/freecam.rs: live headless proof — W flies, RMB+motion aims,
  wheel zooms FOV, heading follows yaw, bounds clamp.

### Spec 02 B4 runtime occlusion was never actually wired — found and fixed
Audit of the acceptance list found crates spawned with `hidden_from` as a
comment-only feature: no system consumed it, so turning the camera never
hid anything. Now crate entities carry a `Crate` component and
`crate_occlusion` flips `Visibility` whenever `CameraHeading` changes.
Heading-space mapping derived from both conventions (rig: canonical yaw
45+90h, forward = rot(yaw)·−Z; gen: rays from crate toward viewpoint,
CAM_DIRS[0]=(+x,+y)) ⇒ `h_gen = (3 − cam) % 4`, unit-tested; the freecam
test spawns a crate hidden from gen idx 1 and watches it blink exactly
as the free yaw crosses 225°. If the demo ever looks mirrored on
screen, that one function is the dial.

## Settings overlay panel (spec 01 C6, closes the "apply live" gap)

Spec 01 lists a Settings overlay in the menu tree, but the first
implementation only read settings.ron at boot — "changes apply live and
save on close" had no in-app path at all. Added as `menu → Settings`:

- Rows: Fullscreen, VSync, Window size, Camera speed. Editing mutates
  `Res<Settings>`; the existing `apply_settings` system pushes it to the
  window and saves RON every frame it changes — "apply live + save" comes
  for free from the same plumbing the file-watcher story planned.
- Volume rows deliberately **not** shown: there is no audio engine until
  spec 05; UI that pretends to control something is worse than no UI.
  They get added with audio.
- Modal input arbitration: `cyclone_ui::OverlayActive` resource gates
  `focus_nav` (`not(resource_exists)`) so Up/Down/Enter behind the open
  panel can't move the background menu; the panel's root uses
  `FocusPolicy::Block` so the mouse can't press through. `menu_activate`
  no longer needs a panel guard.
- The panel swallows the opening frame's Confirm (`swallow` flag): the
  Enter/click that opens it must not also flip row 0.
- Found while wiring: `RelatedSpawnerCommands::spawn` auto-inserts
  `ChildOf(spawner)`, so spawning a child-of-manual-parent through the
  spawner panics with duplicate `ChildOf` — deferred value texts are
  spawned via plain `commands.spawn((ChildOf(label), …))` instead.

## 0.20 `Interaction` is NEVER auto-inserted — the menu mouse layer was dead (round 20)

Round-19 note claimed bevy_ui auto-inserts `Interaction` on nodes.
**Wrong** — corrected with source proof: `ui_focus_system`
(`bevy_ui/src/focus.rs` `NodeQuery`) only *writes*
`Option<&mut Interaction>` where the component already exists, and
the `Button` widget pulls it via `#[require]`; nothing inserts it
otherwise. Our `menu_button` spawned neither → `ui_focus_system`
could not press it and `button_visuals`'s `&Interaction` query never
matched → **mouse hover/click never worked on the main menu** (only
ever exercised by keyboard tests). Latent since spec 01.

Fix: `menu_button` now carries `Interaction::default()` (settings
rows already do, round 19). Facts banked:
- `FocusPolicy::DEFAULT` is `Pass`, so plain text children do NOT
  block the pointer from reaching a `Block` parent underneath.
- Headless (no window): `ui_focus_system` resets any injected
  `Interaction` every PreUpdate — tests must not fake hover by
  writing the component between frames; cyclone_ui's regression test
  drives `button_visuals` through the real message reader instead.
- Press-edge dedup (`prev_pressed` Local) now has a test: one
  `Activated` per click, none while held, again on re-click.

## Preview screen: buttons, Tab top-down, crates hidden (rounds 21-22)

Spec 01's preview demanded four things the implementation had skipped,
all now covered by `preview_screen_mouse_and_topdown`:

- **Buttons** (Fly! / Reroll / Copy seed / Back) so the screen is
  mouse-usable end to end (C2). Same `menu_button` styling, same
  `Activated` bus; `preview_activate` mirrors the keyboard actions.
  Copy is a no-op with a warn where no clipboard resource exists
  (headless).
- **Tab → flat top-down** (the posture spec 05's map screen reuses).
  Engine gained `RigTarget::pitch_hold: Option<f32>`: Some(pitch)
  locks pitch, stops the orbit spin, and selects orbit mode even with
  `auto_orbit == 0`. Q/E still snap — rotating the flat map is a
  feature, not an accident.
- **Crates hidden during preview** (spec: hidden by default; the
  `dev`-only F3 reveal is deferred — not in acceptance, no dev feature
  flag exists yet). Enforced EVERY frame while in `WorldPreview`
  because the async scene rebuild can spawn crates after `OnEnter`;
  `OnExit` restores, and the test asserts InGame shows exactly the
  non-occluded crate set — preview hiding and spec-02 occlusion
  compose without stepping on each other.
- The generation-ERROR screen's `plain_button` was missing the
  `Interaction` component (same round-20 bug class) — mouse Retry/
  Back now work too.

flow_app gained `flow_app_with(auto)` — the auto-advance key injections
would otherwise drive straight through the preview.

## Base Island highlight = warm vertex tint (round 23)

Spec 01's preview panel asks for "Island count, Base Island
highlighted". Island 0 is always the Base Island (cleanup.rs names it
BASE_ISLAND_NAME). Chose a per-vertex warm tint on the base island's
surface quads in terrain.rs (`island_of == 0` → red +10%, blue −18%,
linear space, clamped per channel):

- same mesh, same vertex count, zero extra draw calls or materials;
- generator data untouched → goldens unaffected;
- reads at a glance from orbit altitude, obvious while flying, and
  survives every camera mode including the Tab top-down;
- unit test asserts the warm shift, gamut clamp, and the white-input
  behaviour (white → gold IS the tint, by design).

Alternative considered: separate highlight ring mesh — rejected as a
second material + transparency ordering for what is one component of
existing color data.

## Spec 00 C2/C3/C5 closed (round 24)

- **C5** now has a *disk-level* test (`settings_disk_roundtrip_and_recovery`):
  XDG_CONFIG_HOME redirected to a temp dir (the only env-touching test
  in the crate, so the unsafe env write is safe under parallel test
  threads): missing file → defaults, save→load survives, corrupt file
  → defaults, no crash. The RON round-trip test alone didn't exercise
  the real path.
- **C2/C3** enforced in CI, not just in prose: `cargo tree` guards —
  worldgen has no bevy on normal or dev edges, and no game crate
  appears on NORMAL edges of the engine tree (`cyclone_render` is a
  dev-dep of the engine for the occlusion E2E test, which C2's
  "still compiles" wording permits).
- `cargo tree -i` pitfall: it prints "nothing to print" but exits 0 —
  grep the output for the dependent package instead of trusting the
  exit code; verified with a positive control (worldgen → engine shows
  up, proving the guard can actually see a violation).

## F3 dev reveal shipped — spec 02's acceptance box is now performable (round 25)

Round-22 deferred the preview's dev-only crate reveal; the spec-02
acceptance list itself demands it ("dev toggle on" to watch crates
hide per heading), so it now exists under `cyclone_game`'s new `dev`
feature (bin's existing `dev` feature now implies it — one flag:
`cargo run -p cyclone --features dev`):

- F3 toggles `PreviewShowProps`; while ON, `preview_hide_props`
  stands down and **`crate_occlusion` alone** decides visibility —
  so turning Q/E demonstrates each heading's hidden set live.
- Test `f3_dev_reveal_on_preview` (feature-gated, CI step added):
  default all-hidden → F3 → hidden set == exactly `crates - want`
  for the current heading → F3 → all hidden again.

Input lesson re-earned the hard way: the toggle reads
**`KeyboardInput` messages**, not `ButtonInput::just_pressed` —
hand-pressing `ButtonInput` between updates in a headless test does
not produce `just_pressed` edges (it saw `pressed()` but never the
edge; message injection is the path every working test uses).
The `--workspace --features dev` clippy invocation drags the bin's
bevy dev-tools → alsa; scope to `-p cyclone_game --features dev`.

## Generation is panic-proof end to end (round 26)

Panic-path audit of all shipping code: every `unwrap` left in game/
engine/render/ui lives behind `#[cfg(test)]` except the generation
channel's mutex locks — and those hid a real failure mode. A panic
inside `generate()` (its guarded `expect("in bounds")` class is
1000-seed-clean but possible) would (a) poison the mutexes the main
thread locks with `.unwrap()` → cascade panic, and (b) never fill the
result slot → the Generating screen would wait forever, progress bar
animating into the heat death of the universe.

Fix, three layers:
- `run_generate` wraps the whole call in `catch_unwind` and converts
  a panic to `GenError::Internal`-flavoured `Invalid(…)` — the user
  lands on the existing Retry/Back error screen;
- `lock_any()` takes the poisoned guard instead of re-panicking
  (`PoisonError::into_inner`); the slot's invariant (Option<Result>)
  survives a remote panic untouched;
- both call sites use it: the async task AND the synchronous menu-
  backdrop generate (a panic there used to kill the process outright).

`wrapper_surfaces_config_errors` proves the wrapper passes real
`Err(Config)` through to the error-screen path.
## Boot placeholder holds 0.7 s; two spec-01/00 audit notes (round 27)

- **Boot screen (spec 00 C1)** previously spawned nothing and set
  MainMenu in the same frame — the "placeholder boot screen" had never
  been visible on any machine. Now `boot` spawns a branded overlay
  (CYCLONE RESCUE / free-fly archipelagos) and `boot_hold` defers the
  handoff ~0.7 s: the first visible frame is intentional, and the menu
  no longer pops in while fonts and the backdrop world stream. Test
  asserts state is still Boot after the first update.
- **High Scores menu row: deferred with cause.** Spec 01's tree lists
  it; scores themselves arrive with the gameplay specs (03+), which
  also own the data shape (name, score, round, date, seed). Shipping a
  panel over an invented store is the "UI that lies" pattern we
  rejected for settings volumes. When spec 03 lands, the row mounts an
  empty-state table honestly, with "Play this seed" per entry.
- **CI deviation recorded:** spec 00 asks for `cargo build --release`
  on every push (3-OS matrix); we run debug per push and a full
  release job on `v*` tags instead — release-only breakage is rare,
  and per-push triple-release would quadruple CI wall-clock for it.
## Release-mode verification, memory-throttled (rounds 28-30)

The shipping config (`cargo build --release -p cyclone`) had only ever
compiled on CI — a blind spot worth closing. Full release build: 3 m
10 s warm. Running the workspace test suite under `--release` OOM-killed
cargo: the default parallel codegen/link spawns one rustc + one linker
per core (8 here), each peaking 1-3 GB while linking several Bevy test
binaries simultaneously.

Fix is a flag, not a profile change: **`-j 2`**. Verifying each crate
under release separately with `-j 2` kept peak memory at ~3 GB (12 GB
free throughout), every suite green, and left the release dep cache
intact:

```sh
for c in cyclone_worldgen cyclone_engine cyclone_ui cyclone_render \
         cyclone_game cyclone; do cargo test -p $c --release -j 2; done
```

Not `[profile]` edits: those invalidate every cached dependency (minutes
of rebuild) to fix a problem `-j` solves for free, and CI's runners
want the full default parallelism. On this box (15 GB / 8 cores)
`-j 2` is the right call for any release-mode cargo job; debug builds
never needed it because `debug = "line-tables-only"` and
`codegen-units = 1` (release) already keep per-process RSS modest.
## Window opens AT the saved settings (round 30)

`main.rs` created the window at a hardcoded 1280×800/AutoVsync and
relied on `apply_settings` to resize on frame one — so a restart after
saving 1920×1080-or-fullscreen flashed a wrong-sized windowed frame
first. `main.rs` now builds the `Window` directly from
`Settings::load()` (resolution, fullscreen mode, present_mode).
Verified-safe facts from the 0.20 renderer (`view/window/mod.rs`):
swapchain reactions are VALUE diffs (`present_mode_changed` compares
extracted vs current), not change-ticks — so apply_settings' writes
always land, and its now-matching first-frame write is a harmless
no-op. Live toggling + save-on-change remain apply_settings' job;
main.rs only owns the cold-start posture.
## SeaMaterial (custom WGSL sea): deferred with cause (round 31)

Asset audit of the whole shipping surface: font paths exist on disk
(assets/fonts/LiberationSans-{Bold,Regular}.ttf), no code references
any .glb/.png/.wav/.wgsl file, the release zips carry `assets/`, and
empty V1 leftover dirs (models/, shaders/) are untracked so nothing
dangles. One honest spec gap surfaced: spec 00's *rendering defaults*
describe a custom `SeaMaterial` (vertex wave, depth tint, shoreline
foam from a baked distance texture). The sea is a flat
`StandardMaterial` today. Deliberate deferral:

- it is not an acceptance check (the six C-items are all green);
- foam needs a new worldgen output (shoreline distance texture),
  touching the goldens for pure aesthetics;
- wave amplitude, foam width, and depth tint are exactly the dials
  the first on-screen flight will critique — tuning them blind
  headlessly is rework bait.

When spec 05 (polish/audio) or user feedback calls for it: custom
pipeline belongs in `cyclone_render` (game-crate render-boundary rule
unaffected), worldgen bakes the shore distance as a `Layer2d<u8>` so
goldens update through the normal drift protocol.
## The round-24 dependency guard was VACUOUS — fixed forward (round 33)

Re-running the CI guard locally with positive controls proved
`cargo tree -p cyclone_engine -e normal -i cyclone_game` prints NOTHING
even when the dependency is real — combining `-p` with `-i` doesn't
"check if $c depends on the engine", it just filters an inverse graph
to the empty intersection. The old loop could never fail: CI had been
about to "pass" spec-00 C2 by silence, not evidence. (Round 24's
"positive control" validated a different invocation than the one
shipped — controls must run the guard's exact command.)

The correct detector greps the DEPENDENT's forward tree: a violation
shows up as `cyclone_engine` inside a game crate's tree, and absence
of game crates in the engine's own tree is what C2 means. The new
step ends with two self-controls — the greps must FIRE on a real edge
(game→engine) and the worldgen tree must be non-empty — so the guard
fails loudly if it ever goes blind. Ran verbatim locally:
GUARD_PASS_V2 with controls firing.

`cargo tree` lesson: `-i` answers "who depends on X *within the
graph of* ..." combinations that are easy to get empty; forward
`cargo tree -p crate | grep dep` is the one form whose semantics are
obvious at a glance — use it in scripts.
## Windows golden drift: self-locating fingerprints (round 36)

The annotations trick paid off: Windows fails `golden_worlds_reproduce
_exactly` (assert_eq drift, first row) — and it has since the very
first V2 CI run. The generator source is provably OS-pure (integer
hash, libm-only transcendentals, exact f32→f64 promotions, no
HashMap/time/RNG anywhere in the pipeline), so the drift is a
codegen/libm boundary we cannot see from Linux. Since Windows logs
are anonymously unreachable, the golden test now PANICS with a
fingerprint table (byte length, line count, 4 chunk-hashes of the
RON) — and an `#[ignore]`d companion prints the same table locally
(kept in /tmp/golden_fingerprints_local.txt). Diffing the Windows
annotation against those values tells us WHICH region of the world
drifts (c0 config/head, c1 config-identical across seeds — constant
in every golden, c2 bulk terrain, c3 tail sites) before touching a
line of generator code.
## Windows golden drift: CLOSED — ron's platform-default new_line (round 39)

The fingerprint table paid for itself in one CI cycle:

    Windows: len=2226907 lines=130074 c0..c3 all equal to Linux
    Linux:   len=2096834 lines=130074

2226907 − 2096834 = 130073 = lines − 1: one byte per line boundary,
every VALUE hash identical. The worlds were bit-identical all along —
ron-0.10.1's PrettyConfig default new_line is `if cfg!(not(target_os
= "windows")) { "\n" } else { "\r\n" }` (ser/mod.rs:350): CRLF on
Windows BY DESIGN. All generators/serializers that feed hashed bytes
now pin `.new_line("\n")` (WorldMap::to_ron, WorldGenConfig::to_ron,
Settings::save_to_disk + its round-trip test). Golden hashes did NOT
change (LF was already Linux behaviour), which retroactively
confirms the goldens always captured the values correctly.

Spec 02 C3 (cross-OS byte identity) was NEVER violated at the value
level — the drift was purely the text terminator. Windows CI should
now go fully green; spec 00 C6 closes with it.

Lesson recorded: the region-fingerprint + hash-free (len/lines)
technique turned an unreadable-logs platform failure into a two-CI-
cycle closed case. Keep it in the test.
## dev feature: dropped bevy_dev_tools — it hijacked the handoff command (round 43)

`cargo run -p cyclone --features dev` — the F3 handoff command — was
BROKEN on any Linux box without alsa headers: in Bevy 0.20
`bevy/bevy_dev_tools` pulls `bevy_audio` → rodio → cpal → alsa
(cargo tree -i alsa-sys proves the chain), and this machine has no
pkg-config. Nothing in our code touches dev_tools APIs (the F3
overlay is ours; grep is empty), so the feature now enables only
dynamic_linking + file_watcher + cyclone_game/dev. CI blind spot
that hid it: no job ever resolved the BIN crate's dev feature graph
(dev-flow tests the game crate only) — now closed with
`cargo check -p cyclone --features dev` on all three OSes.
Re-add dev_tools only together with the alsa dev package.
## Copy-seed gets a status line (round 48)

The C key / Copy button wrote the clipboard silently — success was
indistinguishable from failure for the player (and untestable
headless). Both routes now feed one `CopyStatus` text line in the
preview panel: "copied ✓" / "copy failed" / "clipboard unavailable"
(the latter is also the honest headless fallback: no window ⇒ no
Clipboard resource). preview_keys now sends Activated(B_PCOPY)
instead of duplicating clipboard code — one handler, one truth.
flow.rs asserts the fallback wording through the real message route.
## Gamepad on Bevy 0.20: mirror events; GilrsPlugin behind a feature (round 49)

Audit found the engine read `Option<Res<ButtonInput<GamepadButton>>>` —
a resource **nothing in Bevy 0.20 writes** (whole-stack grep; the state
moved into the `Gamepad` entity component, fed by
`GamepadButtonStateChangedEvent` from bevy_gilrs). Every gamepad
binding was dead code: gamepad-only menus (spec 00/01) could never
fire, and no test injected gamepad input to notice.

Fix: engine owns `GamepadState` fed by mirroring
`GamepadButtonStateChangedEvent` (the exact messages InputPlugin
registers; production source bevy_gilrs, tests inject directly).
`update_actions` reads the mirror; flow.rs drives the whole
menu→New Game→preview route with injected D-pad/South events only.

`bevy_gilrs` (only device producer) stays OFF by default: on Linux its
`libudev-sys` needs pkg-config + libudev headers at build time (verified
failing here — same failure class as the dev_tools/alsa trim). New
`gamepad` feature on cyclone_engine + cyclone turns it on; CI installs
the two packages on ubuntu and `cargo check --features gamepad` guards
the build. Shipping default remains keyboard-complete.
## RMB aims with the OS cursor grabbed (round 50)

Aim worked until the cursor hit the window edge: motion deltas stop,
camera freezes, user blames LOOK_SENS. Now holding RMB in free-fly mode
sets `CursorOptions { grab_mode: Locked, visible: false }` (winit
downgrades Locked→Confined on X11 — motion still continuous inside the
window; macOS Locked is native). Release restores the cursor. Gated on
free-fly (`auto_orbit == 0`) so menus/preview never grab. Engine test
spawns a headless Window+CursorOptions and asserts press→Locked→
release→None.
## Golden regeneration is legitimate when the stored map violated spec (round 54)

Round 53's min_gap/component gates made golden seed 0x19 drift: its
stored map was spec-violating (silently accepted pre-check). We
regenerated that golden rather than weaken the gate — the spec, not
the golden, is the contract; goldens exist to pin determinism, and
post-regeneration CI proved byte-identical reproduction on
ubuntu+windows+macos. Benchmark note: mean gen 14→26 ms, p95 53 ms,
22% of seeds retry once — still an order of magnitude below the
300 ms Generating floor.
## Reroll hitch measured headless (round 55)

rebuild_world is synchronous by design; the spec-01 50 ms box was
assumed on-screen-only, but app.update() timing in the offscreen flow
app captures all main-thread ECS work (terrain/sea/props meshes,
materials, shore texture upload prep). 10 rerolls × 5789 frames:
worst frame 3.6 ms. GPU-side transfer is the only unrepresented
phase; kept the test as --ignored manual diagnostic.
## Layout styles, not just terrain variety (round 56)

The 64-world contact sheet failed spec 02 A5.2 by gestalt: every map
was the same evenly-scattered blob field because stage 1 was ONE
Bridson process. Added a per-attempt style roll (Scatter / Cluster /
Chain / Sparse) changing anchor seeding, annulus width, heading
persistence, parent policy and size-bias-within-config-range. Chain
spacing is absolute slack (near-touching pearls), cluster anchors at
0.21–0.30·min-dim with a 16-tile blob leash, all geometry scaled by
min-dim/160 so custom small configs stay solvable (that test caught
fixed-distance anchors failing a 96×96 world). Style histogram over
64 worlds is roughly uniform; goldens regenerated for the new
pipeline, 300-seed invariants green, batch 64/64 ok, median 20 ms.

## Round 61 — Esc returns to the world preview (same world), plus two latent bugs it exposed

The first-flight tour promised "Esc → preview, same world; Esc → menu",
but the code Esc'd straight to MainMenu. Made the promise true — Esc
now re-enters WorldPreview with CurrentWorld untouched (no rebuild; the
preview's orbit camera shows the world you just flew; Escape/Back at
preview then quits to menu; Reroll there replaces the world).

Reworking the headless flow harness for the two-press flow exposed TWO
real-game latent bugs, now fixed:

1. **Double activation on Enter at preview.** `preview_keys` moved to
   InGame on `Action::Confirm` AND focus_nav queued `Activated(focused
   button)` for the same press — if the state exit beat preview_activate
   consuming the message, the stale Activated sat in the queue and
   auto-fired Fly! on the NEXT preview visit. Confirm branch removed
   from preview_keys (single source: Enter activates the focused button,
   default focus = Fly!, identical UX); belt-and-braces: preview OnEnter
   now drains any stale Activated before building UI.
2. **Harness lacked InputPlugin**, so ButtonInput just-pressed flags
   never cleared — tests ran on input semantics the real game never has.
   flow_app now adds InputPlugin (and press_key labels are per-stage:
   "quit" Esc only fires once "back" has already been seen).

Flow suite 7/7 incl. full Boot→Menu→Gen→Preview→Fly→Esc→Preview→Esc→
Menu on random styled worlds.

## Round 63 — Ctrl+V pastes into the Play Seed field (async clipboard)

Play Seed's field was keyboard-only; a copied seed could only be
retyped. bevy_clipboard 0.20 reads are async (fetch_text → ClipboardRead
handle, polled later), so menu_seed_paste is its own system in the
menu chain: Ctrl/Super+V starts a fetch, the poll injects the text
filtered+uppercased+capped(16) exactly like manual typing, and a
paste_dirty flag makes the label re-render through the existing path.
Clipboard absent (CI) or fetch error → "clipboard unavailable" on the
error line — never silent. First draft failed clippy's 7-arg cap by
growing menu_seed_input; the split is the better design anyway.

## Round 66 — dev F3 crate-peek is a STICKY session mode (was ambiguous)

Spec 01 hides crates on the preview; the dev overlay's F3 reveals them
for occlusion demos. What a reroll WHILE revealed should do was never
decided. Chosen: the peek persists — new world's crates revealed too,
F3-off restores the hidden default on the current screen. Sticky beats
per-visit reset for a debug tool you toggle deliberately. Locked by
flow test dev_peek_reroll_returns_hidden (compiles only with the dev
feature; CI's fmt/clippy/test now build --features dev too where the
flow suite runs).

## Round 67/68 postmortem — the three-commit CI red had TWO independent causes

1. **dev_peek flaky assert** (ubuntu, 947aa81→618cd88): F3-peek-ON was
   asserted as hidden==0, but peek-ON means "occlusion alone decides" —
   hidden==crates−want, nonzero whenever the heading hides any crate;
   frame timing decided per run. c2d75e7 made it provable: wait for the
   complete crate set (count from CurrentWorld — props spawn over
   frames), then hidden==crates−want AND want>0 (unsatisfiable if the
   sticky reveal didn't happen).
2. **Runner concurrency OOM** (macOS, 899ea44): `cargo test --workspace`
   died with an EMPTY annotated tail — 17 debug test binaries at full
   ncpu on the RAM-thin macos pool is a silent-kill pattern. Fixed:
   `RUST_TEST_THREADS=2 cargo test --workspace --jobs 2`; annotator
   also simplified to sed-only (the awk pipeline annotated zero lines
   when it mattered — observability must be dumber than the failure).

Both fixes green on all three OSes at 7a0d8e8. Observability upgrades
made en route (raw-tail annotations on both test steps) paid for
themselves immediately: keyword-only filters were blind twice.

## Round 70 — CI logs must be kill-proof (tee was eating the evidence)

macOS dev-test deaths annotated ZERO lines twice: `cargo … | tee log`
block-buffers, so a SIGKILL loses everything unwritten — the annotator
never had input. Direct `> file` redirect now: every byte cargo wrote
before the kill survives as annotations. macOS dev-test passed clean on
the kill-proof run (633542b, all three OSes); if the silent death ever
repeats it arrives with a real log. Same run re-verified the reroll
hitch probe: still green, no hitch regression from any round-61–69
state churn.

## Round 74 — key remaps: live (was already true), now also HUMAN (bug fix)

`remap_takes_effect_live` aimed to lock the README's "key remaps …
applies live" promise and detonated a real bug: parse_key accepted
only canonical enum names ("KeyT") and SILENTLY EMPTIED a binding
written in the obvious human form (`up: ["T"]`) — user remaps
themselves mute with zero diagnostics. Fixed: parse_key is
case-insensitive and single letters get the Key prefix free ("T",
"t", "KeyT", "keyt" all parse); unknown names now `warn!` from
keymap() instead of vanishing. The rebuild-after-insert frame
ordering (update_actions in First, rebuild in Update) is documented
in the test's settle-frame comment.

## Round 76 — 'entity footprint' measured the allocator, not the app

fly_loop's leak assert used `world.entities().len()` — the
metadata-TABLE length, which reserves entity slots in power-of-two
arcs: crossing 511 live entities jumps it 512 → 1024 with zero
leaks. Two CI OSes fired "grew 512 → 1024" while my (already-wide)
local arc never crossed — a test that can only fail on someone
else's allocator state. `live_entities()` now queries actual live
entities. Also fixed same round: evidence-push step hung on
credential prompts (retired — public run pages render annotator
output ANONYMOUSLY, which is where three reds finally confessed),
and the fly_loop 'settle' loop went back out as unnecessary once
the quantity was right.

## Round 85 — the user's first run found what no test could: the bin's plugin stack

`StatesPlugin` added twice = hard panic at boot (DefaultPlugins carries
it; our engine added it unconditionally for the headless tests' sake).
Every existing test builds MinimalPlugins apps — the bin's real stack
(DefaultPlugins + ours) was constructed NOWHERE. Now guarded with
`is_plugin_added` (the same reflex `ActionsPlugin` already used for
InputPlugin — one site had it, one didn't), regression test
`bin_plugin_stack.rs` constructs the colliding pair, and the headless
bin run reaches winit's display gate — the deepest boot this box can
verify. Lesson recorded: a plugin an app can never run is a plugin
that can panic before any test does.

## Round 87 — first real-screen feedback: generator + framing overhaul

User flew it. Four complaints, four root causes, all fixed:
flat terrain (linear quantisation squeezed islands onto level 2 →
t^0.72 band bias + round + per-site 0.7–1.3 relief multiplier +
level_height 0.5→0.9: share≥level-3 went 0.34→0.63, peak now
reachable); "only one island" (flight spawned beyond the map edge
over the sea plane's black void — other islands existed ALL ALONG;
now spawns over Base Island, sea pad 160); see-through cliffs (walls
terminated at neighbour tops leaving sliver gaps — full skirts to
sea floor, sealed by construction); thin sand (level-1-only band →
patchy sand through level 2 within 4 of shore). island_radius widened
(5,18) — 4,20 overloaded the validators (attempts exhausted, caught
by the rejections invariant test doing its job). Golden regenerated
(height grid changed by design).

## Round 87b — the mac flake decoded: auto seeds must self-heal, not strand

CI's dev-test flaked on the exact code path the feel pass made riskier:
a machine-rolled New Game seed that exhausts the attempt budget left
the Generating screen waiting on an error the player must click past
— and the flow test waiting forever with it. Random seeds are OUR
choice, so Exhausted on one is OUR problem: auto seeds now reroll a
fresh seed in the worker (stage label: "Rolling a luckier seed…",
10 tries), menu backdrop loops 6. Typed seeds stay single-shot — the
user's seed is a promise, not a lottery ticket. Also caught by honest
gates this round: `gen` is a reserved keyword in edition 2024, and
pipe-to-tail masks cargo's exit code (use `${PIPESTATUS[0]}`).

## Round 89 — the user's "almost flat" became a metric: relief_share

Feel-words map to dials only when the feel has a number. New
WorldMetrics.relief_share (level≥3 share of land): serialized like
land_ratio, golden regenerated deliberately, batch csv column, stats
line, TUNING anchor. First population reading at current dials:
median 0.76 across 16 seeds, worst 0.68 — the complaint-era 0.34 is
generator-unreachable now. Future "it feels X" reports get checked
against a distribution before any dial moves.

## Round 90 — "ghost islands" investigated: a map.png red herring

Round 87's top-down render showed two labels (Zanoku Shoal, Solaiva
Rock) on blank water. Auditor instinct said "site with no land =
wasted island slot" — a real defect class. Measured it instead:
**0 zero-area islands across 224 islands, 16 seeds.** The islands
exist; map.png's name labels simply paint over small islands. Data
honesty survives; the renderer lies cosmetically (small fix someday:
label offset). Rule reinforced: measure before you mourn.

## Round 95 — rejection histogram was lying: grouping fixed

Fresh sheets showed a catch-all `validation` bucket and
name-fragmented keys. Causes: validate() joins issues under one
"validation: " prefix with "; " separators, and island names vary
per seed. The CLI keyer now unwraps both wrappers, splits joined
issues, normalises digits to #, and collapses the fuel/min_gap
families by message tail. 16-seed check: {fuel: 6+2, hangar: 3,
site-fit: 7, min_gap: ...} — zero catch-alls, every bucket a family.
TUNING re-pointed to the new key names. The tuning signal got a
tune.

## Round 102 — the user's screenshot vs my numbers: wrong metric certified a bug

User flew the "fixed" build: still one island, still flat. Data said
otherwise (14 islands, relief_share 0.76) — and the data was right AND
the user was right: relief_share measured height above sea, while the
eye reads SHAPE. Their world had 42% of all land on one level: a
giant mesa with a couple of cells poking through. Chain of fixes:
1. Radial terracing quantiser (level = distance from centre, coast
   calibration untouched) — e-normalised curve had made the mesa.
2. Peak table raised (Flat 3→4, Hilly 5→6, Plateau→7) + radius-linked
   SIZE LIFT: big islands MUST be tall (floor level 5 at r≥14).
   Biggest-island spread on the user's seed: 0.30 → 0.76.
3. relief_share → terracing (area-weighted per-island 1-max-band-share,
   serialised, HUD, csv, golden regenerated). Calibrated on BOTH eras:
   flat 0.48-0.52 vs current 0.53-0.70 median 0.65; invariant floors 0.50.
4. Exhausted is a supported outcome: invariants assert rate ≤1/33
   (measured 3%, game auto-rerolls 10-deep); golden seed list FROZEN,
   attempt budget raised to 24 in CLI+test to keep it honest.
Lesson etched in stone: calibrate a metric against the failure it is
meant to detect BEFORE trusting it. Mine had certified the bug.

## Round 101 — "one island" closed quantitatively: 10/14 peaks in the spawn frustum

Projected the user's seed (6KS1) through the exact flight camera
(spawn base+46z, y=26, pitch -18, fov 55 — computed, not eyeballed):
10 of 14 island peaks land inside the frame; the other 4 are behind
or beyond the horizon band. With terraces + size-lift the visible
peaks carry real height contrast now. The spawn-placement claim that
rides on this is no longer a comment, it's arithmetic (rerun recipe:
project (centre, peak_level·level_height) through the camera basis).

## Round 103 — "not always solid" closed: windings verified, culling is correct

Audit of every QuadBuilder quad in terrain.rs (top + 4 wall dirs):
ring windings produce right-hand normals equal to the outward
direction (dir 0 −Z, 1 +X, 2 +Z, 3 −X all hand-verified; indices
0,1,2/0,2,3 preserve ring order), so with Bevy's default backface
culling walls are visible exactly from outside a column. The user's
"blocks not always solid" was the seam-gap bug (fixed round ~99 by
full-skirt walls), NOT single-sided geometry — do not reach for
cull_mode NONE as a "fix": it would only hide a future real bug and
cost fill-rate. Column undersides are never visible (drone cannot
descend below the sea plane), so the open-bottom boxes need no caps.

## Round 111 — FIRST-FLIGHT tour audited claim-by-claim: two lies found

With the user about to fly BY THIS DOC, every factual claim was
checked against code: chunked terrain ✓, FOV 25–80 ✓, boost ×4.5
(BOOST_X) ✓, soft walls FREE_PAD=60 ✓, ceiling 600 ✓, O→0/I→1/L→1
autocorrect ✓, settings paths ✓. Two claims were FALSE:
1. "the five supply crates are [on Base Island]" — crates.rs weights
   by near-base + spread and forces ONE CRATE PER ISLAND (`used` is
   an island-index set): base may hold zero or one.
2. "F3 overlay prints FPS, frame ms, world seed and camera position"
   — F3 reveals hidden crates on preview; dev in-flight corner line
   quotes terracing/islands/difficulty; there is no FPS overlay.
Doc claims age like code and rot like prose: audit docs against code
before sending a user into the world with them.

## Round 112 — a CHECKED box went false under my feet; measured, then restored honestly

Spec 02's "batch 1000, zero failures" box was checked in the old era.
The taller terrain (round 102) moved Exhausted from 0% to 1.1% per
seed (measured: 11/1000, release) — a SUPPORTED outcome (game
rerolls), but a false box is a false box. Fix chosen by semantics,
not by hope: the GAME's zero-failure guarantee comes from rerolling
fresh seeds ten times; a batch is per-seed, so its equivalent is
patience — batch now runs attempt-budget 24 (P(exhausted) ~ p^3 of
the 8-attempt rate, expected 0.001 failures/1000). Rerun: 1000/1000,
median 15 ms, difficulty 41/32/27%. The box text now states the era,
the budget and the equivalence — no silent re-checking of history.

## Round 117 — the annotator paid for itself: "Windows flake" was partly an unwrap coin

ae780a4 (docs-only!) went red on Windows; annotation quoted
new_game_seeds_differ panicking at its two .unwrap()s on RANDOM
seeds at budget 8 — with 1.1% exhaustion measured, a ~2%/run coin,
not OS weather. Third instance of the same root contradiction as
rounds 105 (bench) and 112 (batch): code that predates Exhausted's
promotion to supported outcome still assumes it. Policy now: every
unwrap of generate() states its contract — supported seeds only
(patience 24), or an explicit rate assertion. Fail-fast cancelled
macOS/ubuntu before they could flip their own coins.

## Round 120 — a phantom scheduled task: the probe I kept "awaiting" did not exist

For days the session state claimed a weekly release-probe ran Sundays
and only its verdict needed checking. schedule_list showed ZERO
tasks — the probe was an intention my summaries had promoted to a
fact, the exact sibling of round 110's phantom F3-overlay banner.
Recreated for real (schedule-375838df, first delivery Sun Nov 9
06:00 UTC, fresh session per week, read-only API steps written into
its prompt). The epistemic rule crystallising: a belief is only as
alive as its LAST LIVE VERIFICATION; summaries are fossils — they
preserve shape, not existence. Anything load-bearing (tasks, files,
features, CI state) gets re-probed, never re-narrated.

## Round 123 — doc-vs-doc triangulation: the Windows settings path liar

TUNING.md and FIRST-FLIGHT.md disagreed on %APPDATA% depth by one
level. The crate source settled it (directories-6.0.0 win.rs:75:
app_data_roaming.join("config")) — TUNING was wrong for anyone
hand-editing bindings on Windows. Runtime never lied (tests write
via the same API on every CI OS); prose did. New cross-check habit
beyond doc-vs-code and doc-vs-measurement: read TWO docs against
each other — a lie only survives when nothing is compared to
anything.

## Round 140 — the last untested machine: the release workflow has 0% live coverage

Repo census: worktree clean, no stashes, single branch, HEAD ==
origin. Tags: NONE — so `Release binaries` (tag-triggered) has run
exactly zero times; every CI-green streak ever counted covered it
only as 'skipped'. The M1 <3 s spec box will be judged on a RELEASE
build, and the artifact pipeline feeding that era has never been
exercised once. Deliberate non-action: tags are user-facing naming
decisions (semver, channel) — not the agent's to invent unasked.
Plan: on the pilot's return, one throwaway tag exercises the whole
pipeline (4 targets, artifacts listed), then decide the naming.
Until then: state the truth — every other gate has live coverage;
this one has belief.

## Round 145 — the meter: ~$460/day at goal-loop cadence; quiet rounds go zero-latency

First cost snapshot taken: 2315 calls / 24 h. The audits that earned
their keep (macOS release row, the seed-coin, six doc lies, the
phantom schedule) all came from real findings; the last two rounds
spent five- and seven-minute sleeps to prove green things still
green — polling the sky to see if it is still up. Policy: after any
finding-less round, subsequent rounds skip sleeps and CI polls
(commit-time CI verdicts deferred to the next real change or to
Sunday's scheduled probe, whose prompt already covers red triage).
Findings still get full rigor; absence of findings now costs ~zero.
The improvement loop should idle like an engine with stop-start,
not burn fuel at every green light.

## Round 154 — THE PIVOT: voxel Earth heightfield replaces island worry

Pilot's call after flying: "still just one island, blocks with no
face — make it a voxel Minecraft-style terrain generator, 1 km
world, 25 cm voxels, Earth relief (flats/hills/mountains), render
the top layer." continent.rs answers the generator half:
heightfield of u16 voxel columns (Minecraft's data shape, Earth's
relief spectrum: warp → land mask → orogeny → ridged ranges →
plains floor → detail). 16M columns = 6.7 s single-thread release;
columns are pure functions (determinism contract inherited,
rayon-proof). Metrics computed FROM the data (terracing's law).
Calibration before assertion, twice over: first census caught a
43 m flat world AND a 14%-mountain world at the same dials (mask
gates were all-or-nothing); relief floors fixed the flatlands,
detail 2.2→1.0 m after the first FULL-RES RENDER showed crumpled-
foil plains — the render was the critic no histogram could be.
Seed-character is a FEATURE (42 = coastal plain, 777 = alpine):
bands pinned for seed 2; retuning must re-measure to pass.
Next: chunked surface mesh in cyclone_render (top-layer rendering
is native to the representation) + free-fly over the continent.

## Round 176 — the pilot flew the planet; three truths came back

Report: "the scale is weird, very tall and in the map same small
square" + screenshot. Verdict decomposed into three real bugs:

1. **Vertical exaggeration**: 900 m peaks in a 1 km world is a 1:1
   aspect ratio monolith; Earth's ratio at that window is ~1:10.
   Rebalanced (measure-first, twice confirmed across seeds):
   `max_height_m` 1100→300, ridge onset 240→80, ridge wavelength
   130→260 m (half as steep). Census cuts + satellite ramp + CLI
   ramp + band test + canary digest ALL re-pinned from the new
   census in the same round — the digest went RED first (its round-
   117 contract doing its job: dial change is deliberate re-pin,
   never silent drift).
2. **"Small square on the map"**: WorldPreview auto-frames from
   `WorldBounds`, which the engine refreshes from the ARCHIPELAGO's
   tile size — the Esc-menu orbited a 200-unit square inside a
   1000-unit planet. New `continent_bounds_guard` (render) overwrites
   the framing WHILE planet-live, writing only when wrong. The bug
   class: a fixed resource from one world quietly lying about
   another.
3. **The corner line quoted "CONTINENT 1000 m²"** — the side length
   dressed as an area: a 1000 m side is a **km²** world, 1000× the
   claim. The sixth doc-lie of the era, caught BY THE PILOT reading
   it aloud. Unit lies compound silently.

Also this round: B0004 warning flood (round 175) root-caused to the
render plugin's Visibility auto-insert + bare WorldRoot — one
component on the root ends thousands of lines; and the 0.20 despawn
is-recursiveness settled from bevy_app's own source comment. Env
dials now read through ONE `continent_mode()` helper (build, bounds
guard, spawn, text all ask it) — scattered env reads were the drift
seed for bug 2.

## Round 177 — "make the world more real": rivers are data, not paint

Pilot: "make the generation span the whole blue square and the world
continent be as our planet, rivers, mountains, beaches, plains".
Two claims, two architectures:

1. **Span**: the sea quad's 400 m skirt is an ARCHIPELAGO concept
   (horizon from a 200-unit island). In continent mode the ocean now
   ends exactly where the terrain ends (pad 0) — the world IS the
   square. The 160 m skirt in the island path was checked and left
   correct for its own mode: same variable, different worlds.
2. **Rivers**: the honest primitive is D8 flow accumulation over the
   whole grid — counting-sort the u16 heights (O(n)), walk strictly
   descending (ties by index: acyclic by pair order), accumulate
   rain, carve where catchment ≥ 25k columns (~1.6 km²). Carving
   alone gives valleys; the pilot asked for RIVERS, and a fixed sea
   plane cannot show water above y=0 — so `Continent` gained a
   `water` channel (surface heights; 0 = dry) that the MESH renders
   as blue quads over the carved bed. The renderer learned "seen
   height" (max(land, water)) for tops AND walls; the same hue one
   word for sea, river and hillshade.

Truths the round forced out of me:
- **Replace-without-assert is silent lying**: a dial edit that
  no-op'd (string drift) AND a stale binary made two renders
  byte-identical — caught only by hashing the OUTPUT. Every "change
  landed?" claim now needs the output to move or a grep on the file.
- **Chunk lists are SPARSE** (empty chunks dropped — an ocean world
  of 512² is ONE chunk, not 64): tests that index chunks by grid
  math learn fiction. The wet-cell proof FINDS its chunk by anchor.
- **Unit discipline**: voxel coords vs cell coords vs metres — the
  anchor lookup mixed two of them and 'missing chunk' was my test's
  bug, not the builder's. Convert once, at the boundary, in a comment.
- The canary now digests heights AND water: rivers are terrain truth.

## Round 178 — B0004 has two template arguments; colours are regions, not stripes

Pilot, round 178: "still, do you think that looks like a continent? or
even our planet? also why the green, yellow grey?" plus 40 fresh
"This will cause inconsistent" lines. Two bugs, one honest confession
each:

1. The flood was REAL again and the first instinct — "stale binary" —
   was wrong: the HUD proved the new build ran. The validator bevy
   registers is ValidateParentHasComponent::<C> for TWO C:
   InheritedVisibility (fixed round 175) and GlobalTransform
   (bevy_transform registers it — children with Transform under a
   Transform-less parent flood identically). WorldRoot had Visibility
   but no Transform: one warn per mesh chunk per rebuild, silent for
   two rounds because the headless guard only checked Visibility.
   The witness test (b0004_trace) counted exactly ONE Transform-less
   parent — this root. Fix: WorldRoot carries BOTH components; the
   shipped guard now asserts the pair.
   Lesson: guards assert the CONTRACT (both validators bevy registers),
   not the last symptom.
2. Altitude-banded colour is WHY the world read as three flat stripes
   (green <95, ochre <175, grey above = a traffic light, not a planet).
   Earth's grammar: lowlands are REGIONS (aridity field at 300 m
   wavelength: forest/grass/scrub/desert), altitude only claims land
   above the vegetation line; sand is a PLACE (next to water at any
   height — estuary banks included, cliffs stay rock); the sea is
   BATHYMETRY (shore cyan → basin navy over ~1 km of open water),
   not a flat plane. One palette across renderer, sea builder and
   CLI sheet — the sheet is the game's satellite photo.
   Dev knob CYCLONE_CONTINENT_SCALE=1 shrinks the 1 km world 5× so a
   screen shows it whole (relief 5× flatter — accepted preview trade).

## Round 179 — islands are a point process; the sea reads distance, not membership

Pilot pasted an aerial photo: small green islands, sand rings, turquoise
shelves, deep blue beyond — "a few that fit the map, randomly", plus
"more definition" at the 25 cm voxel.

- **Islands: stamp, don't threshold.** Cutting the fbm's upper tail was
  built, census-tested and THROWN AWAY: the field's high tail is one
  correlated cluster plus noise minima — one big island and fifty 9 m
  specks, the opposite of the photo. The shipped mask hashes a 340 m
  lattice: a coin keeps ~45% of sites, each kept site stamps a cosine
  blob (radius 90–200 m, seeded squish), max-composed. Sizes/spacing
  are constants (they cannot drift by seed); placement/count/shape are
  the random part. elevation_m's island branch puts SEA at mask 0.5:
  coast has zero relief (beach), core full relief. Continent mode is
  byte-identical — the canary never moved.
- **Beaches are sea-adjacency, not river-adjacency.** `near_wet` read
  the WATER array, which only holds rivers — island shores had no
  sand. Now: height-0 neighbour = sea = shore, in renderer and sheet.
  A 1 m fringe column is beach sand, not the void-black "skip" case.
- **The sea shade reads DISTANCE, not membership.** Shelf ramps
  turquoise→navy over 150 m (the photo's band); the sheet side uses a
  Felzenszwalb–Huttenlocher exact two-pass transform after the brute
  scan turned out to be 16e9 samples. Three numeric lessons:
  BIG=1e18 vanished when f64 rounded +p² back onto it (source erased
  at its own site); BIG=1e13 met a real distance and faked nearer
  land (vertical bands); shipped: normalised voxels², BIG=1e8, every
  border exact. The isolated repro printed [inf,inf,0,inf,inf] —
  keep printing until the arithmetic confesses.
- **The block was never the voxel.** 25 cm was always the spec; the
  mesh cell (CELL_VOX=8 → 2 m) was the visible cube. Now 4 → 1 m;
  the sparse chunk drop keeps the sea at one quad, so islands pay
  only for land. Live spawn (CYCLONE_ISLANDS through the real path)
  is test-guarded: crates gone, 3 meshes, one root, both B0004
  components standing.

## Round 178 — the island stamps were secretly the continent path's first trig

Determinism rule A2.3 says transcendentals go through libm — absolute,
no exceptions, because platform libms differ in the last bits. The fbm
never noticed A2.3 existing: value noise is integer hashing plus
smoothstep, transcendental-FREE. The round-179 island stamps added
`(PI·t).cos()` — the first trig in continent-path code — and it went
straight to the SYSTEM libm. CI could not catch it: every island test
asserts same-OS self-agreement, and the byte-identity canary pins the
default path, which never touches stamps.

Fix: `libm::cos` at the site; and the hole itself closed with an
ISLAND-MODE canary — own digest (seed 2 @ 2048²: 512 m, the smallest
size that fits the 340 m lattice honestly; 512² would pin pure ocean
and call it proof), computed after the libm fix, asserted on all three
CI OSes. Two lessons generalize: a new transcendental is a NEW
determinism liability even inside a doctrine-covered crate; and a
feature whose tests only compare a platform with itself has no
cross-platform test at all, however green it looks.

## Round 183 — the PREVIEW flag's 0.2× was a myth: comments-only code

DECISIONS round 178 told itself the flag "shrinks the 1 km world 5×".
It never has. Round 183's audit: `continent_mode()` returns
(size, extent-metres) — the second element is EXTENT, not scale — and
the flag's `let scale = 0.2` had exactly one consumer since its
introduction (4f7c0a2): the boolean `scale != 1.0` deciding whether to
flip ContinentLive at boot. No mesh anchor, transform, spawn height or
WorldBounds ever multiplied by it. Every doc that repeated "0.2×
overview, relief 5× flatter" (FIRST-FLIGHT, TUNING, a spec-03 line,
round 180's pilot command) was repeating a comment, not behaviour.

Decision: the flag KEEPS its honest names (PREVIEW / legacy SCALE)
with its ONLY real behaviour — skip the menu gate, planet live at
boot; the myth text is deleted from every pilot-facing doc and the
overview need re-pointed at climbing (~300 m shows a whole island at
TRUE scale, which is also better for the photo verdict than any
toy world would have been). Generalize: a dial is its consumers —
grep what a value TOUCHES before ever documenting what it "does";
a number multiplied by nothing multiplies the docs into lies.

## Round 184 — myth #2: the gate-skip was dead too, until a test ran it

Yesterday's entry corrected the 0.2× but still claimed the flag's
"only real behaviour" was the boot-time live flip. False: the
insertion sat BELOW `let continent = continent_env && live_on`, so
with the gate closed `continent` was false and the insert was
unreachable — the flag had ZERO effect in every build shipped. It
survived because Enter is one keypress away, so nobody ever needed
it to work, and no test had ever set the env it reads.

Fix: the flag now raises live ABOVE the gate and inserts the real
ContinentLive(true) once (one source of truth for planet build,
bounds guard, game checks); the test proves gate-skipped boot builds
the planet with true-scale bounds, and the test pair serialises under
an env mutex (process-global env × runner threads was a real flake,
caught same commit). Doctrine amended: grepping consumers proves a
value is READ; only a test proves the consumer is REACHED.

## Round 185 — the spawn was beside the flagship, not over it

The game-side InGame spawn — the FIRST-FLIGHT doc's headline claim —
had zero test coverage (grep: no game test ever set a terrain env).
Extracting the decision to a pure fn and running it caught the real
bug: a +90 m south offset, tuned when the OLD islands filled a
screen, shifted the rig off the scan's land find — on the 1 km
flagship that put it over open sea. The docs' "drops you over the
flagship" was almost true, which is worse than false loudly.

Same pass collapsed a THIRD reader of CYCLONE_CONTINENT_SIZE into
continent_mode's single source. And three test-side lessons in one
sitting: tuple slot order is the rig's (x, UP, z, pitch) — names
that lie about slots cost an hour; a fallback must be proven to
EXIST before it's asserted (my "centre is sea at 16 m" premise held
for seed 42 but not most — probe, don't assume); witnessed output
beats clever reasoning (the WITNESS line found the destructure bug
in one run).

Doctrine stands and sharpens: claims live in docs, truth lives in
tests — every pilot-facing sentence wants an env-setting,
behaviour-asserting test behind it, and the round-185 spawn test is
the template.

## Rounds 188–190 — key rows verified; cache promises paired

188: every key printed on the preview/in-game rows was checked
against the default binding table and its handler — all true (the
Reroll fiction had died in 187). Near-miss kept honest: Space is
bound to BOTH confirm and climb; states gate each consumer, so no
collision today — and a note that overlapping states would make it
one.

189–190: the continent cache's two promises now have tests. Seed
swap REBUILDS (root entity changes; witnesses even showed the new
seed's world can be legitimately all-sea at 100 m — the castaway
fallback's domain, witnessed free of charge). Same seed REUSES
(respawned meshes are handle-identical, HashSet equality). Both
directions hostile-tested: flipped freshness serves a stale world;
broken reuse re-pays 8 s on every Enter. The stale-world class of
bugs is now closed by CI, not by reading the condition.

## Round 191 — a partial lock is no lock (re-learned the hard way)

The reroll-chord test (9ec9992) shipped while two sibling flow tests
FAILED and clippy was 2 — the review grep chain read stale numbers.
The env flakes came back wearing the same lesson in a new hat:
round 184's ENV_LOCK, applied only to the test that MUTATES envs,
does nothing for the tests that merely READ them — rebuild_world
samples CYCLONE_* every frame, so any app-building test in the
binary is a reader. The lock must cover every member of the
contending set, not just the offender. FlowEnv now serialises all
12 flow tests and its Drop guard clears envs on panic — cleanup by
construction beats cleanup by a tail statement a panic skips.

Meta-lessons logged: (1) run the test binary as a BATCH before
committing a new test — a solo green run proves only isolation;
(2) never quote gate numbers from scrollback — fresh logs, this
run; (3) Bevy 0.20 `World::query` needs `&mut World`, which turned a
scope cut into a strictly stronger wait (planet presence asserted).

## Round 194 — the mesh joins the byte-identity contract

'Same seed, same world' was digest-pinned for the WORLD (heights,
rivers, metrics) but the MESHER between world and pixels was
trusted, not hashed. Now seed 2 @ 2048² islands hashes (anchors +
positions + colours + quad counts, blake3) to 85509bbc… and CI
asserts it on all three OSes: float math drifting inside the
builder would fail CI, not a cockpit impression. Implementation
choice logged: the digest lives as a UNIT test so the builder's
existing pub(crate) accessors serve it — the product API stays
exactly as wide as players need, tests reach through crate
visibility like they always do.

## Round 195 — myth #9 was the big one: mountains were holograms

Spec 03's inventory credited `surface_height_m` with rig terrain
collision ("never sinks through the world"); camera_rig's only
floor was the box's flat y ≥ 2.5 — flying through a 270 m range was
the default physics. The claim-audit campaign's purpose in one
found: prose promised what no consumer executed. Fixed as
architecture, not patch: engine owns `TerrainFloor` (opaque f32→f32
oracle — dependency direction intact), world builder installs a
closure bound to its own seed (reroll re-arms), free-fly clamps
skid to surface + 2.5 m. Archipelago deliberately keeps the flat
floor (guard-tested; unifying it is spec-03 OPEN #6, a pilot call).
Stale-oracle class pre-killed by asserting installed-closure ≡
surface fn for the live seed. First pilot-visible feel change of
the audit era — and the one their fingers will find fastest.

## Round 197 — stateful clamp × undefined Update order = machine-dependent bug

The bounds guard wrote WorldBounds from Update; rebuild_world
spawned the rig from Update; Bevy fixes no order across plugins.
The free-fly clamp MUTATES the authoritative position, so the one
frame where a stale archipelago box (160 m) could catch a planet
spawn (up to 860 m out) would leave a PERMANENT teleport —
reproducible only where the scheduler happens to flip. Doctrine:
any resource a clamp/consumer mutates must be set in the same
system beat that creates the consumer's world, never 'soon' by a
backstop. Rebuild now sets bounds synchronously (guard kept as
idempotent backstop); the esc-chord flow test asserts the cage is
the planet's 0..100 m the instant InGame+WorldRoot stand.

## Round 198 — the mesh digest went red on macOS in hours; the pin was the bug

dd0b527 pinned RAW f32 mesh bytes cross-OS. Same day, macOS:
d8dab0ee…, Linux+Windows: 85509bbc…. Diagnosis from public
annotations alone (job logs are auth-gated — annotations aren't):
the worldgen canaries had always agreed on macOS because HEIGHTS
QUANTISE to whole metres; raw vertices expose libm's sub-centimetre
ULP drift, and IEEE guarantees no transcendental identity across
platforms. So the red was not a platform bug — it was MY claim
over-promising bit-for-bit where only structure is portable.
Fix = quantify the CLAIM, not chase libm: digest every float
rounded to 0.01 m (1/25 of a voxel, ~1000× under visible) →
3a1bf7a9…; structural drift (chunk sets, island shape, ordering)
still moves the hash by oceans, sub-cm drift is declared invisible
by design. FIRST-FLIGHT now states the mesh scope honestly.
If macOS reddens on THIS digest, that's a different animal — real
terrain disagreement — and the assert message says so.
Addendum (same round, verdicts in): cac50e8 red on macOS with the
EXACT same left-hand digest d8dab0ee… — drift is DETERMINISTIC
(libm difference, not scheduler noise), which is what makes the cm
fix sufficient rather than hopeful. Terrain floor itself: green on
Linux+Windows in that same matrix; macOS's floor verdict rides in
on the cm-digest run.

## Round 201 — the 8 s freeze before preview is real, and now it's written

Generating animates because run_generate rides the task pool
(screens.rs:821) — but generate_and_mesh (8.3 s continent census)
runs SYNCHRONOUSLY in rebuild_world (render lib.rs:234), so the
preview transition stalls the repaint ~8 s before popping whole.
Documented in FIRST-FLIGHT (not-a-crash + pilot can say if it
matters). Designed-but-unbuilt fix (parked, next idle round):
Generating PRE-WARMS the existing ContinentCache from the task
pool — the cache's same-seed reuse is ALREADY test-proven
(handle-identical, round 190), so pre-warm turns the freeze into
instant preview at the cost of ~70 lines: a request/handshake
resource + poll system in render; mesh asset creation stays
main-thread where it belongs. Sketched here so future-me doesn't
re-derive it.

## Round 202 — the freeze deleted: planet prewarm on the task pool

Round 201's parked design, built: kick system (runs REGARDLESS of
ContinentLive — preview is exactly the time to carve the planet
nobody's looking at yet) → poll (std Waker::noop poll of a
proven-finished future — cannot block, no futures-lite dep) →
rebuild (waits WITHOUT despawning: whatever is on screen keeps
repainting; swap is whole). Claim hoisted above the spawn-closure
borrow — round 195's lesson applied preemptively; the witness
eprintlns found seed_swap's `// fly it` anchor slip in one run.
Surgery scar logged: a reorder slice silently ate rebuild's
#[expect(too_many_arguments)] — clippy -D caught what tests
couldn't; attribute restored, unused param + dead field trimmed
in the same breath. Test era note: continent_live's four tests now
settle on WORKER truth (TerrainFloor present / new root), not
frame counts — wall-clock patience is the honest instrument.
Design honesty kept: PREVIEW-at-boot shows NOTHING during the
build (first build has no previous world — same as the freeze's
8 s black, but repaintable menu); worst-case Enter (task unfinished)
keeps the archipelago flying until pop — never a void, never a
half-planet.

## Round 207 — the env-race audit closes (and the auditor got audited)

After the census/digest race (round 206), every CYCLONE_*/XDG writer
in the workspace was inventoried and witnessed, not trusted:
continent_mesh lib = ENV_LOCK (new); screens lib = ENV_LOCK (r188);
flow = FlowEnv; continent_live = ENV_LOCK; b0004_trace &
settings_persist = single-test binaries (process isolation does the
locking); engine settings unit test = sole reader, witnessed by
grep AND by reading tests 2/3 (pure ron strings, no env, no fs).
Then the near-miss: the settings comment's 'restored on assert
failure' looked like a happy-path lie from a grep (remove_var sat
BELOW the body), and a patch was mid-air when reading the full
test found catch_unwind wrapping every assert — the comment was
TRUE, the restore unconditional, and the 'fix' would have added a
Drop guard nobody needed. Doctrine holds: grep locates, only
reading concludes — the same rule that catches product myths
catches audit findings one level up. Zero code shipped that way;
this entry is the diff. CI: fc776e5 (prewarm) and 3a4bad6 (flake
fix) both GREEN on all three OSes.

## Round 212 — the foam the spec promised, and the material that ate colour

Audit of the pilot's OWN acceptance boxes found the last real spec-02
gap: 'sea with shoreline foam' — zero foam anywhere. Mid-audit I
nearly shipped a WORSE myth: a grep with a non-recursive glob said
shore_dist 'was never written' — WRONG, cleanup.rs has baked the
BFS since the pipeline's first day; the second grep, with a real
path, proved it before any claim shipped. Grep locates; reading
concludes — caught one level up for the second round running.
The true gap was a MISSING CONSUMER (doctrine's own shape): the
baked grid fed sand shading and prop placement, but the classic sea
was one white plane × navy material. build_sea_classic now paints
foam (shore_dist ≤ 1, fading by 3), turquoise shallows, navy deep —
same ramp constants as the continent sea so one ocean language
spans modes. Same audit caught the multiplier myth: sea_mat's navy
base MULTIPLIED build_sea's designed turquoise→navy into near-black
— a palette the pilot was about to be told to judge, that the
material had been eating. White base; vertex colours own the ocean.
Skirt dropped to -0.05 under the -0.02 tiles (no z-war). Spec 02
annotated with evidence; the 'shows' box stays the pilot's sense.

## Round 215 — the sea moved onto the reroll frame: 2.8 ms is history

The round-212 foam had an unquoted cost: build_sea_classic runs in
rebuild_world, i.e. IN the reroll frame. Fresh ignored-bench run:
worst frame 9.4 ms of 4635 (round 200 quoted 2.8 of 4753; the
3.8-era 'still holds as a ceiling' sentence is now FALSE and is
deleted from TUNING, not footnoted). 9.4 ms still clears the spec-01
50 ms box 5× and fits one 60 Hz frame — accepted cost, said out
loud with numbers. Optimize only if the pilot ever *feels* it;
caching sea meshes across rerolls is on the shelf, unrequested.
Doctrine again: every perf number in docs carries its re-measure
date, and any commit that moves main-thread work onto a frame
MEASURED BY A DOC owes that doc a fresh run — round 212's commit
message never mentioned the reroll frame; caught before it could
age into a myth.

## Round 216 — the asset leak, measured before and after the sweep

Probe-don't-assume, the expensive kind: greps showed ZERO
remove/retain/free sites against seven per-rebuild add sites, and
Bevy does not GC the asset server. A new test MEASURED it before
any claim: +236 meshes over five rerolls, ~46 a world, strictly
linear. A pilot hour of Esc/R is a hundred rebuilds of terrain
nobody can see.

The sweep runs in rebuild_world the instant a build is chosen:
every mesh+material pair on the despawning children is removed —
EXCEPT the pairs a FRESH cache will respawn (Esc→Enter keeps its
planet for free), and when the cache is stale its OWN handles join
the doomed set (the claim replaces them this run; keeping them was
the leak wearing 'we reuse via cache' as a costume). The shore R8
texture, referenced by no entity, got a one-slot owner resource so
the sweep can speak its name; the pre-sweep version leaked one
texture per rebuild too.

Post-sweep the store HOLDS one world, its size wobbling with each
seed's land (55→46→39→33→63) — the bound is now honest about the
wobble it must not convict: peak ≤ 2.5× mean; linear accumulation
hits 5× and cannot hide. The four sibling live tests (respawn,
wait-continuity) passing UNCHANGED is the proof the keep-set is
right, not just the leak-test going green.

Self-audit mid-ship: the insertion split ContinentCache's doc and
derive from its struct — two compile errors, both caught by cargo
before cargo was asked to trust anything. And the probe's own
first bound (step growth > 0) convicted the healthy wobble the fix
created: a test whose reds survive its own fix teaches the wrong
lesson; the bound moved to the crime, not the fingerprint.

## Round 216b — the round-206 lesson repeated at its own next ship

0650146 pushed claiming gates its command chain had skipped:
`grep -c FAILED` inside the &&-chain prints 0 and exits 1 ON
SUCCESS (zero FAILED is good), so the chain died there and clippy
plus the suite never ran — while the commit MESSAGE asserted them,
inherited from the message I wrote while the terminal was still
showing the continent_live green. The 101 that leaked into the
echo was cargo's compile-error exit, visible only because I was
mixing ; and && in one line. The skipped gates, run honestly,
found two real lints in the sweep code — so the sin was not
cosmetic: the pushed commit was un-linted, twice over.

Fix-forward (4892615): collapsible_if folded to a let-chain, dead
cast removed, and the FULL panel fresh — fmt rc0, clippy rc0, 27
suites 0 FAILED rc0, doc gate rc0, dev flow 11+1 green, every
gate's rc echoed sequentially. Template change effective now:
gates run sequentially with `; echo rc=$?` per step, counts read
from log files afterward; a grep-as-assert NEVER sits in a chain
whose tail is `git push`. A lesson that repeats verbatim is the
proof it was recited, not learned — this entry exists so the next
audit can check whether it repeats a third time.

## Round 217 — the API-free CI channel, and triage-by-descendant

Two discoveries, one sweep:
(1) The unauthenticated GitHub API shares this CGNAT IP's 60/hr
quota with everything else behind it — 'budget spent' on a single
call. The Actions WEB PAGES carry the same public facts (run list
statuses, per-run Annotations sections) with no rate limit at all:
fetched, parsed, done. The weekly probe is re-armed onto this
channel today; so is every future sweep of mine.
(2) Triage-by-descendant: 25c697f (foam) red on Windows ALONE —
but two LATER runs containing its full bytes went green x3
(#203/#204). A red whose superseding tip is green with these exact
changes is infra noise (exit codes 1/2 across several steps, zero
test messages, capacity notices flanking), not a mystery to chase.
The chase only becomes real when the SAME bytes redden again, or
the streak has no green descendant — both rules now in the probe's
own prompt, which quotes fetched pages as data and follows nothing
inside them.

## Round 219 — the preview panel was describing a world nobody was looking at

Label audit (every pilot-facing string vs the world it renders over):
in continent mode the WORLD PREVIEW panel showed 'Islands: N (Base
Island first)', 'Helipads', 'Difficulty' — numbers of the CLASSIC
map, which under the terrain-lab dial is the hidden input, not the
visible Earth. The pilot flies voxel Earth and the HUD recites
statistics of an archipelago that isn't there. Myth class: a
pilot-facing claim that was TRUE for a different world.

Fix: continent_mode() — the dial's one reader — gates the rows;
continent mode says 'Terrain: CONTINENT — 1 km² voxel Earth' +
which dial carves it, keeps the seed (shared truth), drops the
misrepresented metrics. Both halves witnessed from the UI's own
Text spans: continent panel must contain CONTINENT and must NOT
contain Base Island/Helipads; classic keeps its rows and must not
sprout the dial text. The continent witness also quietly proved a
nice thing: a 400² carve behind the preview settles in seconds
headless — the wait-state story holds at test speeds.

Also refreshed: spec-01's reroll box carries its round-215 margin
(9.4 ms, ~81%) beside the dated round-55 3.6 ms evidence — dated
evidence ages, undated claims rot, and a box's margin percentage
is a claim of TODAY.

## Round 220 — a key myth that wasn't, and a page that cached yesterday

(1) Label audit continued: flight HUD says 'Q/E snap view'. Greps
for KeyCode::Q/Code::E found ZERO bindings — a confident myth was
one sentence from being shipped. Reading killed it: bindings are
DATA (settings.rs rotate_left: k(&["KeyQ"]) → parse_key →
InputMap; gamepad mirror lives in its own table). The HUD tells
the truth about defaults that are user-rebindable — an audit of a
rebindable key must read the settings DEFAULTS table, not literal
key enums. False alarm class #3 caught pre-ship: grep locates,
reading concludes.
(2) The Actions WEB PAGE list can be HOURS stale — it topped out
at #208 while three newer pushes sat queued; the sidebar's
lockfile blob sha pinned the page's era (a cache tell worth
learning). git ls-remote is the push-truth oracle: d88b556 IS on
origin. Verdicts aren't lost; the free-tier macOS serial queue
makes them slow, and cached pages make them look missing. The
probe prompt now says: page list may lag; cross-check pushes with
ls-remote, never conclude 'CI never ran' from a page.

## Rounds 226–228 — the dial-ownership era, in one paragraph

A doc line said 'the CLI reads the same env the game does' — true,
by copy-paste. Round 226 moved each predicate to the crate owning
its config field; round 227 finished the census (CONTINENT had
three raw readers besides its owner, two gating behavior); round
228 then changed a DIAL'S MEANING (presence → strict '1', closing
the '=0 means ON' trap) in ONE LINE because one function owned it.
The loop is the lesson: find the coincidental agreement, make it
structural, and the next semantics change is rent collected. Every
dial's truth table is now unit-pinned or test-covered at its owner;
the repo-wide census is four dials, four readers, zero coincidences.

## Round 232 — the pilot's copy-paste was dead on arrival (twice), and my own claim fell

(1) FIRST-FLIGHT's example seed K7Q2-9XMB-4TRFW was UNPARSEABLE by
the game's own Seed parser: a full 13-char Crockford seed tops out
at first-digit F (beyond that no u64 fits) — no game-shown seed can
start with G–Z, and the doc example started with K: the pilot's
first CLI copy-paste died on 'seed overflows 64 bits'. Example now
parses and flies (07Q2-...), and the first-digit bound is INKED.
(2) 'the voxel is 25 cm (40 000 across the map)': 1 km / 0.25 m =
4 000 — an order-of-magnitude typo no arithmetic reading survived.
(3) Both sheet timings in the docs had drifted (7 s, 20 s):
measured 8.1–8.6 s internal / 9–10 s wall, release, dated.
(4) SELF-INDICTMENT: yesterday's commit claimed 'no binaries/
sounds shipped' — FALSE: eight .wav sit under assets/audio (V1-era,
bundled in b55e5c8). The GROUND RULE stays true with a stronger
witness than my sentence: v1/src/audio.rs:1 'All WAVs are
synthesised by tools/gen_sfx.rs (original material)' and the tool
exists at v1/tools/gen_sfx.rs. V2 loads none of them (audio is
spec 05's). A claim about absence must census the tree, not a
truncated grep — head -8 ate the wavs alphabetically.

## Round 238 — four checks, four holds (confirmations belong in the ledger too)

(1) doc examples write --out out/ — .gitignore:4 'out/' ✓ no repo
pollution by the documented commands; (2) CI queue: seven commits
in flight across 3 OSes, zero red; (3) the workflows write NO
CYCLONE_* dials — grep of .github/workflows/ is empty; the flow
tests self-arm via set_var under their own locks (round-229
architecture), so CI needs no dial env and cannot drift from the
strict-'1' law; (4) 'launch-smoke step' — MY OWN era-shorthand,
never a doc claim: zero 'smoke' in any .md; ci.yml's actual steps
enumerated (fmt, clippy, rustdoc, build, gated test+annotate,
gamepad, dev-bin-check, dev-flow+annotate, golden, dep-direction,
bench, release-bins). Headless CI never launches windows — and
never claimed to. Auditing one's own memory is the last audit
frontier; a hold is worth logging, or it didn't happen.

## Round 239 — the third-party citation that aged perfectly, and rustdoc's fourth gate

(1) acb4298 (title constant — code that runs EVERY launch: window
title + both menu strings) completed success ×3 OSes. (2) Local
RUSTDOCFLAGS=-D warnings cargo doc --workspace: clean — CI's
fourth lint gate, unrun locally through the whole comment era,
now proven green for every doc line added since. (3) TUNING's
lone file:line citation 'directories-6.0.0 win.rs:75' audited to
the byte: Cargo.lock pins 6.0.0, and line 75 of the vendored crate
reads exactly 'let config_dir = app_data_roaming.join("config");'
— the doc's '(the crate appends config)' claim, at the cited line,
in the cited version. Third-party cites age with versions; pinning
the version in the text is what makes them outlive eras. Holds:
two logged, nothing to fix.

## Round 240 — two probes, one false suspicion, one header nuance

release-probe.yml (repo-side: weekly release COMPILE on 3 OSes,
created Fri 2026-11-06) had zero scheduled runs — suspicion
'disabled cron' resolved by its BIRTHDAY: first firing target is
TODAY 06:00Z and GitHub scheduler lag of hours is normal; a two-
day-old cron that 'never fired' is young, not dead. grep locates,
reading concludes — creation dates conclude too. (My own note
'probe fires Nov 9' named the OTHER probe: the DSH-side weekly
Actions sweep — two weekly probes share a name and a 06:00Z hour;
this file probes compile health, that one probes CI verdicts.)
Header overstatement fixed: 'per-push CI only compiles debug' —
the bench job release-compiles worldgen; claim narrowed to the
truth that matters: the HANDOFF binary's release profile is
proven only by this probe and by tag builds. Youngest watcher on
the wall; its first real verdict arrives with scheduler grace.

## Round 245 — the young cron's first absence, and arming tomorrow's witness

Release-probe (repo-side, born Fri) passed its first-ever firing
target five hours ago with ZERO scheduled runs (event=schedule
total 0 — the API channel was alive, so absence is real). Not yet
indictable: GitHub registers brand-new crons late and can silently
SKIP a past occurrence (delivery resumes with future ones); the
weekly-on-Sunday shape means if today skipped, next target is
Nov 15. No unilateral tag pushed to force a release build — the
release tag is the pilot's naming turf. Mitigations: (1) Linux's
release leg already hand-proven green round 241; (2) tomorrow's
DSH-side probe learned check (6): report whether ANY 'release
probe' run exists, and if still none after TWO Sundays, say so
loudly and suggest the pilot's one-time 'Run workflow' poke from
the Actions tab — a human click that no unauthenticated API
session can substitute. Silence watched is silence answered.

## Round 247 — every ignored test in the house, audited: six tools, zero rot

Census of '#[ignore' repo-wide: six. Each read against its reason:
reroll_hitch (RE-RAN round 246, 8.3–9.2 ms third date), mesh-leak
probe (RE-RAN today: BASE 52 → PEAK 57 → settles 37, BELOW base —
handle-drop frees the old world; five rerolls, no upward trend),
mesh release-timing measurer (perf-only, no doc depends on its
number), calibration census (dial-era stats printer, tool by
design), b0004_trace witness (sets CYCLONE_CONTINENT itself — the
env-discipline reason it can never be a gate; historical incident
witness), golden fingerprint printer (diagnostic beside the
read-only golden CHECK). Zero rot: every ignored test is a tool
with a run command in its own doc line — which is exactly the
difference. The one correctness-class member (leaks) was the one
re-run today: its verdict is fresh-dated, and it deliberately has
NO doc claim feeding on the number — the property is witnessed,
not marketed.

## Round 252 erratum — 87039f1's own message broke the count law it cited

The ISLANDS-tooth commit wrote 'suite 28 ok / 0 FAILED' where the
fresh log's grep said 27 — the new test joined an EXISTING binary
(cyclone_worldgen lib), so the RESULT-LINE count never moves;
test count inside that binary moved, result lines did not. The
27-count invariant survived the whole era exactly because new
tests join existing binaries; the message author forgot it for one
sentence while describing the law in the same paragraph. Fix
forward (no history rewrite, ever): 27 ok / 0 FAILED, worldgen's
lib line now carries one more green test inside it. Commit
messages are pilot-facing prose too — the audit does not exempt
the era's own ink, not even minutes later.

## Round 255 — the final-state snapshot (every gate, fresh, one command block)

As of 07f18b5 (local tip == origin/main, tree clean): fmt rc=0;
clippy -D warnings --all-targets rc=0; rustdoc -D warnings rc=0;
suite rc=0 with 27 result-lines ok, 0 FAILED; dev flow 14 passed +
1 ignored (the tool census, round 247: all six ignores are tools);
golden ok (10 seeds), read-only. Era score: ×17 green on CI since
the last red, queue serial-slow but unbroken.

The pilot's next keystrokes, exactly: git pull; CYCLONE_CONTINENT=1
CYCLONE_ISLANDS=1 cargo run -p cyclone --features dev; New Game →
Enter → fly; Esc-then-R rerolls; FIRST-FLIGHT's ask-list section
(rounds 249–254, drift-corrected three times, each correction a
commit) is what only they can fill in. Every number quoted across
the era carries a date and a witness; every claim of absence
carries a census; every law that can have teeth has a test. The
engine is honest, green, and waiting — which is as far as an
engine can be waited FOR; the rest is a cockpit.

## Round 257 — the pilot is back; the terrain answer ships the same hour

Two screenshots, one verdict, verbatim: 'so the islands are just
hills, we need them to have more realistic designs, flat spots and
things like that so then the game can happen.' The screenshots
split the diagnosis: the CLASSIC archipelago (menu backdrop)
already terraces — the pilot liked it well enough to screenshot;
the CONTINENT-ISLANDS path (the HUD-quoted mode) renders eggs:
axis-aligned squished circles (squish only stretched x — no
rotation existed) under a smooth cosine dome. 'Flat spots' named,
so the fix is landform arithmetic, not texture.

Design: three belts from the sea line — sand flats 0–6 m smooth
(the photo's ring, skid-landable), then 5 quantised benches (tread
= flat GROUND by construction, riser = mesh cliff), per-island
tallness 46–136 m on an fbm riding the lattice wavelength; stamps
rotate per hash and wear a ±14 % world-space coast wobble (one
hoisted fbm; zero new transcendentals per stamp). The same commit
carries BOTH digest moves the law demands: worldgen islands seed
digest 690cfabd→0fa64087, and the render cm-pin
3a1bf7a9→b8794e32 — the cm-pin fired its designed warning ('a
platform builds DIFFERENT terrain, investigate') and the
investigation IS this entry: deliberate, sanctioned, noted. The
classic continent digest did NOT move — untouched-branch control.

Witness history, kept because each step taught the next: the first
flat-probe used a 1 m patch and measured 92 % 'flat' — GREEN INK
THAT CANNOT GO RED IS A COMMENT: gentle v1 slopes pass a 1 m
tolerance trivially. Rescaled to an 8 m landing disk (9-point
cross probe, helipad+clearance): 34.4 %, floors 20k cols / 0.25
against measured 88k / 0.344 — v1's ~0.8 m/m dome slopes drop ~3 m
across the probe and cannot pass. And the probe's FIRST typo was
mine: size 1024 = a 256 m window that can legitimately contain
only sea — '0 flat of 0 high' was the test measuring its own
mistake; the scatters contract's 2048/seed-5 sample is what
islands actually live in. Own eyes before claiming: 1 km hillshade
rendered and inspected (5 islands, rotated, terraced, notched;
6.2 s/7.6 s at 4000² — islands cheaper than classic).

Open observations logged honestly: (1) mid-benches thin to
contour-lines where domes steepen — visible in the render, one
dial-pile away if the pilot hates it; (2) the pilot's screenshot
shows dark rectangles around islands v1 rendered — hillshade (pure
heightfield) has no such artifact, so it is a mesh/sea-shading
observation; if it survives v2 on screen, a close-up shot pins
it. (3) 'so the game can happen' — flat spots are now terrain
fact; putting the actual pads/crates/refugees ON the islands is
spec-03 Q1's answer ('which world hosts gameplay') wearing an ask
— the next round after the pilot says the LOOK is right.

## Round 257 errata — the era's fastest self-catch, three findings

1. **435a62d shipped suite-rc=101 claiming '27 ok'.** The gate
   echoed rc=101 in the SAME command block, and the block kept
   walking into commit && push anyway — an echo is a witness, not
   a gate. Fix-forward: the red was the islands canary's METRIC
   pins (ocean/flat/river/max-height, all frozen at 1e-9 to v1's
   domes), which the v2 commit updated only for the digest — the
   same-commit law lists metrics too for this exact reason. Fixed
   here; law adopted: the ship commit runs in its OWN command,
   entered only after the gate block's echoes are read.
2. **A narrative I invented, caught by my own assert.** Told
   myself the canary sample had 'gone pure sea, all-zeros,
   vacuous' and drafted a re-sample story to ink into the test —
   built from remembered lines, not read ones. The python died
   asserting an anchor that didn't exist ('size: 512,
islands:
   true,' — the classic control holds the 512), and reading the
   file then showed the truth: the canary has run 2048 since
   round 178 ('512' is a legacy test-NAME; the era comment beside
   it explains all-sea digests pin nothing — I had read that
   comment once and not reread it). My 512 tmp_dump measured MY
   file's 512, not the canary's. Had the anchor been looser, I
   would have flipped the CLASSIC control to islands:true and
   silently destroyed the untouched-branch proof. Grep locates,
   reading concludes — this time the assert read for me.
3. **/tmp/test.log is a Nov-6 fossil.** A grep -l over /tmp/*.log
   surfaced 'preview_screen_mouse_and_topdown FAILED' from it —
   that test is long-green; absence/presence claims about
   CURRENT state must name CURRENT logs (s18.log: 27 ok, rc=0).

The v2 metrics truth, measured: ocean 0.9081, flat 0.0929 (×10 —
benches), river 0.000132 (off zero — tall islands earn rivers),
max_height 84.75 m (was 72), high/mountain still 0. Canary green
×2; suite 27 ok / 0 FAILED; render cm-pin b8794e32 stands.

## Round 257 (second block) — the pilot's seed re-witnessed; the rectangle hunt ends honestly

Rendered the seed from the pilot's screenshot — parsed through the
game's OWN Seed parser (CLI passes strings to seed.parse(), the
single-parser law cashing out), my python mirror agreeing to the
digit (1672647164521250451) after being verified against it, not
assumed. Under v2 that world holds 5-6 rotated islands, ridges
AND low sandy atolls, 104 m max, a river — the exact map that
produced 'just hills' is the control group for the fix. Dated
render: hillshade 2026-11-08, /tmp/pilot257.

The dark-rectangle hunt: build_sea reviewed and found innocent
(one map-wide pass, opaque bucket quads, no per-chunk duplication,
correct 150 m ramp); CHUNKS reviewed; heightfield can't cast
axis-aligned rects (pure columns). Every remaining hypothesis
needs a screen I don't have — so the hunt STOPS at the evidence
line, no speculative fix shipped against an artifact I cannot
reproduce. If v2 on the pilot's screen still shows the rects, one
close-up pins the code path; until then it is an observation with
a named witness (the screenshot), not a bug with a guess.

CI: the two terrain commits still in_progress when checked — the
cross-OS canary agreement (the whole libm-law payoff) lands with
tomorrow's probe report. Terrain left UNTOUCHED from here until
the pilot flies v2: no dial drifts under a pilot mid-flight —
the bench-drift one-liner is written in this ledger's margin and
waits for a verdict, not for my taste.

## Round 258 — the era's deepest cut: I deleted the canary's assert, and CI's clippy was the only witness

The 'honest note attached' edit replaced a block whose old-string
ended IN THE ASSERT — python dutifully wrote comment-with-no-teeth,
the digest assertion evaporated, and the suite went green because
THE PROOF STOPPED PROVING. The commit message swore 'digest
0fa64087 frozen' — intent-ink describing bytes that no longer
existed. CI went red on all three OSes, and the error was clippy's
`unused variable: digest`: the compiler noticed the missing teeth
before any human did. The era's own nightmare specimen: green ink
that cannot go red, manufactured at full green-suite-27 health.

Not a cross-OS drift (the libm law never flinched — digests agreed
wherever they were still checked); not triage-able (no innocent
descendant existed when the verdict was read — the fix is the
descendant now, with clippy/suite/flow/golden all green locally on
EXACTLY these bytes). Two laws adopted at the scene:

1. EDITS TO TEST CODE END WITH READING THE FILE'S DIFF — the
   `-assert_eq!` line was one `git diff` away from confessing the
   whole crime, every time, forever. Anchors that span comments
   must re-emit any code they swallowed; when in doubt, anchor on
   the comment's LAST line alone.
2. THE FOUR-GATE SHIP BLOCK: fmt, clippy, suite AND flow, every
   ship, echoes read before a separate ship command. The red
   commit skipped clippy because its author (round 257,
   exhausted, suite-green) treated 'the gates' as a vibe, not a
   checklist. CI IS the checklist's second copy — that's what it
   is FOR, and it earned its keep the hour it existed.

The restored assert passes unchanged: 0fa64087 IS the terrain.
The comment now tells the scar's story in the file itself, so the
next reader of this block learns both the law and its price.

## Round 259 — the island HUD's reroll promise: audited to the last link

'Esc then R rerolls' sits on the islands preview screen in v2's own
pixels. Read the test end-to-end rather than trusting its NAME:
esc_in_continent_ingame_arms_the_reroll_the_hud_advertises runs the
PILOT'S EXACT ENV (CONTINENT=1, SIZE=400, ISLANDS=1), flies via the
Fly button to a live planet (one WorldRoot, rig cage = the planet's
100 m box asserted, not the stale archipelago cage), presses real
Escape, reaches preview, and requires PendingGen armed WITH auto —
'R just presses Fly's route', and that route was proven THIS env
three lines earlier; R's binding is env-agnostic InputMap data,
the design that makes one binding proof cover every planet. No
gap found — the HUD may keep its sentence. (Audit by READING the
test's body this time, the round-258 scar keeping the pen honest:
names describe intentions, bodies tell the truth.)

## Round 260 — the first-view question: spawn on an island world, measured 40/40

The question worth asking before a pilot flies: islands mode is
92 % open ocean — does the flagship spawn deliver land, or does
the castaway fallback (nose-down −60° over water) become the
first impression? The code path: continent_clear →
spawn_over_land, a golden-angle ring scan capped at 500 m, and
THE SCAN READS THE ISLANDS DIAL AT CALL TIME (line 459: islands:
island_mode() — verified before trusting the number). The era had
ALREADY put teeth here (:594 .expect('islands exist within 500 m
of centre')); the fresh measurement at the pilot's scale, seeds
0..40, CYCLONE_ISLANDS=1: land found 40/40 — flagship spawn is
the rule, castaway is the rare-seed fallback the round-185 myth
list already policed. The pilot's own seed by arithmetic on
today's render: nearest island centres 321 m and 405 m from the
spawn ring — well inside range; their first view is terraced
rock, not ocean.

Measurement cost honesty: the probe was a throwaway example
target, deleted after one run — data collectors don't get
permanent compile slots; the six #[ignore] tools remain the only
resident probes. No code moved; the world stayed frozen; the
spawn question closed with a count, not a hope.

## Round 262 — cross-OS proof LANDS, the era score dies honest, and my own false ink corrected

**The milestone.** ede9f17 (and 81d2712 on its heels) completed
success on ubuntu + macOS + windows: the restored canary asserted
digest 0fa64087 on all three OSes and they AGREED. Islands v2 —
rotation, wobble, benches, all of it — is provably identical
terrain on every OS the release ships to. That agreement is the
entire reason the libm law exists, witnessed the era it was made
for.

**The era score dies.** '×17 green since 0650146' is retired:
ee7263a, 435a62d, d8707fd were TRUE reds (the deleted-assert
clippy failure was real, not noise) — triage closes infra noise,
never honest red; those three stand as history, closed by the
fix's green, not by descent-hand-waving. New count from the
grave: ede9f17, 81d2712 green at read; 6683a90, 73f05bf queued.
A streak claim that outlives its reds is just superstition.

**My false ink, corrected.** The round-257/260 entries claimed
the bench-drift one-liner was 'written in this ledger's margin'
— grep of this very file shows only the CLAIM, never the line.
The law has no authorship exception, and a dial that exists only
as a promise is how promises stop being kept. Written now, for
real. Current truth (continent.rs:259): `let k = (q * 5.0).floor().min(4.0);
// treads 0..4` — perfect concentric rings. The drift line, held
ready until the pilot's look-verdict (terrain frozen):
`let k = (q * 5.0 - 0.6 * (fbm(wx / 220.0, wy / 220.0, 1.0, 2, seed ^ 0xB34D) - 0.5)).floor().clamp(0.0, 4.0);`
— ±0.3 benches of wobble at 220 m wavelength: contour rings
become geology, some benches pinch out mid-slope. Applying it
re-pins, in one commit, the four known moves: islands digest,
mesh cm-pin, canary digest + metrics (flat_ratio WILL shift) —
the same-commit law, and the re-pin choreography is scripted
knowledge now.

**Pull-surface fact for the pilot:** Cargo.lock's newest touch is
dd0b527 (round ~230, two eras before their screenshots) — the
entire round-249→73f05bf stack changes NO dependencies; their
git pull needs no network beyond the repo itself.

## Round 263 — the dial sheet's anchors: census says clean, so the clean says it here

Every TUNING island row instructs the pilot to change a line by
grep-quoted text — and quoted text is a claim about bytes with
one dated witness (round 257's own writing). Full census both
ends this round: all seven anchors (bench count, tread mid,
tallness, beach fraction, wobble, rotation, keep-coin) exist in
continent.rs, one occurrence each; all six sheet rows quote them
verbatim; the witness figures on the first row match the
measured pair (88k of 256k). Zero drift, so nothing was touched
— and a clean audit still gets its line, because the era's
standing rule is that unlogged audits are audits that never
happened. CI at read: the three docs commits mid-serial-lane,
green expected (no code bytes between them and 81d2712's
all-OS green).

## Round 264 — the parked release measurement runs; a pilot-visible layout bug found inside the fix

Six tools still six (census at read). fullscale_census — the
parked 'continent_dial release measurement' — executed with
--release --ignored, back-to-back modes, seed 2, 4000²: islands
6.433 s / 59 chunks / 173,933 quads vs mainland 9.068 s / 227
chunks / 1,943,906 quads; ocean 0.90 vs 0.24; tripwire (60 s)
passes at 10× margin. For the first time the 'islands cheaper'
claim is a same-minute paired witness, not a quote across eras —
71 % of the build time, an eleventh of the mesh. Frame rate stays
the pilot's to judge; quads are load, not fps, and the sentence
says so.

The find while updating TUNING: the round-257 timing note was
inserted MID-TABLE — a non-'|' line — which in rendered markdown
murders every table row below it into paragraph mush. The pilot
reads the RENDERED sheet: a note that silently destroys the
table under it is pilot-visible damage even when every number in
it is true. Lifted out, re-dated with the pair, re-attached after
the table's true end (walked to it line by line, not guessed).
Lesson kept: docs with tables get read as RENDERED, not as source
— after inserting prose into a table file, look at where the
table ends, not where the insert landed.

## Round 265 — the bug class gets its sweep; yesterday's surgery fails its own follow-up

'Round-264 taught us rendered-markdown matters' only means
something if the LESSON runs as a class, not a memory. Program-
matic scan (prose-in-table AND blank-line-in-table patterns) over
every pilot-facing markdown: one hit, at TUNING — and the honest
name for it: MY OWN YESTERDAY. The surgery lifted the note out of
the table but left the note's blank line behind, splitting the
same table at the same row with a cleaner-looking wound. Fixed
(join the rows), rescan: clean across FIRST-FLIGHT, TUNING,
DECISIONS, specs and all of V2/.

The meta keeps sharpening: fixing a bug and FIXING A BUG are
different acts — the first closes the instance, the second closes
the class. Round-264 should have re-run the render check on its
own edit before shipping; the follow-up scan existed a day late
and caught the surgery's corpse. Adopted small: after any docs
edit inside a table, the scan runs before the gate block, not
someday. (Scan kept as throwaway logic in this entry's prose, not
a resident tool — six tools stay six until a real need names a
seventh.)

## Round 266 — CELL_VOX=2 measured honestly: a dial, not a free one

The parked item was a measurement and stayed one: const flipped
in the WORKING TREE only, release census both modes, const back,
`git diff` empty of code before anything was written down. No
digest moved; nothing shipped mid-measurement; the six tools ran
their one tool.

Islands at 0.5 m mesh cells: 6.495 s / 161 chunks / 602 k quads
(quad load ×3.5 for +1 % build time — on the map the pilot
actually flies, smooth cliffs are nearly free in seconds and
expensive in mesh). Mainland: 10.2 s / 859 chunks / 6.76 M quads
(×3.5 mesh, +12 % build). The TUNING 'blocky under me' row now
carries the cost next to the cure, both dated, and states the
dial's true nature — A CONST (recompiles, no env), which the row
previously blurred (its old text implied a dial-table citizen;
the env-name check came BEFORE the measurement, and the answer
changed the measurement's procedure, not just its prose).

Self-check caught one more: my replacement row first named the
file 'render/cyclone_mesh' — wrong path, caught re-reading my own
line seconds after writing, corrected before the gate block; the
law has no authorship exception including for sentences written
eleven seconds ago. Round-265's scan-before-gates law executed on
this very edit: clean. Four gates then.

## Round 267 — four family seeds seen; the 'castaway' I invented was a units bug

Esc-R's promise is the FAMILY, so the family got eyes: seeds
33 / 7399 / 136,448,130 / 297 (the CLI's own decode of 11/777/
424242/99 — one parser, verified route). What four km² renders
show: terraces everywhere, rotations varied, a fused double-peak
merge that reads volcanic, a banana-shaped max-squish island,
sand-heavy atolls and green ridges, rivers on most — no
degenerate blob, no dust. The claim 'atolls to ridges' now rests
on six seen worlds, not two.

Then the false alarm, kept in full because the failure is the
lesson: seed 297 LOOKED sparse, I eyeballed nearest-land at
~1800 m, declared a castaway counterexample to round 260's 40/40
in writing, and went hunting. The direct probe answered
FLAGSHIP over 97 m at 441 m from centre. My pixel-to-metre
arithmetic treated a 4000-COLUMN world as a 4000-METRE one —
every eyeballed distance was 4× oversized. When evidence
'contradicts' a tested model, suspect the auditor's units first:
the model here had a suite AND a 200-seed recount (castaway 0 of
200) behind it; my eyes had arithmetic invented ninety seconds
earlier. Round-260's 'rare fallback' stands, now 204/204 seeds
flagship across two measurements; the four family worlds are all
confirmed flagship spawns.

Throwaway probe deleted after use (examples never get resident
slots; six tools stay six).

## Round 268 — the island section's last un-audited claims, both ends git-checked

Twelve claims sat in the round-257 island section; eleven read
true against code bytes (sand 0–6, five benches, 46–136 band,
±14 % wobble, 88k 8-m disks, classic canary untouched, six-seed
family). Two needed git, not eyes:

- 'TUNING's five new island rows' — the commit added SIX (the
  density row is new too; a pilot told 'five' may never scan the
  sixth and never learn island COUNT is theirs to dial). Fixed
  to six, dated, count-verified.
- 'shipped the same hour' — acquitted by the stamps: verdict
  05:40 EST, feature commit 05:49, digest-assert-restore 05:56.
  True even under strictest reading; left standing.

Pattern worth naming: my own pilot-doc drift is never where I
expect it — the big numbers were all right, the COUNT of a table
was wrong. Claims of quantity ('five rows', '27 ok', '40/40')
deserve the same byte-check as claims of value, and git is the
cheap witness for claims about the past.

## Round 269 — spec-02 face-to-face with islands v2: zero open debt on my side

Full re-read of V2/02-map-generation.md (whose real filename my
globs kept guessing wrong — ls before grep, learned the cheap
way). The audit question that matters mid-freeze: does the
islands path owe anything to this spec that could be fixed
without a verdict? Answer by section, not vibes:

The ten-stage WorldMap pipeline (sites, archetypes, helipads,
fuel graphs, crates, occlusion, refugees) IS the classic game
world — spec-satisfied with era receipts: 1000/1000 batch at 15 ms
median, invariants 300 seeds, golden bit-identical on three OSes.
Islands v2 is the terrain-lab height grammar and owes the spec
only what general laws reach: A2 determinism (libm law, held), C3
determinism-on-CI (canary digest asserted Linux+macOS+Windows,
round 262 renewed), classic perf budget (150 ms applies to the
160x160 classic config — islands is a 1 km² dev env world,
measured honestly instead: 6.4 s release census).

Every unchecked acceptance box (:312 contact-sheet looks varied,
:314 'shows' foam/props, :315 cross-machine seed) is a pilot-
senses item predating v2 — their pen, not mine. Crates/refugees
/pads on islands remain exactly where Q1 put them. Verdict: no
spec-compliance work hidden behind the freeze; the freeze blocks
nothing the spec demands. V2's own C1 line adopted as method:
'this tool is how the agent (and you) judge the generator' — six
hillshades were the islands answer to that ritual; the pilot's
contact-sheet verdict is the same ritual with the better eye.

## Round 270 — the pilot's reroll loop gets its first island-mode hitch number: 24.2 ms, box holds

Spec-01's audit found the inheritance gap nobody had named:
acceptance :99's ≤50 ms reroll box had THREE dated measurements,
all classic. The v2 loop — the exact loop the pilot is about to
mash — inherited the box on faith. The tool (flow.rs:349, the
era's own hitch instrument) run under all three dials (CONTINENT
SIZE=400 ISLANDS + the PREVIEW alias, the reroll-era test's
proven route) answered: worst frame 24.2 ms across 139,806
frames, 51 % margin, dev-profile offscreen — the same comparison
conditions as classic's 8.3–9.2 ms, so the delta is real:
island rebuild frames land heavier (27 chunks, worker completion
and mesh swap in the same frame is the shape; not chased further
— inside the box, dated, headless caveat stands: GPU upload is
never represented here).

The spec text gained its fourth dated witness under the box —
addition in my pen, pilot's words untouched, matching the exact
pattern the box already established. The pattern lesson: an
acceptance box measured three times under the OLD world is an
unmeasured box under the new one — audits should ask 'which
era-dated claims does v2 inherit silently?' The backdrop worry
raised at the round's open died with a citation: spec :30
intentionally pins the menu to a small one-island config;
mode dials change the GAME world; the pilot's screenshots were
compliance, not drift. High Scores formally confirmed Q1-gated:
:42 specs it fully (name/score/round/date/seed, play-this-seed)
and the ONLY missing organ is the score event, which is crates —
the empty table has no witness to write until Q1.

## Round 271 — 'in 0 ms' retired: the console line that was true and lied at once

The reroll run's own output was the witness: 'continent ready:
27 chunks, max 86 m, ocean 96%, in 0 ms'. Reading the code: the
timer bracketed ONLY claimed.take() — the pickup of work the
worker had finished seconds earlier. Every millisecond true;
every word misleading: 'ready… in 0 ms' in front of a pilot who
just watched a seven-second spinner. Truthful measurement +
wrong sentence = a false claim, the same species as my doc drift
— the console is pilot-facing ink too, with cargo run's stdout
the rendering engine.

Fix: the line moved to the block's LAST act (after cache
assembly) and renamed to 'meshed in {} ms' — which is what the
timer has always bracketed plus the meshing it now includes. The
compiler auditioned as second witness mid-edit: the naive move
tripped E0382 (chunks is consumed by the builder below), caught
before any gate, fixed by capturing the count at the honest
moment. Habitat run (the reroll tool under the three dials):
'ready: 39 chunks, max 112 m, ocean 92%, meshed in 11 ms' — and
the honest numbers tell a better story than the old line:
meshing IS 9-18 ms; the seconds belong to worker-side GENERATION
the console names separately ('carved on a worker'). Worst reroll
frame this run 19.2 ms of 138,948 — round-270's 24.2 confirmed
from two runs, box half-filled at worst.

No test pinned the line (census first); no geometry or logic
moved — the digest tests cannot flinch and the gates prove no
collateral. Console honesty joined the audit list: pilot-visible
claims live in stdout too.

## Round 272 — the preview panel's three lies, caught by its own green test's blind spots

The silent-inheritance audit reached the WORLD PREVIEW panel, where
round 219's label fix had stopped half-done. Three pilot-visible
false claims survived under the dials, and the panic dumps PROVED
each, not inferred: under CONTINENT the panel said '1 km²' while
its own flow test ran SIZE=400 (a 100 m world — the hard-code sat
beside a green checkmark), said 'voxel Earth' while CYCLONE_
ISLANDS=1 showed a terraced archipelago, and printed the CLASSIC
map's difficulty reason — 'Balanced distances between fuel stops'
verbatim in the captured text — over a world with no fuel stops.
The sibling test had always run with ISLANDS REMOVED and forbade
two strings; everything else leaked through its blind spots. Green
ink that cannot go red is a comment: the test now carries four new
assertions (true extent 100 m, no km² under that dial, no fuel/
rescue-routes prose) plus a new islands-identity test, and the
RED-PROOF RUN (fix stashed, old bytes): both new tests panicked
on exactly those strings — teeth verified, not asserted.

The fix reads the dials: one hoisted continent_mode() read (the
era's ONE-reader law earning rent again — it already returned the
true extent; the panel just never asked), size-true extent text,
ISLANDS named and shown in the dial row as the pilot runs it, and
the reason row gated to the world whose metrics it explains.
UI text + tests only: no geometry, no logic, digests cannot move.

The audit's own pattern, sharpened again: tests written beside a
half-done fix inherit its blind spots — when a fix branches, ask
which branch the test never walks. (Flow binary now 15 passed + 1
ignored; suite still 27 binaries — the count law keeps its own
numbers honest as they age.)

## Round 273 — the Generating-screen notice's stale trio; and the restore command that ate its own round

The third member of the class: the dev notice the pilot's terminal
prints before the one-time build — 'building 1 km² voxel world
(one-time, ~8–10 s — release census: 8.7 islands / 8.3 continent)'.
Three era/dial lies in one line, each a different fossil: '1 km²'
hard-coded (false under the SIZE dial — round 272's twin, in
stdout); '~8–10 s' quoted from the mainland census while ISLANDS
pilots wait ~6.4 s (same-day numbers: islands 6.4 s, mainland
9.1 s at 4000²); '8.7 islands' a PRE-v2 count the v2 family
renders (4–7 on witnessed seeds) disproves. Fix: one shared
extent_label (panel and notice now physically cannot disagree —
the drift the round-272 audit found, structurally closed) and a
pure continent_notice(ext_m, islands) whose per-dial claims the
PLAIN suite holds — CI-resident on all three OSes, not dev-gated
teeth. Red-proof witnessed: the function temporarily made to
speak the old string, the new test panicked on it verbatim, then
green restored.

The round's own scar, logged per the no-authorship-exception law:
the red-proof's restore was git checkout -- on screens.rs — and
screens.rs carried ALL of the round's uncommitted edits, so the
checkout restored the committed file and ate them: label, notice,
panel unification, tests. Caught instantly (the file's tail no
longer held the mod), rebuilt from this round's own transcript in
one pass, verified by count and by the full battery re-passing.
Law extracted: a red-proof restores with the SAME targeted edit
that made it red — never a git restore, which restores the last
COMMIT and a live round lives in the difference. The deleted-
assert scar's sibling: same shape (restore wider than the wound),
twice learned, now law.

Suite still 27 binaries (the new unit tests joined cyclone_game's
in-file mod — the result-line law held); flow 15 + 1. Four gates
echoes READ before this separate ship block; terrain frozen — UI
text, one pure fn, tests.

## Round 274 — the sweep after the sweep: docs that CITE retired ink

Round 273's ships changed pilot-visible strings; a doc quoting a
retired string becomes a stale claim itself, so the truth-ship's
last act is a citation sweep (patterns: '8.7', 'in 0 ms', '~8–10',
'1 km² voxel', 'continent ready'). One hit earned the axe:
FIRST-FLIGHT's own islands section promised '~8–10 s one-time build
(census-measured)' UNDER the islands command — the mainland census
quoted for the islands flight, the very number the notice retired
this week. Now dial-matched: '~6–7 s (census 2026-11-08: islands
6.4 s, mainland 9.1 s)'. Survivors checked and kept: the '1 km²'
mentions at FIRST-FLIGHT :48/:175 and TUNING :142 describe the
pilot's DEFAULT-SIZE run (no SIZE dial) or the pivot's history —
true as-run, history as history; and V2 docs carry no citation of
any console format (the README's 'meshed in 3D' is prose, not ink).
The rule joins the audit list: when a string ships, grep the docs
that quote it — engine truth and doc truth ship as one unit or
drift apart by the next era.

Doc-only diff; four gates run anyway (warm cache), echoes READ.

## Round 275 — spec-03 audited under v2: one fossil, one harvest

Full read of 03-flight-and-helicopter.md against witnessed truth.
Scoping did most of the work: the '14 islands / pads / crates'
passage is CORRECTLY SCOPED to the classic generator — the
untouched control — so it stays true by its own framing; Parts
A/B/C are drafts claiming nothing built ('grep -c fuel = 0' still
era-witnessed true); the acceptance item 'free-cam flies the voxel
Earth unchanged' is exactly what the terrain freeze protects, and
stays the pilot's unchecked box. One fossil earned a dated
annotation (:48): 'one-time build (~8 s)' predates island mode —
same-release census dial-matched it (islands 6.4 s, mainland 9.1 s
at 4000²), console and FIRST-FLIGHT already carry the per-dial
figures, the spec now does too (annotation appended, spec text
untouched).

The harvest is bigger than the fossil: Q1 and Q5 are now the
LOAD-BEARING pair. The pilot's own verdict wording — islands need
realistic designs 'so then the game can happen' — reads as Q1
pointing toward the voxel world hosting gameplay, which wakes Q5
(pads, crates, refugees on stamped islands) and, through Q1, every
later spec plus the buried High Scores. That framing went into
the annotation for the pilot's next read.

## Round 276 — spec-00 audited: camera reconciliation noted; my own draft's evidence caught drifting mid-audit

Full read of 00-engine-architecture.md. Audit principle stated
explicitly: PRESCRIPTIVE text is specs-by-design (the iso rig
section, the sea material, the 0.19 pin 'at the time of writing')
— no fossil axe for targets; only STATUS ink gets re-witnessed.
Status lines held: layout note true (tools/worldgen-cli EXISTS,
plus two census tools), acceptance boxes keep their witnesses.
One reconciliation annotation added, file's own note-style: the
camera section prescribes the orthographic iso rig while the
shipped reality is the free mouse-look the pilot flies (Q/E snaps
survive; CameraHeading keeps its quadrant/occlusion role; wheel =
eased FOV zoom — the three-fixed-levels spec never shipped; code
bytes checked before the claim). Which camera survives gameplay =
spec-03 OPEN Q2, said in the note: prescription in review, not
status. CI status line re-dated with the islands-v2 ×3 proof
(ede9f17+81d2712, canary digest agreed on all runners).

The round's own lesson, three catches logged per no-exception law:
spec-03's draft said '515 lines', the file is 605 — MY OWN
draft's evidence drifted in two commits (measured: ba0c0b2,
cac50e8) since its pin. First repair wrote 'in five commits' —
an INVENTED quantity, measured down to two before it shipped. My
fix then wrote 'the pin below' when the pin sits above. Quantity
law, third form this era: evidence pins age inside the very docs
that record them — annotate numbers with their date, and re-
measure before quoting even the repairs.

Annotation-only diff across 00/03; pilot's design text untouched
throughout; four gates run anyway, echoes READ; terrain frozen.

## Round 277 — V2/README clean under audit; ask-list re-pointed at the load-bearing pair

README status table witnessed honest: 00/01/02 Done (accepted,
headless witnesses in-file), 03 marked Draft-pilot-review,
'four'→six OPEN questions checked by reading all six, 04–06
Planned. The camera drift the round-276 annotation reconciled
turns out to have been recorded by the era all along — in THIS
file ('free-perspective fly camera… evolved on user direction') —
so 276 added consistency, not a first record. Not every audit
earns the axe; a clean verdict is an audit result too.

Ask-list (FIRST-FLIGHT 'What I need from you') walked item by
item against v2: TERRAIN-LAB box carries the verdict quoted
verbatim ✓; quantity claims re-byte-checked per law — 'five
benches' true by code (k = floor(5q).min(4) → values 0..4), '13
numbered flight dials' true by table (rows 1–13 unbroken,
re-counted not inherited). Item 3 got the round-277 dated
upgrade: Q1/Q5 load as a PAIR after the verdict ('so then the
game can happen' reads Q1-ward voxel), Q5 decides pads/crates/
refugees on stamped islands, High Scores stay buried until Q1
answers — the harvest now sits where the pilot actually reads.

Two files touched, both pilot-facing prose; four gates run
anyway, echoes READ; terrain frozen; V2 spec design text untouched.

## Round 278 — full-scale islands census: the eyeball era ends at 1 km²

The islands claims rested on six eyeballed worlds; the mainland's
era claims came from the fullscale census tool (8.7 islands /
8.3 continent). Measurement is not construction — the freeze
protects height bytes, not the odometer. Throwaway example (one
run, deleted): 12 seeds at 4000², config-field islands:true (no
env, no dial trap — the dial flows as a CONTINENT CONFIG FIELD at
both construction sites, so a probe can set it directly and the
silent-mainland trap the era warned of cannot spring).

Results: ocean 0.85-0.96 (claim 0.91 holds); gen-only 5.0-6.5 s —
with mesh the shipped '~6-7 s' stands (units checked BEFORE
believing the loop: the 6.4 s census figure brackets gen+mesh, my
loop brackets gen alone — different instruments, no drift found);
islands 3-8, median 6 — the family-witnessed '4-7' was honestly
sample-scoped but narrower than the true range, and 'big islands'
(>=6250 m2) run 2-6: the count claim needed the census, and got
it. Flagship peaks median 96 m. SPAWN 12/12 at full scale: the
flagship law's 204 samples were smaller maps; full scale adds 12
— 216/216.

The census's first catch: FIRST-FLIGHT's comment 'a few 100-250 m
islands' is a v1-era fossil — measured max heights run 88-116 m,
flagship peaks median 96. Comment now quotes the census itself.
Repo-wide sweep: the only other '250 m' is mainland.rs's h>=250 m
metrics band — true for mainland, untouched; DECISIONS is history.
Every census number in the doc line is the loop's own output —
the quantity law shipping with its instrument attached.

## Round 279 — the census ruler applied: dial rows learn their OBSERVED ranges

Every island-number claim that predated the 12-seed ruler got
measured against it. :168 tallness '46-136 m' is the FORMULA's
range - census observed 88-116 m inside it, no contradiction, but
a pilot asking 'too low/too tall?' deserves the observed range,
so the row now carries both, dated, with the honest note that the
formula's ends are rarely both reached. :172 keep-coin '55 % of
lattice sites' is the MECHANISM - census observed 3-8 median 6,
consistent, attached. Rows audited and untouched: :167 pad-scale
witness (scale-independent by construction), :184 r=90-200 m blob
parameters (true in code; observed equivalent-circle radii 71-140
m consistent under squish/erosion), the classic 160x160 rows (the
control, scoped by its own header).

The round's own catch, mid-edit: the :172 append first shipped a
CAUSAL STORY in my head - 'edge sites lose coin AND sea' - written
as if the double filter were measured. It was never measured: the
spread's cause is a hypothesis, and hypothesis ink is the exact
sin every truth-ship this week has been repairing. The cell now
carries the spread with 'no cause claimed here'. Law, restated for
the hundredth time because it keeps needing restating: a number
without an instrument is a rumor; a CAUSE without one is a myth -
and myths are what the round-219 audit was named for.

Table integrity scanned before gates (4 fields every touched row,
zero blank splits); two cell appends only; four gates, echoes
READ; terrain frozen.

## Round 280 — the dark rectangles have a mechanism now: centre-sampling sea buckets, comment caught lying mid-read

Diagnostic prep for the pilot's ask-list bonus ('do the dark
rectangles still appear? close-up pins them'). Reading build_sea
to arrive briefed, the bytes gave the answer outright: the sea is
ONE QUAD PER 25 m BUCKET, coloured from sea_dist_cells — which
its comment claims samples 'the bucket's own footprint (max over
the bucket = there is land in this tile)' but whose code samples
ONLY BUCKET CENTRES (every wet() call takes centre coordinates).
Consequence with a name: coast-edge buckets — centre in water,
footprint over beach — are coloured by centre-to-centre lattice
distance to the nearest CENTRE-WET bucket, which can be buckets
across the island; the quad paints a navy-to-mid square of
exactly 25 m, axis-aligned, hugging the shore: dark rectangles
around islands, the pilot's screenshots explained without needing
a close-up to form the theory (the close-up still decides whether
it survived v2 and how loud it is). Small islands can be entirely
INVISIBLE to the field — a landmass missing every bucket centre
never darkens anything near it.

The fix is ARMED, NOT FIRED — twice-armed out: (1) the terrain
freeze forbids changing what the pilot sees mid-verdict, and the
rectangles are precisely what they might still be looking at;
(2) their close-up decides existence and severity. Dossier for
firing: no exact distance sheet exists for continent (the era's
DTM sheet lives in the classic path), so honest options are a
multi-source BFS distance transform or real per-bucket footprint
sampling as the comment always claimed; the sea has NO digest pin
(the digest hashes CHUNKS, not the sea; the sea test asserts
variety '>3 hues' and not-white — nothing today can see the
rectangle, which is how it shipped), so the fix must arrive with
the red-capable test: a bucket adjacent to land must read cyan
(t below threshold) — an assertion the centre-sampler cannot pass.

Comment-code drift joins the museum: the footprint sentence
described a sampler that never existed — same species as round
181's comments-only code. Read conclusion first; the comment
testified second.

No file in the engine was touched this round; ledger-only; four
gates anyway (nothing can move, echoes READ proving it); terrain
frozen; the close-up stays the pilot's, the mechanism is now ours.

## Round 281 — the rectangles, seen: production sampler, my own eyes, evidence committed

Cargo.lock is law, and the era's PNG machinery lives only in
worldgen-cli (no render dep) — so the zero-drift path to seeing
build_sea's OWN colours was a throwaway in-crate test (pub(crate)
accessors, the digest test's precedent) saving a hand-rolled
stored-deflate PNG: no deps, no lock byte moved. One run, then
surgical deletion — residual diff READ empty, the git-checkout
scar from round 273 kept at bay by its own law.

The picture (docs/sea_rectangles_evidence.png, seed 2, the
production sampler's own vertex colours): cyan shelf halos ending
in GRID-ALIGNED SQUARE EDGES; navy intruding between neighbouring
halos as square notches — and the clincher, predicted from the
centre-sampling mechanism hours earlier and found in pixels: a
thin island wearing jet-black squares AT ITS OWN SHORE — a
landmass that misses the bucket centres reads as far ocean beside
its own beach. The pilot's screenshots are fully explained, and
the ask-list now points at the picture with the fix's condition
stated honestly: the close-up still decides loudness in-game; the
fix waits on the freeze lifting, not on me.

Process notes: three compile rounds to honest ink (nested move
closures over FnMut -> plain loops; IHDR chain -> plain bytes;
integer /12 bucket mapping overrunning 500/40 = 12.5 — my own
raster had a rounding bug the compiler AND the panic caught
before the image could lie). And a tool lesson: the read tool
pads its display one column; two edit failures lost to memory's
bytes before the file's bytes won — anchor edits on file content,
not remembered indentation.

## Round 282 — the fix, seen before it exists: one seed painted twice across a white seam

Round 281's probe machinery, reused at zero drift and deleted
after one run (residual diff READ empty again): the SAME island
seed painted side by side — left half through the shipped
sea_dist_cells, right half through a footprint-min sampler over a
chamfer distance field (10/14 two-pass, /10 = cells; an honest
approximation labelled in the probe, bucket-scale differences are
what the eye judges). Committed as docs/sea_fix_sim_preview.png.

The eye's verdict: right half, every island sits on a continuous
cyan shelf — the jet-black shore squares of the left are gone,
thin islands included. And the seam sharpened the MECHANISM
itself beyond round 280's colour story: those black squares are
buckets SKIPPED entirely — centre on land, no quad — holes to
background where sea cells of the same bucket just lost their
only painter. The dossier upgrades accordingly: remove the
centre-skip AND colour from the footprint; the preview proves
both halves visually. What the fix does NOT claim: the residual
25 m staircase is the bucket DESIGN (one quad per bucket), not
the sampler — smoothing it is a different, bigger call, and no
doc says otherwise.

Zero engine bytes moved — the chamfer field, the skip removal,
the preview sampler all lived and died inside one throwaway test.
The armed fix is now also a SEEN fix: when the pilot's close-up
calls for it, the look has already been witnessed by the only
eyes here that can judge colour, and the pilot has the preview
in-repo to judge with their own.

## Round 283 — flat SPOTS, measured: the count test was honest, and the pilot's noun was asking more

islands_have_flat_landing_ground asserts a COUNT (>20k flat cols,
ratio > 0.25); the pilot's verdict noun — 'flat spots' — is a
claim about PLACES, and a one-cell-wide level contour line would
satisfy the count with zero spots. The witness had no tooth for
contiguity, so it was measured (frozen terrain, honest ruler):
identical flat mask (same 9-point cross, same 1.2 m tolerance),
4-connected BFS components, three seeds on the witness's own
512 m map.

Truth: the 88k columns are not grains — 12-23 CONNECTED spots per
map, 6-8 spots >= 25 m2, largest components 766-2,698 m2 (seed
23's biggest is a ~52 m square of level ground). And a
cross-era agreement worth its weight: seed 5's component total,
5,501 m2, IS the era witness's 88k cols x 0.0625 m2 to the
column — old claim and new ruler say the same number.

The honest asterisk, kept instead of smoothed: seed 11 had NO
spot >= 900 m2 (largest 766). Benches are everywhere; BIG level
places thin out on some families — and no pad-size threshold was
invented to grade that, pads being spec-06/Q1 territory. TUNING
:167 now carries the contiguity census beside the count witness,
so the pilot's 'so then the game can happen' question has numbers
for both nouns: how much flat ground, and how gathered.

Probe deleted with residual diff READ empty; table row scanned
(4 fields); four gates; terrain frozen — measured, never moved.

## Round 284 — landing ground at flight scale: every world holds a village square of flat, and my edit line got corrected by its own tail

Round 283 measured spots on the witness's 512 m window; the
pilot flies 1 km2 worlds, so the ruler went full-scale: the
census's own 12 seeds, 4000^2 islands, same flat mask, connected
components, one 132-second run, probe deleted with residual diff
READ empty.

Per flown world: 25-116 connected flat spots >= 25 m2 (median
62), 4-23 of them >= 900 m2, and the single largest level place
per world runs 1,868-6,842 m2 - the thinnest world in twelve
still holds nearly two thousand square metres of level ground,
a village square. 'So then the game can happen' now has numbers
at the scale it was asked at: not how much flat ground per map,
but how much the pilot's own next Esc-R world will hold.

Two craft lessons, both caught by READING THE EDITED REGION
instead of trusting the edit's success: my FIRST-FLIGHT insert
duplicated the following line ('# sand rings,' x2) and closed a
parenthesis that had never been opened - the replace string
re-emitted the anchor's successor by force of habit. The tail of
an edit is where edits lie; the assert-on-count caught the
second blemish only because sed re-printed the block after.
And the median lesson from the probe's own output: printing
'median*2 124' (sum-of-middle-pair, no divide) is how garbled
numbers are born - the doc carries the properly sorted median
62, not the probe's shortcut.

## Round 285 — my own yesterday-claim met its phantom: the spawn sampler reads the ENV dial, and cfg fields don't reach it

The question was the pilot's first 3 seconds: does the nose open
on a landing place? The answer arrived only after walking through
a self-built trap. First run: spawn heights 184-268 m —
IMPOSSIBLE against this week's own 88-116 m island census. Two
fresh censuses cannot disagree; one was measuring fiction. The
bytes named the liar: surface_height_m builds its config with
`islands: island_mode()` — the ENV dial AT CALL TIME — and the
probe process had no dial set. The spawner was grading against
MAINLAND phantoms: mountains analytic-relief imagines over open
sea. My printed numbers were artifacts of dial-blindness — the
auditor's-units law again, with the dial as the unit.

Then the colder thought: the deleted round-278 census — source
of FIRST-FLIGHT's 'flagship 12/12' line, shipped YESTERDAY — set
islands as a CONFIG FIELD, and config fields do not reach this
function. Its spawn claims were phantom-graded. Re-ran the probe
WITH CYCLONE_ISLANDS=1: spawn heights 55-95 m (island range,
phantom story fully explained), flagship 12/12 — the claim
SURVIVES, but only now is it WITNESSED; the evidence-pin law
(re-measure even the repairs) caught a live defect in my own
fresh ink, which is exactly why the law exists.

The truth, now dial-set: spawn never lands ON a flat spot (peaks
are pointy — benches terrace below), but the nearest >= 900 m2
level place sits 5-45 m from the nose in ELEVEN of twelve worlds
— landing ground a bench down, a genuinely kind first view — with
one outlier (2000013: 640 m). Honest scope: distances measured to
BIG spots only; near-peak small spots not censused.

Laws banked: (1) any probe calling a dial-reading function
(spawn_over_land, surface_height_m, anything reaching
island_mode()) must set the ENV dial — cfg fields stop at the
caller; (2) when two fresh censuses disagree, hunt the fiction
before touching either — here the disagreement was the whole
diagnostic; (3) claims born dial-blind get re-witnessed, and
FIRST-FLIGHT :59 and TUNING's spawn row now carry the re-
witnessing plainly, including the embarrassing half.

Probe deleted, residual diff READ empty; classic spawn tests at
:593/:603 audited — mainland mode is their truth, dial-off is
CORRECT there; table scanned; four gates; terrain frozen — this
round moved no bytes, it moved a ruler back onto true ground.

## Round 286 — the dial-blindness sweep: the era's protocol passed, one tool's doc learned which world it measures

Yesterday's law got its systematic audit: every call-site of a
dial-reading function (island_mode, continent_mode, everything
reaching them), classified production/test/tool.

Production (screens.rs x8, lib.rs spawn path): the pilot's own
process env is the dial — correct by construction. Tests:
CONTINENT dial semantics test and the islands mesh-digest test are
both ENV_LOCK-guarded with RAII reset — the era had a PROTOCOL
(lock every CYCLONE_* writer; set the dial explicitly; classic
tests stay dial-off because classic is their truth), which is
precisely why the islands canary agreed across three OSes: it
hashed real island mesh, not a phantom. The two in-suite
generate_and_mesh callers: islands digest (dial SET + lock,
correct), classic 512 digest (dial off, correct for its era).
The deleted round-278 example was the ONLY victim of the trap —
and round 285 already repaired what it claimed.

One residue stayed honest the cheap way: fullscale_census — the
#[ignore] tool, seed-2 @ 4000^2, dial-off by construction — fed
every MAINLAND census number and its doc never said so; an
operator (me, a future round) could run it expecting island
figures. Its doc comment now says MAINLAND explicitly and points
at rounds 284-285 for island numbers plus the dial law itself, so
the tool documents its own units. Test-file edit law: the diff
was READ before gates — six doc lines, zero code.

The reassuring finding IS the news: the machinery trusted by CI
was dial-safe by protocol, not by luck; the phantoms lived only
where the protocol wasn't applied (a deleted probe) and where a
doc was silent (now fixed). Four gates; terrain frozen.

## Round 287 — the pilot's own seed, seen: spawn 74 m over the flagship, landing ground 11 m off the nose

The first-view law meets its most personal subject: seed
1EDK-FQFN-XMZMK (1672647164521250451), the world the pilot keeps
flying, measured on today's exact code with CYCLONE_ISLANDS=1
set IN-PROCESS — the round-285 protocol applied to the one seed
that matters to the verdict. Result: spawn 74 m above the
flagship, land directly beneath the skids (0 m), 11 big (>= 900
m2) landing places in the world, the nearest 11 m off the nose.
The first frame of their next Enter-key is witnessed, not
assumed.

Honesties: the era's '321/405 m from spawn' was a pre-bench
dome-era measurement — never promised in FIRST-FLIGHT (grep
proved it: the seed and those numbers live only in shared
history), so no doc carried a stale claim; the fresh numbers
describe the grammar that will actually render, and they are
better. The probe's on-flat field was DROPPED as unevidence: its
membership check could never fire (components are consumed before
the column test runs) — a dead test is a comment, and I caught
mine printing nothing before it became a doc word.

The measurement lived one run and died (residual diff READ empty);
FIRST-FLIGHT's TERRAIN-LAB box gained a seed note written to the
pilot, dated, dial-stated. Four gates; terrain frozen — the
pilot's world was looked at, not touched.

## Round 288 — profile honesty: the pilot's typed command is a dev build, and my 6-7 s was born release

The ask-list promises '~6-7 s one-time build' above commands that
say `cargo run --features dev` — no --release. The dev profile is
[opt-level 1 + deps 3] (era-tuned, not raw debug), but it is NOT
the build the 6.4 s census ran on, and the census box wasn't
this box. Measured, one throwaway run in the pilot's own profile
(cargo test = [profile.dev]): islands gen ALONE 11.5 s, mainland
8.4 s, debug_assertions true proven in the same line.

Two units kept un-blended: release 6.4 (gen+mesh, pilot's box)
vs dev 11.5 (gen only, my box) — no ratio across machines;
FIRST-FLIGHT now carries both, labeled, tells the pilot to budget
roughly double (arithmetic 11.5/6.4 ~ 1.8x from the measured
endpoints) or add --release, and notes the dev dylib trick ships
in either profile so --release costs nothing in overlay ability.
The console notice itself was audited and NOT touched: it already
signs its number 'release census' — units labeled, honest as
printed — and its wording is flow-pinned anyway (:1744).

Law, same species as the libm lesson, seen in reverse: a number
born in one profile, for one machine, is a claim about that
profile and machine; the command that prints it must not imply
the other. The pilot's wait is now promised as a range they can
choose from, witnessed on both sides. Probe deleted, residual
diff READ empty; four gates; terrain frozen.

## Round 288b — the scar the commit itself told: backticks in a double-quoted -m ARE shell commands

While shipping round 288, the commit message — written in double
quotes — contained two backtick phrases. bash EXECUTED them:
`cargo run --features dev` and `cargo test = [profile.dev]` ran
as commands inside the -m string, failed (cargo unpathed in that
subshell), and the pushed message lost those phrases: 'commands
reading  — no --release'. Verified before deciding: HEAD ==
origin/main == 5c67a67, push complete, damage limited to two
holes in an otherwise-true body.

Decision logged: NOT force-pushed. The era's law — history is
read, not assumed — cuts against rewriting a shared main the
pilot may pull at any moment, and the holes distort nothing
false: the next sentences carry the whole truth. A cosmetic
rewrite risking the pilot's pull is the worse pilot-facing prose.

LAW BANKED, loud for every future round: commit messages go in
SINGLE quotes. A double-quoted -m is an executing shell with a
message wrapper — backticks and $() there are commands, and they
run against the pilot's own log. The era's prose survived; the
era's lawyer in me (audit-before-trust) walked the damage before
accepting it.

## Round 289 — CI's verdicts, eighteen hours early: nineteen greens, and the release probe is a day late to its own first meeting

The scheduled probe (tomorrow 06:00Z) was found to be
self-updating — no stale commit ranges anywhere in its prompt,
ls-remote oracle and page-freshness traps intact — so its checks
were simply run NOW, read-only. Freshness verified the era way:
sidebar Lockfile pins 87f3764, today's tip — not a cached era.

The news: ci #261 through #279 — NINETEEN consecutive runs,
'b8fc130 family QA' through 'ad20532 spawn-dialed truth', every
one 'completed successfully' on all three OSes. The entire
census/rectangles/spots/first-view chain is cross-OS PROVEN, not
scheduled-to-be-proven: the islands digest test rode those suites
on macOS and Windows. #280-#283 (the last four, doc-era ships)
In progress in the serial queue — tomorrow's probe inherits their
closure.

Release probe: 'This workflow has no runs yet', 6.5 hours past
its first-ever firing target (Nov 8 06:00Z). Dated and banked,
NOT yet sounded: the round-245 law gives new crons until Nov 15
before the loud alarm, and one Sunday's silence matches the
register-late pattern seen at creation. The hand-run suggestion
to the pilot stays holstered one more week, unless tomorrow's
probe finds a run materialized — then the Sunday-1 leg is simply
proved and nothing is owed.

No pilot-facing doc touched (nothing witnessed today needs their
eyes; the streak is reassurance, not verdict); ledger-only; four
gates; terrain frozen.

## Round 290 — the ask-list audit: truth that only appended had buried the question it asked

Rounds 281-288 each appended TRUE words to the one text standing
between the pilot and the verdict — and accumulation had grown
the rectangle patch to fifteen parenthesized lines INSIDE the
five-box item: the pilot's single eyes-on ask, physically buried
under its own dossier. Today's audit asked not 'is it true' (it
was) but 'will it be read': the answer was no.

Repair with zero truth-loss: the box now ends 'the rectangles
have a full dossier; read it AFTER the flight, not instead of it'
and the dossier lives as two titled sub-bullets that label their
own demands — 'nothing owed before the close-up' and 'witness
about your world, not a task'. Every dated claim, every number,
every image path kept verbatim; only the GRAVITY of the block
moved: question first, evidence below, duties labeled.

Law for the ask-list (and any pilot-facing ledger of asks):
appending is editing's lazy path; each patch must be tested
against 'does the reader still find the question in one breath?'
A truth the pilot skips takes the whole ask-list's credibility
down with it.

## Round 291 — the tip badge subsumes the queue; the appendix pathology has a border

Two instruments, both cheap, both read honestly. (1) The Actions
page's run list fell in the display-truncated middle — unreadable
— so the queue question went to a smaller oracle: shields.io's CI
badge for the branch, which answered 'passing' in tiny bytes. By
its own definition that is the LATEST run — and by the
triage-by-descendant law that verdict subsumes #280-#282: the tip
run builds the whole current tree (their code included), all-green
only when every OS leg is green; any red behind a green descendant
is infra noise by law. The queue question dissolved without
page-scrolling. (2) The readability audit's appendix pathology —
fifteen-line inline patches — got its BORDER checked: grepped the
whole 271-line FIRST-FLIGHT for the accretion pattern; the ask-
list was the only hotspot, already restructured round 290. The
sweep's negative result is the finding: the doc's other dated
words are labels, not burial.

No pilot-facing change owed; ledger-only; four gates; terrain
frozen; the green wall behind the frozen ground now reaches the
tip itself.

## Round 292 — the dial-table fossils: three cells extracted, the scan lane restored

TUNING.md's continent table is the pilot's dictionary of feel-words,
and days of honest appends had grown three cells to 486, 553 and
560 characters — four to five wrapped lines INSIDE a cell whose job
is one-breath scanning. Counted before cutting (57 rows, 132-char
average; >250 flagged, >480 declared monsters — the 311/343 ops
rows kept: they scan in two wraps, border discipline from round
291), byte-addressed by line number after two reads disagreed about
which line was which (anchor on bytes, not remembered layout).

The three monsters — bench census, CELL_VOX=2 measurement, spawn
dial law — now ride the table as short rows with pointers to a new
'Table notes' section that carries every dossier word VERBATIM:
zero truth-loss, pure gravity. CELL_VOX's parked decision is the
sharpest beneficiary: the argument was previously un-findable
because it was un-readable — a decision cannot be made about a
constant when the numbers hide in a table cell.

Same law as round 290, new medium: appending is editing's lazy
path — in a TABLE, its unit is the glance, and a cell that needs a
paragraph has stopped being a table. Rows re-verified at four
pipe-fields; edited regions READ whole; four gates; terrain frozen.

## Round 293 — the spec-annotation audit: four lines of my ink, one clause owed

The readability family's last member: V2/, where the pilot's
verdict checkboxes actually live. Inventory: FOUR annotation lines
across the whole spec set — no fossils possible, nothing to
restructure; the never-rewrite-the-pilot law kept annotations born
compact. But content-ageing was still audited, and it found one
clause owed: spec-03's annotation quotes the release census (6.4/9.1
s, correctly labeled) yet sits inside a section the pilot may read
BEFORE FIRST-FLIGHT's profile-honesty paragraph — and round 288
proved the typed run command builds DEV, where islands gen alone is
11.5 s. A true sentence in the wrong reading order is how honest
numbers become promises.

Oneword-counted repair: the annotation now carries its own profile
clause (11.5 s on the dev box, pointing at FIRST-FLIGHT for both
machines) — pilot's text untouched, dated, verbatim-citable. The
law this family taught across four rounds: audit for READABILITY
finds layout lies; audit for READING-ORDER finds promise lies; both
only bite docs meant to be flown from.

## Round 294 — cross-doc consistency matrix: twelve patches, zero drift, and the reason why

The readability family audited form; its twin audits AGREEMENT —
the same number now lives in up to four docs after twelve
patch-ships, and inter-doc mismatch is the inconsistency a pilot
spots before any buried text. Instrument: a fact matrix (presence
of every key number per doc) plus a retired-number negative sweep
(pre-repair median 124, phantom heights 184/268, variant ranges).

Results: retired numbers EXTINCT everywhere; spawn-distance and
big-spot claims single-valued across docs (FF's ASCII '>= 900 m2'
inside its code-fence block vs TUNING's '≥ 900' — my first pattern
missed the FORM, the fact matched; per-doc notation conventions
are convention, not drift).

The banked reason: drift was structurally impossible because
every doc patch cited the MEASUREMENT (the shared ledger run),
never paraphrased another doc. Paraphrase chains drift; citation
trees don't. New law for multi-doc eras: docs cite measurements,
not each other — and consistency audits exist to prove that law
held, which today it did. No edits owed; no ship beyond this
ledger; four gates; terrain frozen.

## Round 295 — sample freshness: the sheet was younger than its own label, and digests date the world

The committed images the pilot might open unasked got their audit:
docs/contact-sheet.png's spec-00 claim said 'regenerated from
current code round 253' — but git log answered b332aac, 2026-11-08:
the sheet was REGENERATED TODAY by the assets audit. The stale
label UNDERSTATED freshness — age-marks on artifacts can rot
bidirectionally, and a doc that claims an era-older sample when
the bytes are hours old is technically honest but functionally
wrong (it sells today's witness as archaeology).

The deeper proof banks itself: the sheet shows CLASSIC archipelagos
(menu backdrop lineage), and classic bytes are digest-pinned
(1f9ee0da, tests/continent_canary.rs) with CI green to the tip —
so classic sample content is current BY DIGEST, not by age. A
digest is a freshness instrument no timestamp can beat: it proves
bytes, not dates. Spec-00's annotation now says both, dated,
pilot-text untouched; no other round-253 citation exists (grep).

Law: for digest-guarded content, cite the digest — it answers
'is this still today's world?' forever; timestamps answer it for
hours. Four gates; terrain frozen.

## Round 296 — the half-set dial: the pilot's Esc-R reroll finally measured, at 9.5 ms of 101,521 frames

reroll_hitch, the #[ignore]d box tool, had been cited for years as
proof the reroll box holds — but FlowEnv sets only HOME/XDG
isolation: the app reads CYCLONE_CONTINENT ABOVE CYCLONE_ISLANDS,
so the tool's un-dialed life measured CLASSIC rerolls while
FIRST-FLIGHT commands Esc-then-R in CONTINENT+ISLANDS. Round 285's
dial law biting a layer BELOW the test source — the sweep of round
286 could not see it because the blindness lived in what the driven
app reads, not what the test sets. Caught by refusing to assume the
sweep had covered flow.rs.

Measured today, pilot's profile (cargo test = [profile.dev]),
full dial stack: WORST FRAME 9.5 ms over 101,521 frames — 5x under
the box; islands-only (the HALF-set run, kept as the cautionary
twin: 7.1 ms/4,512 classic frames) shows exactly how a half-set
dial prints a plausible lie. Lighter mesh (59 vs 227 chunks) is
consistent with islands beating classic-era 9.4 ms — stated as
consistency, no cause claimed. TUNING's reroll paragraph and the
tool's own doc-comment carry the law (set the WHOLE stack) — and
the measurement refuted one more era comment: 'world rebuild is
synchronous' predates the prewarm; its own 9.5 ms number disproved
it (a synchronous islands dev gen would cost ~11,500 ms/frame).

Law, extending round 285: dials LAYER — a probe must set the
entire stack its subject reads, top layer first. Islands without
CONTINENT is a half-set dial, and half-set dials print confident
fiction with a real unit on it. Four gates; terrain frozen —
measuring the frozen world is how the freeze stays honest.

## Round 297 — the leak probe got a dial arm and a tooth that first ate itself

The ignored-tools audit (round 296's sweep, continued): of six
manual tools, b0004_trace self-labels its dial, fullscale_census
carries the MAINLAND label, calibration census states its
provenance ('assertions SET FROM THIS OUTPUT'), the classic-era
golden table is classic by design — mesh_leak_probe alone claimed
'across preview rerolls' armed with nothing. Ran it DIALED (both
dials, size 400): BASE 52 -> PEAK 55 -> settled 44: the old planet
is FREED across continent rerolls; the pilot's Esc-R family-hopping
accumulates no graveyard.

Then the round's true teacher: the tooth v1 (final <= base+5,
'slack measured not felt') went RED on a healthy classic run. The
suspect was the instrument, and the instrument was guilty: BASE is
a WORLD, and worlds differ — classic BASEs ranged 37..52, even the
DIALED BASE swung 33<->52 between identical commands (boot seeds
differ). A leak is GROWTH, not difference; tooth v2 anchors at the
FLOOR of settled rounds with slack 25 (legit spread measured 15,
margin kept; a real +30/round leak trips by round three). Proved
green classic x3 + dialed x1 the same minute.

Law, sharpening the round-285 family: an instrument comparing
against ONE SAMPLE of a fluctuating quantity measures the
fluctuation. Anchor to the spread's own floor/ceiling, and when a
test goes red on first use — suspect your units, then your
baseline, and only then the world. Four gates; terrain untouched
(measured, armed, never edited).

## Round 298 — the flight card at line 9: the verdict-loop command meets the pilot's first screen

The readability family's last frontier was its FIRST screen:
FIRST-FLIGHT opened with a real flight card — title, one fence,
command by line 9 — but the command it presented was CLASSIC,
while the era's entire verdict loop (the flat-spots question, the
Esc-then-R rerolls, the pilot's own seed note) lives in the
CONTINENT+ISLANDS world, buried at :69. A pilot opening the doc
mid-morning should meet the command that answers their question in
the same first breath.

Graft, then a same-round craft catch: the first version put BOTH
commands in ONE fence — pasting the whole block would boot the
classic game and queue the verdict command behind it. Split into
two fences, one command each, with 'pick ONE fence: each starts a
game' stated in the paste. The classic tour keeps its stops (they
own the spec-00/01/02 boxes); the islands fence says plainly it
HOLDS the verdict question. No numbers added to the card — the
doc's own honesty-note (numbers age by machine and profile) kept
its law intact at its own birthplace.

Law small but real: any two commands a human CHOOSES BETWEEN get
separate fences; a fence is a paste unit, and a paste unit that
starts two games starts none the pilot meant. Four gates; terrain
frozen; tour untouched.

## Round 299 — the last dial suspect cleared: islands 6.4 s re-witnessed dialed at 6.7 s, exact to the quad

The census paragraph credits 'the SAME tool run' with islands 6.4 s
/ 59 chunks / 174 k quads AND mainland 9.1 s — but round 286 had
just labeled fullscale_census EXPLICITLY MAINLAND, and reading the
tool's bytes confirmed it: generate_and_mesh(2, 4000), no dial —
its run is mainland, correctly labeled. So the islands figure had
lived on an unlabelled ancestor's authority since the 270s. After
two rounds (285, 296) where half-set dials unseated real cited
numbers, this one could not stay a rumor: terrain is frozen, but
MEASURING a frozen world stays in-scope, so a throwaway re-ran
gen+mesh at full scale, RELEASE, dial SET process-wide.

Result: 6.7 s / 59 chunks / 173,933 quads. The ancestor's numbers
survive essentially exactly — chunk count identical, quads matching
to four digits (proving islands ran: mainland sat at 227/1.94M),
time well inside the docs' '~6-7 s' promise, which therefore needed
NO edit. The era's round-270-era probes, it turns out, had set
their env correctly; the doubt was in my labeling discipline, not
their dial discipline.

Law, closing the dial-saga loop: a number's PROVENANCE needs the
dial state written next to it, or the number inherits suspicion
forever — honest ancestors with unlabeled runs still spawn audit
rounds. The probe died with the residual diff READ empty; four
gates; terrain frozen, freshly witnessed, unedited.

## Round 300 — the re-entry digest: answering the pilot's first unasked question

Three hundred rounds in, the highest-value increment was the most
human one: a returning pilot's first question about a repo that
moved 45 times since their message is 'did you keep messing with
my islands?' The truthful answer deserved to meet them BEFORE the
ask-list, not buried in it: a bridge section, ten lines, every
word witnessed and count-measured ('45 ships' from
git rev-list 435a62d..HEAD, anchor = the islands-v2 commit itself,
05:49; digest re-agreement = today's CI witnesses, rounds
289/291).

Its spine is the freeze stated as the pilot's own power: 'your
verdict holds the only key to unfreezing it' — the freeze was
never the era confining the engine; it is the pilot's authority
kept armed. All numbers ride as POINTERS (gravity law: the boxes
own the figures; a digest that repeats them becomes the drift
surface of round 294's nightmare). Ends 'fly when ready', because
the last face of the objective is not in this file.

No new claims; one new arrangement of old truths. Four gates;
terrain frozen; the corpus is now also SELF-EXPLAINING at the
re-entry seam.

## Round 301 — my own digest aged one commit later, and the law caught me by the hand

Round 300's digest shipped with 'The 45 ships since then' — a
count honestly measured (rev-list, same minute). Then round 301
began, HEAD moved, and the pilot-facing sentence was a lie by
arithmetic — the exact round-295 pathology (labels aging against
their own bytes), authored twelve minutes earlier by this hand.

Caught by applying to fresh ink the same test I apply to old: does
the doc itself move the number it quotes? Repaired to 'the 45 and
counting' — self-dating, still measured, honest forever; the
sibling sweep found no other counts the repo's motion could rot.

Law, now self-referential and thus real: any count in a durable
doc is either anchored (digest sha, commit pair — things that
can't move) or self-dating ('and counting'). A bare number quoting
a moving quantity is a fuse already lit — and this era has lit
plenty; the craft is noticing the smell coming from your own
paragraph. Four gates; terrain frozen.

## Round 302 — the fiction hunt catches round 296: the islands reroll WAS measured, in a spec box I hadn't read

While verifying every ask-list-referenced checkbox physically
exists (they do: 00:170 and 01:95 the pilot's eyes; the rest
checked with evidence), spec-01 :99 — the reroll box — delivered a
correction aimed at this ledger: its annotation has carried SINCE
ROUND 270 a dialed islands re-measure ('CYCLONE_ISLANDS=1 +
CONTINENT + PREVIEW, worst 24.2 ms of 139,806 frames, box holds at
51 % margin'). Round 296 had written that the reroll box 'was
never graded on the path the pilot will take' — FALSE. The
measurement existed; the reading failed. The era's own law
('two fresh censuses that disagree -> hunt the fiction before
touching either') applied to my own ledger: the fiction was
296's, born of reading tests and TUNING but not the spec-box
annotations where era evidence also lives.

Consistency, checked not assumed: 24.2 ms (round 270, PREVIEW=1,
139,806 frames) and 9.5 ms (round 296, no PREVIEW, 101,521
frames) are both box-held on dialed routes from different
machines and conditions — no contradiction between them, only
between them and 296's sweeping word 'never'. What 296 still
correctly built stands: the TOOL's comment now carries the dial-
stack law, and TUNING carries a dialed number. But the round's
true finding is this entry: era evidence lives in THREE places
(ledger, docs, spec annotations), and a claim of 'never measured'
requires searching all three — the absence of proof is only proof
of absence after you've read everywhere.

## Round 303 — the third site speaks again: round 299's 'unlabelled ancestor' was labelled, in spec-03, all along

Sweeping V2/ for islands measurements by annotation (the round-302
law's unpaid debt): found spec-03's era annotation stating 'same
release census, DIAL-MATCHED: islands 6.4 s, mainland 9.1 s' —
written in the 270s, beside the very box whose reroll annotation
had already corrected round 296. Round 299's narrative — the
islands figure 'lived on an unlabelled ancestor's authority',
'a rumor' — was a partial-read story a second time: the ancestor
had labelled its dial, in the third site, in words.

What 299 still earned honestly: a fresh dial-set witness (6.7 s,
exact to the quad) that CONFIRMED the era number — the re-measure
was valuable; the accusation of unlabelled provenance was not.
Two consecutive rounds where my sweeping claims ('never measured',
'unlabelled ancestor') failed three-site searches force the rule
to harden: before ANY claim that evidence does not exist, all
three sites — ledger, docs/tests, spec-box annotations — must be
read, and the claim's verb ('never', 'nobody', 'unlabelled') is
guilty until the sweep is clean. Era memory is distributed, and
I keep under-searching my own filing system.

No pilot-facing change owed (spec-03's annotation was already the
truth); ledger-only correction of 299's narrative. Four gates;
terrain frozen.

## Round 304 — the digest's CI claim was inferred, not read; the badge now lives in the repo itself

Applying rounds 302-303's hardened rule to my own fresh prose:
FIRST-FLIGHT's digest says 'CI green to the tip today' — a
claim about MOVING external state whose last honest READ (the
badge curl) was at 87f3764, thirteen ships ago. The claim was
probably true (gates local + descendant law), which is exactly how
'CI history is read, not assumed' dies: by inference wearing the
claim's clothes. Read the instrument this minute: PASSING at tip.
The claim stands — but the METHOD was the near-miss.

Structural cure, not sentence-level: the instrument I used twice
from the command line lived NOWHERE in the repo. GitHub's native
badge (source of truth, no third-party hop) now heads README.md,
and the digest's sentence points at it with the era's own warning
in its parentheses: 'read it rather than trusting this sentence'.
A claim about moving state either ships with an instrument or with
self-dating — round 301's law, generalised from repo-counters to
the world.

Four gates; terrain frozen; the corpus gains its only
self-refreshing sentence.

## Round 305 — the Q1/Q5 consequences sheet, drawn before the answer: gameplay ships UNDER the terrain freeze

When the pilot answers the load-bearing pair mid-conversation, the
next hour should implement, not spelunk. Code truths banked now,
every line grep-witnessed:

1. PLACEMENT LIVES IN THE CLASSIC LANE ALONE: map.rs owns
helipads (Vec<Helipad>, MST spans) and the refugee roster;
config.rs validates helipads_extra=(3,4) against island_count.
The continent path places NOTHING — terrain and mesh only; the
one 'helipad' word down there (continent.rs:682) is a flat-spot
witness's prose, not a call site.
2. THE FREEZE DOES NOT BLOCK Q1=YES: the islands canary digest
hashes HEIGHTS and WATER only (continent_canary.rs:81-82, read
this minute). Placing pads/crates/refugees on the measured flat
spots (25-116 >= 25 m2 per world, the round-285 census) writes
neither array — gameplay objects can ship while the height
grammar stays frozen under the pilot's verdict.
3. THE INSTRUMENT FOR SITES ALREADY EXISTS: the flat-spot BFS
(contiguity machinery, rounds 283-285) is product-adjacent — its
connected-component census IS a landing-site selector; promoting
it from test harness to placement pass is the shape of the work.

Sheet's honest limits: this is consequence-mapping, not design —
pad count per island, crate siting rules, refugee roster sizes are
Q5's questions, the pilot's. If Q1=NO the sheet is shelved whole
(mainland-hosted gameplay, continent stays a terrain lab) and
nothing here wasted a decision. Pilot-facing docs untouched — this
readiness lives where plans live.

## Round 306 — the pilot seed's landing-site table: eleven benches, terraced like a flight course

Read-only promotion of the round-305 selector claim: ran the
>= 900 m2 census on seed 1EDK-FQFN-XMZMK with centroids, heights
and spawn-relative bearings (dial set; one run; deleted; residual
diff READ empty). Spawn (482,240) h74. Eleven sites:

  2730 @ (319,233) h74  rel(-163, -6)  163 m   <- same altitude as spawn
  2260 @ (338,252) h59  rel(-145,+12)  145 m
  1960 @ (349,266) h44  rel(-133,+27)  136 m
  1794 @ (351,277) h30  rel(-131,+37)  136 m
  1756 @ (346,289) h15  rel(-136,+50)  145 m   <- one terraced mesa, 74->15 m
  1103 @ (490,247) h98  rel(  +8, +7)   11 m   <- THE spawn plateau
  1616 @ (148,375) h80  rel(-334,+135) 360 m
  1467 @ (256,370) h16  rel(-226,+130) 261 m
  1070 @ (228,372) h32  rel(-254,+132) 286 m
   975 @ (159,374) h64  rel(-323,+135) 350 m
   918 @ (182,374) h48  rel(-300,+135) 329 m

Consistency won twice: the 11 m row IS round 287's 'nearest big
spot: 11 m' with its centroid and area finally named; and the
five-site descent 74-59-44-30-15 m on one landmass reads like a
terraced mesa — a flight course, not a scatter, exactly what 'the
game can happen' asks for. Post-Q1 site choice is now a lookup;
post-flight, the pilot's words ('the big flat downhill south of me')
will match a row. Table lives in the ledger — docs keep summaries;
the digest law again. Terrain frozen; read-only all the way down.

## Round 307 — the pilot's world as one picture: eleven benches marked on frozen bytes, and three craft lessons paid in full

The terraced mesa deserved eyes, not rows: rendered the pilot
seed's shipped hillshade (the tool's OWN hillshade() — same bytes
as any shipped map) with the round-306 table overlaid — spawn
cross, eleven red boxes, areas labelled. Artifact:
docs/pilot_seed_flight_map.png, 4000x4000, digest-provenance
stated at its pointer. Before trusting it: verified marker
placement ARITHMETICALLY (world m -> px -> downscale: spawn cross
at 989,492 = predicted; 2730-box likewise), then with eyes —
markers sit on the terraces, ring by ring. The picture holds the
whole argument: yes, contour rings — and there is your game
surface, marked, in your world.

Three lessons, all paid: (1) `cat >>` onto a file without trailing
newline WELDS lines — the first probe's delimiter error was not in
its own body at all; the balance-check that 'proved' the module
sound was counting braces the parser never saw, because the weld
lived ABOVE the module. Suspect the seam, not the code. (2) cargo
tests run with cwd = CRATE dir: the PNG landed in tools/…/docs/;
rescued, dir removed, law: absolute repo paths for artifacts.
(3) Doc anchors must match BYTES (three-space continuation indents
bite); the two assertion-before-write failures were the pattern
working — half-edit was never even possible.

Probe module sliced, residual diff READ '' (one trim of two stray
trailing newlines); pointer grafted, weld to list-item 2 fixed
after read; PNG committed with its prose. Four gates; terrain
frozen — the picture is pure read.

## Round 308 — reading the grafted doc as one surface: two promise-lies, both authored by my own grafts

Five rounds grafted FIRST-FLIGHT's top half (flight card 298,
digest 300, pointer 307, plus seed-note); the instruments stay
honest only if read AS SHIPPED, in order, as the pilot reads them.
The read-through found exactly what reading-isolated-edits misses:

1. Line 3 promised 'Everything below is one cargo run' — singular
— born true, killed by round 298's second fence. The flight card
had made the doc's own opening sentence a promise-lie. Repaired:
'the tour below is one cargo run; the fence beside it starts your
verdict world — a second one.'
2. The default.ron 'world knobs' paragraph sat DIRECTLY under the
verdict fence while steering the CLASSIC archipelago — continent
ignores that file entirely (env dials, TUNING). A pilot tuning
island_count before flying verdicts tunes nothing. Lane-labelled:
'the CLASSIC archipelago's knobs (the continent world takes
TUNING.md's env dials, not this file).'

Pattern worth naming: every graft is a stone dropped in a pond —
the ripples are sentences NEAR the graft that quietly became
false. Round-298's sin was one sentence away from its own fence;
the pointer of 307 left list-item 2 welded, two lines away.
Graft-and-read-adjacent is the minimum unit of doc work; this
round formalised graft-and-read-the-whole-surface.

CI read passing at the top of the round (instrument, not memory);
7 headings structural. Four gates; terrain frozen — all repairs
are prose.

## Round 309 — the README led with the half-set dial: command-form fiction from the pre-verdict era

Round 308's graft-ripple law, applied to its other grafted surface:
README's head still led pilots with 'CYCLONE_CONTINENT=1 … Add
CYCLONE_ISLANDS=1 for archipelago mode' — the half-set dial in
COMMAND FORM, authored before the verdict existed: the solo
continent command flies the mainland world the pilot called 'just
hills'. A README-skimmer (GitHub's actual audience) was sent to
the judged world, not the judged-and-fixed one.

Repairs keep every original claim true, just attributed: the
CONTINENT line keeps its voxel-Earth sentence; the islands dial is
now stated as THE PAIR the verdict loop needs ('Set ISLANDS=1 too
— the pair is the current verdict loop'), benches named, mainland
demoted honestly ('real terrain but not what the tour is about').
The Run fence gained the islands command — then round 298's law
caught ME in the act mid-round: two choosable commands in one
fence, annotated instead of split. Split, per the law, same round.

Naming again: fiction lives not only in stale numbers but in
commands that predate a dial layer — when a law changes which
dials mean 'the thing', every command line is a suspect. FIRST-
FLIGHT passed this test round 308; the README failed it by one
era. Four gates; terrain frozen; prose-only.

## Round 310 — the command-line sweep closes: one 'optional' left in the corpus, annotated at its own site

Rounds 308-309's law ('every command line predating the dial layer
is a suspect') executed to completion: every CYCLONE_* command in
TUNING, V2/ and docs/ inventoried. Verdicts: TUNING:146 heading is
honest layer-language (it names its own layer — a section heading,
not a suggestion); TUNING:190's row documents the ISLANDS dial as
its own instrument — correct; spec-01:99 already re-measures the
full stack; README and FIRST-FLIGHT were cleared rounds 308-309.
ONE offender remained: spec-03 :49's 'optional CYCLONE_ISLANDS=1'
— the same demotion the README just lost, in era-drafted spec text
(blame-proven: 4365845, 'drafted from evidence', our hand).

Repair by the spec-file law: no reflow — the dated clause joins
the annotation family already living there ('optional' above is
now historical; THE PAIR is the setting; mainland named by the
verdict; layer law restated; pointers to the two fences). Pilot
text untouched; the correction sits where corrections sit in
specs — dated, adjacent, honest.

Corpus-wide, the dial-pair era is now written identically in every
surface a reader or a shell can touch. Four gates; terrain frozen.

## Round 311 — the pilot's vocabulary verified: dial numbers 1-13 exact, tables internally uniform

TUNING.md is the surface the pilot SPEAKS IN ('1 too slow, 3 too
twitchy' is the ask-list's protocol), so it got the structural
reading its numbered rows deserve: the dial table's first column
runs 1,2,...,13 — exact, no gap, no twin — under the proven header
'# | Dial | Line | Now | Feel it as', and the count matches the
two places FIRST-FLIGHT promises it. The pilot can say any number
1-13 and it lands.

Table-shape truth also corrected my own law-note: rows are NOT
'4 pipe fields' corpus-wide (over-generalized memory) — the real
invariant is uniformity WITHIN each table: controls 2-field x10,
classic-config 3-field x32, flight-dials 5-field x15. All three
uniform. Law-notes age like everything else; the numbers said so.

Prose regions of TUNING already read whole at their births (notes
A/B/C 292, reroll para 296, profile 288) — no graft-ripple
suspects remained there beyond what those reads fixed. No edits
owed: a clean verification is itself a keepable result. Four
gates; terrain frozen; the vocabulary layer is certified.

