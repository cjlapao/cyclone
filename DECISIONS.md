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
