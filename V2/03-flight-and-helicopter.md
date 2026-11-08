# 03 — Flight and helicopter

**Status: DRAFT FOR PILOT REVIEW — do not build until signed off.** The
pilot writes the V2 specs; this one is drafted from evidence (the code
as it stands plus the gameplay rules `V2/README.md` points at, i.e.
`specs.md` §Game design) so the review is an edit, not a blank page.
Everything marked OPEN is a decision only the pilot can make — several
were forced by the terrain work and could not be answered silently.

**Goal:** the free camera becomes a helicopter you can fly, land and
crash. After this spec a New Game is a round you could win, if specs 04
(objectives) and 05 (HUD) existed — 03 stops at "a machine that obeys
the sky".

Depends on: specs 00–02 (engine, menu, generated archipelago). Consumed
by: spec 04 (crates, cyclone, planes — all of them act *on* the machine
this spec builds).

---

## Where the code stands today (Nov 8 2026, commit `5227797`)

Real, shipped, tested:

- `crates/cyclone_engine/src/camera_rig.rs` — 515 lines. Free mouse-look
  camera: `CameraRig` (yaw/pitch/fov), `RigFollow`/`RigTarget` (follow
  an entity), `CameraHeading` (the four-heading quadrant that drives
  crate occlusion), `WorldBounds`, `CameraRigPlugin`.
- Fly controls on the preview/in-game camera: WASD + Space up + Shift
  down + Ctrl boost (rig_fly, `camera_rig.rs`).
