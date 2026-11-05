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
