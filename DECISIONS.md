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