- Terrain collision for the rig (TRUE since round 195, `cac50e8` —
  it was a written-then-tested myth; the test-first order now
  official): `TerrainFloor` oracle resource (engine) installed by
  the world builder per seed; free-fly holds skid at surface + 2.5
  m. `spawn_over_land` remains the golden-angle scan for standable
  spawn ground. Archipelago keeps its flat historic floor — Part A
  may unify if the pilot wants collision there too (OPEN #6).
- Speed model: `FLY_BASE` (38 u/s at fov 45, fov-scaled) in
  `TUNING.md` — a *velocity*, but with no mass, no drag, no fuel.
- World DATA for the round is generated and validated already: 14
  islands on a 160×160 tile grid (1 tile = 1 world unit), Base Island
  with helipad, 3–4 further pads, crates and refugees on islands, all
  invariants gated per seed (`cyclone_worldgen`). The machine just has
  nothing to interact with yet — helipads exist as data, not as
  places that refuel.
- The voxel Earth (`CYCLONE_CONTINENT=1`, optional
  `CYCLONE_ISLANDS=1`) flies as a terrain lab: dev gate at Enter,
  one-time build (~8 s), full scale by default,
  `CYCLONE_CONTINENT_PREVIEW=1` boots straight into flying (it is a gate-skip, nothing more — round 183).

Not written at all: fuel, altitude clamp, landing/crash states, winch,
the helicopter body, its rotor hum, a cockpit or chase view distinct
from the free rig. `grep -c fuel` on the game crate is 0.

---

## Part A — The machine (`cyclone_game`, gameplay side)

Every number below is a *starting value from specs.md*, ported to 3D.
They live in `TUNING.md`, never inline. The 1984 original's feel is the
tiebreaker for anything ambiguous; record each resolved ambiguity in
`DECISIONS.md`.

### A1. State

`Helicopter` resource or component: position (world units), velocity
(Vec3), heading (yaw), `fuel` (0–100), `cargo` (crates, refugees),
`state` (airborne / landed / crashed). One helicopter per round — no
multiplayer anywhere in this spec.

### A2. Controls (inertia, not teleport)

- WASD thrusts horizontally with acceleration + drag; top speed ≈ 3
  world-units/s (`specs.md`: "3 tiles per second"; re-tune by feel at
  V2 scale). The existing fly-cam's instant velocity is the dev tool
  and stays behind the dev gate.
- Space / Shift change altitude at a fixed rate, clamped 0..8 world
  units (the old 16 px units re-expressed on the V2 grid; confirm at
  first flight).
- The mouse keeps looking (it is the same `CameraRig`); turning the
  helicopter and turning the view are ONE yaw — OPEN Q2 below.

### A3. Ground contact

- Landed: over flat land, low ground speed, altitude ~0. Fuel refill
  begins on a helipad (full in 2 s), nothing off-pad.
- Crashed: over a slope, a building, a tree, sea, or touching down
  above the speed cap. Crash = −1 life (3 to start), respawn on Base
  Island keeping cargo.
- Fuel drains with time airborne, faster at speed; zero fuel = engine
  cut, descent at the fixed rate, ground contact decides land vs crash.

### A4. Winch

Hovering within half a world-unit and below altitude 2 for 0.5 s
auto-lifts a crate or a refugee. No capacity limit (1984 rule, kept).

## Part B — Views

Three views, one keybind cycle (numbers in `TUNING.md`):

1. **Chase** (default): camera trails the helicopter, follows yaw.
2. **Cockpit** (optional, OPEN Q3): first-person inside the models
   spec 06 will deliver.
3. **Free** (dev only, `--features dev`): today's fly-cam unchanged,
   for terrain work — this is the terrain-sim pilot and must not break.

## Part C — Tests

Physics is pure math where possible: keep the integrator in a plain
function/module the tests can call without a window (same discipline as
`cyclone_worldgen`'s purity), headless over all CI OSes:

- fuel monotonic while airborne; helipad landing refills to 100 in 2 s
- landed iff (flat land ∧ low speed ∧ altitude ≈ 0); crash otherwise
- zero fuel + sea contact = crash; zero fuel + helipad = landing
- winch requires BOTH the 0.5 s hover window and altitude < 2 (timer
  alone must not lift)
- determinism: fixed-step integrator from a fixed state → bit-identical
  trajectory (no wall-clock in the sim step)

## Acceptance checks

- [ ] Chase view: fly the archipelago, land on Base Island's helipad,
      see fuel refill; take off and watch it drain. (pilot's screen)
- [ ] Crash on sea and on a cliff; life counter drops; respawn keeps
      cargo. (pilot's eyes + a headless unit test per rule)
- [ ] Winch a crate by hovering over it; the cargo counter moves.
- [ ] Dev free-cam still flies the voxel Earth unchanged (the terrain
      lab must survive the game).
- [ ] CI green on 3 OSes; no `fuel`/`crash` arithmetic outside the
      tested module.

## Open questions for the pilot

1. **Which world hosts gameplay?** The specs' gameplay lives on the
   generated *archipelago* (spec 02). The voxel Earth (continent/island
   mode) arrived later as a terrain lab. Recommendation: gameplay stays
   on the archipelago; the voxel world remains a dev-only sandbox until
   the pilot retires the archipelago. Say the word if the voxel world
   should instead BE the game world — that changes A3 (a 1 m-cell
   surface changes what "flat land" and "a slope" mean).
2. **View-and-heading coupling:** in 1984 the world flipped N/S and the
   heading was *the view*. With free mouse-look, should the helicopter
   yaw follow the camera (arcade), or does W thrust along the helicopter's
   own nose while the camera looks freely (sim-ish)? This decides whether
   `CameraHeading` keeps its occlusion role during gameplay.
3. **Cockpit view now or with spec 06?** There is no helicopter model
   until Blender-scripted assets exist. Recommendation: chase only in
   03; cockpit lands the day 06 delivers the mesh.
4. **Fuel numbers:** 1984 gave generous fuel. V2 world is 3× bigger
   screens-wise — keep 5-minute-round assumptions or re-time from the
   new map dimensions?
5. **Helipad on voxel terrain** (only if Q1 flips): landing zones
   painted by `spawn_over_land`-style scan, or explicit pads in the
   island stamp?
