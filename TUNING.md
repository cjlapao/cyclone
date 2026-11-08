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

## Controls (defaults; remap by hand-editing the settings file — no remap UI; it's a pilot-ask)

The overlay edits only Fullscreen / VSync / Window size / Camera speed.
Keys live in the `keys:` block of the settings file (edit it with the
game closed; a bad file falls back to defaults):
Linux `~/.config/rescue/settings.ron`, Windows
`%APPDATA%\cyclone\rescue\config\settings.ron` (the crate appends `config` — verified in directories-6.0.0 win.rs:75), macOS
`~/Library/Application Support/com.cyclone.rescue/settings.ron`
(paths of `directories::ProjectDirs` — Linux verified by running it on a dev box; macOS/Windows paths are what the engine's `settings_persist` test round-trips on the CI runners — on macOS by redirecting `HOME` (the crate reads it from env), on Windows through the OS-resolved AppData, which no env override can reach).
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
| 9 | InGame start pitch | `screens.rs` `let pitch: f32 = if ground > 0.0` | −18° | first-frame angle when Fly! begins (−25° pre-feel-pass; −18° frames 10 of 14 islands at spawn — see DECISIONS round 101) |
| 10 | preview Tab pitch | `screens.rs` `pitch_hold = if top.0 { Some(-89.5)` | −89.5° | the flat top-down toggle angle |
| 11 | Base Island tint | `cyclone_render/src/terrain.rs` `base_island_tint` | ×1.10/1.02/0.82 RGB | the warm gold cast on the starting island |
| 12 | Q/E snap | fixed | 90° | heading snaps (drives crate occlusion per quadrant) |
| 13 | min-show progress | `screens.rs` | 300 ms | Generating screen floor before it can hand off |

## Debug profile is honest (measured, not assumed)

The tour says plain `cargo run` (dev profile). Measured on the
current code and config (160×160, 14 islands): `cargo bench` release
198 ms per 18-seed sample = ~11 ms/world (re-measured round 116; re-verified round 200 at 194 ms and round 243 at 195 ms median — three dates, drift inside noise through the libm freeze, the whole continent era and the mesh rework,
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
in-app (dev, ignored `reroll_hitch` test): **9.4 ms worst of 4635 frames**
(round 215 — the classic sea's foam/shallows mesh moved ONTO the
reroll frame: per-tile quads + one big into_mesh, main-thread, and
it is NOT ashamed: still past 60 Hz headroom and under a fifth of
the spec-01 50 ms box; the historic 2.8 ms/200-round and
3.8 ms/ceiling readings predate the sea and now quote only history)
ISLANDS-mode reroll — the Esc-R FIRST-FLIGHT actually commands —
measured 2026-11-08 with the FULL dial stack set (CONTINENT *and*
ISLANDS envs: islands alone leaves the layer above off): worst frame
9.5 ms over 101,521 frames on the dev box, dev profile — 5× under
the box; lighter mesh (59 vs 227 chunks) is consistent with it
beating the classic-era 9.4 ms, no cause beyond that claimed here.
(re-measured round 133, post terrain-overhaul — the taller terraces cost
nothing measurable; the number predates them and survived them) — under a quarter
of the 16.6 ms 60 Hz budget (round 215 honesty: at 9.4 ms it is now
over HALF the frame budget on the worst frame — still one frame,
never a dropped one, but 'quarter' was the pre-sea number). If a feel complaint ever smells like
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
values are free. Dev builds put the counter-evidence in your HUD
(fps · ms, terracing, seed), so "slow" arrives as "44 fps over the big
island, seed 6KS1-…" — a number and a world, not just an adjective.
After three feel passes we promote the settled values
into defaults and prune this file.

## Continent mode dials (CYCLONE_CONTINENT=1, rounds 154–155)

The terrain-sim pivot: a 1 km² voxel-Earth heightfield, 25 cm
columns, flown with the same rig. Judge without building: `cargo
run -p worldgen-cli -- continent --seed <S> --size 4000 --out <dir>`
→ hillshade.png from the SAME `elevation_m` the mesh uses (round-232 re-measure: ~8.1–8.6 s internal / 9–10 s wall at 4000², release, Linux dev box — the old '~7 s' had drifted; re-measure before quoting again).

**Value law (round 228, pinned by `continent_dial_is_strict_one`):**
every dial here activates on the exact value `1` — `CYCLONE_CONTINENT=0`
is OFF (it once read ON; the value trap is closed). One reader owns
each dial (`continent_mode()`, `island_mode()`, the PREVIEW/SCALE
gate): the meaning of a dial changes in its owner and nowhere else.
`SIZE` is numeric: voxels per side, clamped 64–16 000, default 4000;
the 0.25 m voxel edge is the invariant — the dial scales AREA, not
resolution.

| feel-word | dial (continent.rs) | current |
| --- | --- | --- |
| "flat / boring relief" | relief floor `40+80·fbm/160m` terms | 15.0 + 35.0 (post-census calm) |
| "no mountains anywhere" | orogeny floor `(0.05 + 0.95·mask)` | 0.05 |
| "peaks not sharp enough" | `ridged()` sharpen exponent `r*r` | r² |
| "mountains too small" | `max_height_m` (ContinentConfig) | 300 (round-176 rebalance: the flight said 900 m peaks on a 1 km world read as a tower) |
| "slopes too steep / crumpled" | ridge wavelength `ridged(wx/N …)` | 260 m (was 130; half as steep) |
| "mountains start too high up" | ridge onset `(max_height_m − 80)` | 80 (was 240 on the old ceiling) |
| "coast geometry too round" | warp amp 150 m / wavelength 350 m | 150/350 |
| "want more ocean" | land mask smoothstep(0.44, 0.54) | midpoint 0.49 |
| "detail looks like crumpled foil" | detail amp × 28 m | 1.0 m (was 2.2, the render said so) |
| "islands are just hills" | bench profile: count `(q * 5.0)`, tread mid `(k + 0.55)` | 5 benches; witness `islands_have_flat_landing_ground`; flat-spot census 2026-11-08 → note A below |
| "islands too low / too tall" | tallness `46.0 + 90.0 * fbm` on 300 m | 46–136 m per island (observed 88–116 m, 12-seed 1 km² census 2026-11-08 — the formula's ends are rarely both reached) |
| "sand flats too wide" | beach fraction `p < 0.18` (flats reach 6 m) | 0.18 |
| "coast too egg-smooth" | wobble `0.28 * (fbm(wx / 120.0` → radius ±14 % | ±14 % at 120 m |
| "eggs all point the same way" | rotation `(((h2 >> 8) & 0xFF) …) * …TAU` per stamp | was: NONE — every egg axis-aligned until round 257 |
| "too many / too few islands" | keep-coin `if coin < 0.45` at SPACING 340 m | 55 % of lattice sites (observed 3–8 per world, median 6 — same census; the spread is the loop's, no cause claimed here) |
| "blocky under me" | `const CELL_VOX` (A CONST in cyclone_render/src/continent_mesh.rs — recompiles, no env) | 4 (1 m mesh cells; CELL_VOX=2 is a REAL dial, not free → note B below) |
| "no rivers / too many rivers" | `CATCH` (carve_rivers, continent.rs) | 25k columns ≈ 1.6 km² of rain to earn a stream (3k over-carved a canyon — round-177 crater) |
| "rivers too thin / too wide" | flood window `1 + acc/(4·CATCH)` cap | 1–17 cells (≈0.25–4 m) |
| "rivers should flood more" | surface fill `bed + depth·2/3` | 2/3 of the cut holds water |
| "world is a traffic light of bands" | biome aridity fbm (earth_rgb, continent_mesh) | /300 m wavelength; cuts 0.30/0.55/0.78 (forest/grass/scrub/desert); altitude only above 95 m |
| "sea is a flat blue table" | sea bathymetry fade | shelf cyan → basin navy over 1 km from shore; buckets 100 voxels (25 m) |
| "colours band wrongly" | `earth_rgb` cuts (continent_mesh) + CLI mirror | 14/95/175/245 m — they MUST ride the relief band; the old 260/600/900 lived above a world that no longer exists |
| "./target/release/cyclone: libstd-*.so not found" | `dev` feature = Bevy dynamic linking | launch with `cargo run --features dev` (cargo sets the dylib search itself, debug or release); running the binary BY HAND needs `LD_LIBRARY_PATH=target/release/deps:$(rustc --print target-libdir)` — or build without `dev` (static, ships that way) |
| "what does PREVIEW do?" | `CYCLONE_CONTINENT_PREVIEW=1` (alias `_SCALE=1`) | SKIPS THE MENU GATE ONLY (planet live at boot, not at Enter). Round 183 deleted the "0.2× overview" myth: the 0.2 was never multiplied into any mesh/transform/bounds — comments-only code is dead code. Overview = climb to ~300 m |
| "spawned over the ocean" | `spawn_over_land` golden-angle ring scan | 40 m rings, ≥3 m standable (river-safe lower bound); centre-land hits at ring 0; fallback hovers + pitch −60°. ⚠ The dial law lives → note C below |
| "spawned on a sandbar" | same scan's preference | HIGHEST standable ground in radius wins (the flagship island), ties nearer; witnessed: first-touch picked a 4 m bar 160 m out over a 93 m flagship 393 m out |
| "a few islands like this photo" | `ContinentConfig.islands` (CYCLONE_ISLANDS=1) | point-process stamps on a 340 m lattice: coin 0.45, cosine blobs r=90–200 m, seeded squish; fbm tail-cut REJECTED by census (one island + gravel) |
| "islands have no beaches" | `near_wet` = river OR height-0 neighbour | beaches are sea-ADJACENCY; the water array only holds rivers |
| "shelf too wide / sea banded" | shelf ramp 150 m; sheet DTM normalised voxels² BIG=1e8 | f64 ulp: BIG must beat every true d² yet keep +p² exact — 1e18 erased sources, 1e13 faked distances |

### Table notes (extracted 2026-11-08 — dossier, not scan lane)

**A — the bench row's census.** Pilot verdict round 257; witness
`islands_have_flat_landing_ground` (88k flat cols of 256k high at
pad scale). Contiguity measured 2026-11-08 (3 seeds, the witness's
own 512 m map): those cols form 12–23 CONNECTED spots, 766–2,698 m²
largest, 6–8 spots ≥ 25 m² per map — but one family had NO spot
≥ 900 m², so benches everywhere, big level places thin out on some
seeds. Full-scale 12 seeds (DECISIONS rounds 284–285): 25–116 spots
≥ 25 m² per world (median 62), 4–23 ≥ 900 m², largest per world
1,868–6,842 m².

**B — CELL_VOX=2, measured.** (2026-11-08, release, same census,
both modes): islands 6.5 s / 161 chunks / 602 k quads (quad load
×3.5 for +1 % build time); mainland 10.2 s / 859 chunks / 6.76 M
quads (mesh ×3.5, build +12 %). The VOXEL was always 25 cm; the
MESH CELL is the visible cube; sea stays one quad via sparse drop.
A real dial, not a free one — every mesh digest re-pins with it.
Decision parked; the numbers wait here, not in the cell.

**C — the spawn row's dial law.** It re-derives height through
`surface_height_m` → `island_mode()` AT CALL TIME: the cfg islands
FIELD does not reach it — probes calling `spawn_over_land` must SET
THE ENV DIAL or they grade against mainland phantoms (caught
2026-11-08; the islands claim was then re-witnessed dialed: spawn
heights 55–95 m, big level spot 5–45 m from the nose in 11/12
worlds).

(Islands-vs-mainland release census, 2026-11-08, SAME tool run
back-to-back (`fullscale_census --ignored`, seed 2, 4000², release,
Linux dev box — one combined gen+MESH wall time, no split):
islands **6.4 s / 59 chunks / 174 k quads** vs mainland
**9.1 s / 227 chunks / 1.94 M quads** — the islands path builds in
71 % of the time and carries an ELEVENTH of the mesh, so 'islands
cheaper than continent' is now a same-minute pair, not a cross-era
quote; the 60 s regression tripwire passes with 10× margin. Frame
rate is the pilot's screen to judge; quads are the load I can
count.)

Rivers are DETERMINISTIC HYDROLOGY, not painted lines: D8 flow
accumulation over the real grid (counting-sort descent), carve where
the catchment earns it, water surface at 2/3 of the cut — the mesh
renders the surface as blue quads over the bed (the ocean plane
cannot show water above y=0; a river needs its own surface). The
`water` channel rides the canary digest: platform drift in carving
reddens CI before pilot eyes can be blamed.

Character per seed is REAL (round-176 census, post-rebalance):
seed 42 = 40% ocean strait country w/ ridged highlands, 2 = 24%
ocean rolling green, 777 = open sea coast, 1 = a landlocked plain. The `seed_two_holds_
earth_character` test pins seed-2's measured bands — retuning these
dials SHOULD fail it; re-measure via `census -- --ignored`, then
update the bands deliberately.
