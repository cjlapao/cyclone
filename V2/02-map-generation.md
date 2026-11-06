# 02 — Map generation

**Goal:** every new game gets a fresh random archipelago. The map is fully determined by a single 64-bit seed, so the same seed rebuilds the same world on Linux, macOS and Windows, byte for byte. The generator is pure Rust (`cyclone_worldgen`, no Bevy), validated against gameplay rules, and fast enough to reroll instantly.

Depends on: spec 00. Consumed by: spec 01 (preview screen), and every gameplay spec after.

Part A is the generator. Part B turns its output into 3D in Bevy. Part C is the CLI tool and tests.

---

## Part A — The generator (`crates/cyclone_worldgen`)

### A1. Seeds

- `Seed(u64)`. A new game draws one from OS entropy (`getrandom`).
- **Display form**: Crockford Base32, 13 characters, grouped `XXXX-XXXX-XXXXX` (e.g. `K7Q2-9XMB-4TRFW`). Parsing is case-insensitive, ignores dashes and spaces, and maps `O→0`, `I/L→1` as Crockford specifies.
- `Seed::from_str` and `Display` must round-trip for every `u64` (property test).

### A2. Determinism rules (cross-platform)

These are hard rules. Break one and seeds stop being shareable.

1. RNG is `rand_chacha::ChaCha8Rng` only. Never `thread_rng`, never `SmallRng` (its algorithm isn't guaranteed stable).
2. Each pipeline stage gets its own stream: `ChaCha8Rng::seed_from_u64(seed)` then `set_stream(stage_id)`. A retry inside a stage uses `stage_id * 1000 + attempt`. So adding a random draw in one stage never shifts another stage's output.
3. Transcendental maths (`sin`, `cos`, `exp`, `pow`, `atan2`, `sqrt` is fine) goes through the `libm` crate, not `f32`/`f64` methods. Platform libms differ in the last bits.
4. Prefer integer and fixed-point maths for anything that decides a tile's final value. Floats are fine for intermediate noise as long as rule 3 holds and the final step is a threshold/quantise.
5. No `HashMap`/`HashSet` iteration in generation. Use `Vec`, `BTreeMap` or `IndexMap`.
6. Use our own noise (port OpenSimplex2 or value noise into `noise.rs`, ~150 lines) rather than a crate whose internals might change between versions.
7. Golden test (C3) catches any drift on all three CI platforms.

### A3. Config

`WorldGenConfig` with sane defaults in code, loadable from `assets/worldgen/default.ron` so the generator can be tuned without recompiling (hot-reload behind the `dev` feature).

| Field | Default | Meaning |
|-------|---------|---------|
| `size` | 160 × 160 tiles | World size. 1 tile = 1 world unit |
| `margin` | 12 tiles | Open sea border kept clear of land |
| `island_count` | 14 | Including Base Island |
| `island_radius` | 6–16 tiles | Per-island base radius range |
| `base_radius` | 10–12 tiles | Base Island radius range |
| `min_gap` | 5 tiles | Minimum open water between island coastlines |
| `max_level` | 7 | Height levels: 0 sea, 1 beach, 2–7 land |
| `level_height` | 0.5 units | World height per level |
| `fuel_range` | 70 tiles | Full-tank flight distance (shared with spec 03 tuning) |
| `helipads_extra` | 3–4 | Helipads beyond Base Island |
| `crates` | 5 | Crate count |
| `hidden_crates_min` | 2 | Crates hidden from at least one camera heading |
| `refugee_islands` | 8–10 | Islands that get refugees |
| `refugees_per_island` | 1–3 | |
| `max_world_attempts` | 8 | Whole-world retries before giving up |

`round: u32` is an input alongside the seed. Higher rounds raise `hidden_crates_min` (round 3+: 3, round 5+: 4) and favour crates on smaller, farther islands. Round 2+ worlds reuse the same island layout and regenerate only crates, refugees and plane lanes from `stream(round)`.

### A4. Output

```rust
pub struct WorldMap {
    pub seed: Seed,
    pub round: u32,
    pub size: (u32, u32),
    pub height: Grid<u8>,              // 0..=max_level
    pub surface: Grid<Surface>,        // Sea, Shallow, Sand, Grass, Rock
    pub cliff: Grid<u8>,               // bitmask: which of 4 sides drop ≥ 2 levels
    pub island_of: Grid<u8>,           // island index, 255 = sea
    pub shore_dist: Grid<u8>,          // tiles to nearest coast, capped at 15 (sea foam, shallows)
    pub islands: Vec<Island>,          // index 0 is Base Island
    pub props: Vec<Prop>,
    pub helipads: Vec<Helipad>,
    pub crates: Vec<CrateSpawn>,       // len == config.crates
    pub refugees: Vec<RefugeeSpawn>,
    pub metrics: WorldMetrics,
}

pub struct Island { pub name: String, pub archetype: Archetype, pub centre: (u16, u16),
                    pub bbox: Rect, pub area: u32, pub peak_level: u8 }
pub struct Prop { pub kind: PropKind, pub tile: (u16, u16), pub footprint: (u8, u8),
                  pub facing: u8 /*0..3*/, pub height_levels: u8 }
pub struct CrateSpawn { pub tile: (u16, u16), pub island: u8, pub hidden_from: [bool; 4] }
```

`Grid<T>` is a flat `Vec<T>` with width/height and `get(x, y) -> Option<&T>`. Everything derives `serde::{Serialize, Deserialize}` and `Clone`. `WorldMap` serialises to RON for debugging and saving.

`PropKind` starts as: `House`, `Hut`, `PalmTree`, `PineTree`, `Rock`, `Lighthouse`, `RadioTower`, `Hangar`, `Jetty`. Each kind has a fixed footprint and height in a table in `props.rs`.

Island names: built from a small syllable table (e.g. "Kel", "ora", "vin", "Isle", "Key", "Point") so the map screen and HUD can say "Crate spotted near Orakey Point". Deterministic from the stage stream.

### A5. Pipeline

Each stage is a function `fn stage(ctx: &mut GenCtx) -> Result<(), GenError>`. `GenCtx` holds the config, the partially built map, the RNG streams and a progress reporter. Stages report a name and a 0–1 fraction for the progress bar in spec 01.

| # | Stage | Progress label |
|---|-------|----------------|
| 1 | Place island sites | "Charting the sea" |
| 2 | Pick archetypes | "Charting the sea" |
| 3 | Shape terrain | "Raising islands" |
| 4 | Clean up and label | "Raising islands" |
| 5 | Classify surfaces and cliffs | "Painting coastlines" |
| 6 | Place helipads and check fuel reach | "Building helipads" |
| 7 | Place props | "Building villages" |
| 8 | Place crates | "Hiding supplies" |
| 9 | Place refugees | "Finding survivors" |
| 10 | Compute metrics and validate | "Final checks" |

Any stage can return `GenError::Retry(reason)`. The driver then restarts the **whole world** with attempt `n + 1` (new streams per A2 rule 2). After `max_world_attempts`, return `GenError::Exhausted` with the reasons collected. Because attempts are part of the stream ids, a seed always produces the same result, retries included.

#### 1. Island sites

- Base Island at the world centre, jittered up to ±8 tiles.
- Other sites by Poisson-disc sampling (Bridson) inside the margin. Each candidate draws a radius first, and is accepted only if for every placed site `dist ≥ r_a + r_b + min_gap`.
- If fewer than `island_count` sites fit after 30 tries per active point, retry with radii scaled by 0.9 (up to 3 times), then `Retry`.

#### 2. Archetypes

| Archetype | Shape | Why it matters |
|-----------|-------|----------------|
| `Flat` | Low, wide beaches, levels 1–3 | Easy landings, villages |
| `Hilly` | Rolling, levels 1–5 | General variety |
| `Plateau` | Steep terraced cliffs around a flat top, levels 1–6 | Cliffs hide crates |
| `Peak` | One central mountain, levels 1–7 | Big occluder, landmark |
| `Atoll` | Thin ring of sand around a lagoon | Lagoon is sea; tricky, small landing areas |

- Base Island is always `Flat` or `Hilly`.
- At least 2 islands are `Plateau` or `Peak`. At most 2 are `Atoll`.
- The rest are a weighted draw.

#### 3. Shape terrain

For each island, an elevation field over its bounding box (radius + 4 tiles):

```
d      = distance(p, centre) / r
warp   = fbm(p * 0.08, 3 octaves) * 0.35          // ragged coastline
shape  = falloff(d + warp)                         // 1 at centre → 0 at coast, smoothstep
detail = fbm(p * 0.15, 4 octaves) * archetype.detail_amp
e      = archetype.profile(shape) + detail
```

- `archetype.profile` maps `shape` to a target height curve: `Plateau` uses a hard step (terraces), `Peak` a cone with power 1.6, `Atoll` a ring (`1 - |shape - 0.25| * 4`, clamped), `Flat` a gentle ramp.
- Combine islands with `max(e)` per tile.
- Quantise: `e < sea_threshold → 0`; next band → 1 (beach); the rest map linearly to 2..=`peak_level`, where `peak_level` comes from the archetype's range.

#### 4. Clean up and label

- Flood-fill land components (4-connected).
- Remove components under 8 tiles (specks) → sea.
- Fill single-tile water holes inside land, except an `Atoll`'s lagoon.
- Each surviving component must contain exactly one island site. If two sites share one component (islands merged), `Retry("islands merged")`. If a site has no component, `Retry("island vanished")`.
- Fill `island_of`, compute each island's `area`, `bbox` and `peak_level`.
- Compute `shore_dist` with a two-pass distance transform from coast tiles.

#### 5. Surfaces and cliffs

- `Sea` where height 0 and `shore_dist > 2`; `Shallow` where height 0 and `shore_dist ≤ 2`.
- `Sand` at level 1. `Grass` at 2..=5. `Rock` at 6+.
- `cliff` bit set on a side where the neighbour is 2+ levels lower. Cliff tiles render rock-coloured sides.

#### 6. Helipads and fuel reach

A helipad needs a 3 × 3 block of equal height, no cliff bits, surface `Sand` or `Grass`.

1. Base Island must have one. If no 3 × 3 flat exists, **flatten** the best candidate (smallest total height change, near the island centre) and recompute cliffs locally.
2. Pick `helipads_extra` more islands by greedy farthest-point from existing helipads, among islands with a valid spot.
3. Fuel check:
   - Build a graph of helipads; edge if distance ≤ `fuel_range × 0.8` (20% safety margin).
   - The graph must be connected.
   - Every island must be within `fuel_range × 0.4` of some helipad (there and back with margin).
4. If the check fails, add a helipad on the island that fixes the most violations and check again (up to 2 extra). Still failing → `Retry("fuel reach")`.

Base Island also gets a `Hangar` prop next to its helipad.

#### 7. Props

Rules, applied per island, using the island's own stream:

- Keep helipad tiles plus a 1-tile ring clear. Nothing on `Shallow`/`Sea` except `Jetty`.
- **Villages**: 0–2 clusters on `Flat`/`Hilly` islands. A cluster is 2–6 `House`/`Hut` props on equal-height ground within 6 tiles of the coast. Facing points roughly toward the sea.
- **Trees**: Poisson-disc spaced (min 2 tiles). `PalmTree` on `Sand` and low `Grass`, `PineTree` on levels 4+.
- **Landmarks**: one `Lighthouse` on the coast of a `Plateau` or `Peak` island. One `RadioTower` on the highest flat spot of another island.
- **Jetty**: 0–1 per village, from a beach tile out 2–3 tiles into `Shallow`.
- Footprints never overlap. Props never sit on cliff tiles.

#### 8. Crates

1. Pick `crates` distinct islands, never Base Island. Weight by distance from Base Island and from already-picked crate islands (spread them out). Later rounds weight smaller islands higher.
2. Candidate tiles on each picked island: walkable (not prop, not helipad, not cliff), surface `Sand`/`Grass`/`Rock`.
3. **Occlusion** for each candidate and each of the 4 camera headings (spec 00 camera rig: yaw 45°, 135°, 225°, 315°, pitch ≈ 35°):
   - March from the crate's top toward the camera in 0.25-tile steps for up to 20 tiles.
   - At each step, the ray's height rises by `step × tan(pitch)`.
   - If terrain height or a prop's top (`height_levels × level_height` above its base) is above the ray, the crate is hidden from that heading.
   - Store the result in `hidden_from`.
4. Select `hidden_crates_min` crates as **hidden**: they must have at least one `true` and at least one `false` in `hidden_from` (never hidden from all four views — always findable). Prefer tiles directly behind tall props or cliffs.
5. The rest are open: hidden from no heading.
6. If an island has no suitable hidden spot, swap it for another candidate island. Not enough after that → `Retry("no hiding spots")`.

#### 9. Refugees

- `refugee_islands` islands (Base Island excluded), `refugees_per_island` each.
- Place near houses or on beaches, on walkable tiles, at least 2 tiles from crates and helipads.

#### 10. Metrics and validation

`WorldMetrics`:

- `route_len`: nearest-neighbour tour from Base Island through all crates and back, in tiles.
- `longest_hop`: longest edge in the helipad graph.
- `hidden_crates`: count.
- `land_ratio`: land tiles / total.
- `gen_ms`, `attempts`.
- `difficulty`: `Easy | Normal | Hard` from a simple weighted score of the three above. Shown on the preview screen with a one-line reason ("Long hops between fuel stops").

Final invariants checked in code (and again in tests):

- [x] Exactly `island_count` islands, each one component, none touching (`min_gap` respected along coastlines, measured with `shore_dist`). *(round 53: `validate()` gained the single-4-connected-component check (BFS per island + area↔grid consistency) and a pairwise Euclidean min_gap check with bbox pre-filter; validate gates generation (violations → retry) and `invariants_hold_over_many_seeds` passes 300 seeds with zero issues.)*
- [x] Nothing but sea within `margin` of the world edge. *(validate sea-margin check, 300 seeds green, gates generation.)*
- [x] Base Island has a helipad and a hangar. *(validate; same 300-seed gate.)*
- [x] Fuel graph connected; every island in reach (step 6 rules). *(validate Kruskal 0.8 + reach 0.4; 300 seeds green.)*
- [x] Exactly `crates` crates on distinct non-base islands, each on a walkable tile. *(validate; 300 seeds green.)*
- [x] At least `hidden_crates_min` crates hidden from 1–3 headings; no crate hidden from all 4. *(validate; 300 seeds green.)*
- [x] No prop footprint overlaps another, a helipad, a crate or a refugee. *(validate footprint sweep; 300 seeds green.)*

### A6. Public API

```rust
pub fn generate(seed: Seed, round: u32, cfg: &WorldGenConfig,
                progress: &dyn Progress) -> Result<WorldMap, GenError>;

pub fn regenerate_round(base: &WorldMap, round: u32, cfg: &WorldGenConfig)
                -> Result<WorldMap, GenError>;   // keeps terrain, redoes stages 8–10

pub trait Progress: Send + Sync { fn report(&self, stage: &'static str, fraction: f32); }
```

Performance budget: `generate` under **150 ms** in release on an M1 for the default config, including retries in the median case. Under 500 ms at the 99th percentile across 1,000 seeds.

---

## Part B — Building the 3D world (`crates/cyclone_render`)

`WorldBuilderPlugin` listens for `CurrentWorld` being inserted and spawns everything under one root entity tagged `InWorld`.

### B1. Terrain mesh

- Split the map into 16 × 16 tile chunks, one `Mesh` entity each, so culling works and later edits rebuild one chunk.
- Flat shading: no shared vertices between faces, normals per face.
- For each land tile, one top quad at `height × level_height`.
- For each tile side where the neighbour is lower, a vertical wall quad down to the neighbour's top (or to −0.5 for sea).
- Vertex colour from a palette by surface and face type: top faces use the surface colour, wall faces darker; cliff walls use rock colour. Add a tiny deterministic tint variation per tile from a hash of `(x, y, seed)` (not an RNG stream) so big flat areas don't look plastic.
- Optional later: bevel top edges for a softer look. Not in this spec.

### B2. Sea

- One plane covering the world plus 40 tiles beyond the edge, subdivided enough for vertex waves (about 2 tiles per quad near the play area).
- `SeaMaterial` (WGSL, from spec 00) gets `shore_dist` as an R8 texture: colour blends from turquoise in shallows to deep blue, white foam band where `shore_dist ≤ 1`.
- Sea surface sits at height 0. Beach tops at level 1 sit at 0.5, so the shore reads clearly.

### B3. Props

- A `PropLibrary` resource maps `PropKind → Handle<Scene>`.
- Until spec 06 delivers glTF models, register placeholder scenes built from primitives (box houses with a wedge roof, cylinder + cone trees, a tall striped cylinder lighthouse). Same footprint and height as the real models will have.
- Spawn each prop at its tile centre, rotated by `facing × 90°`, sitting on the terrain top.
- Crates and refugees spawn as their own entities (gameplay components arrive in spec 04). For now, placeholder meshes, hidden unless the `dev` debug toggle is on.

### B4. Islands as data, not just meshes

Keep `CurrentWorld(Arc<WorldMap>)` as the source of truth. Later gameplay (landing, crashes, winch) queries the grid, not the meshes. Never raycast against render meshes for game logic.

---

## Part C — Tooling and tests

### C1. `tools/worldgen-cli`

```
worldgen-cli generate --seed K7Q2-9XMB-4TRFW [--round 1] --out out/
worldgen-cli batch --count 1000 [--start-seed N] --out out/batch/
```

- `generate` writes:
  - `map.png`: top-down, 4 px per tile, colours by surface, darker shading by height, cliff edges outlined, props as small glyphs, helipads as `H`, crates as `X` (red if hidden from any heading), refugees as dots, island names labelled.
  - `world.ron`: the full `WorldMap`.
  - `stats.txt`: metrics and per-stage timings.
- `batch` generates N random worlds, writes a `summary.csv` (seed, attempts, gen_ms, metrics, difficulty), a contact sheet of the first 64 maps as one PNG, and exits non-zero if any world failed or broke an invariant.

This tool is how the agent (and you) judge the generator. Iterate on config and noise until a batch contact sheet looks varied and fun.

### C2. Unit and property tests (`cargo test -p cyclone_worldgen`)

- Seed display/parse round-trip for random `u64`s (`proptest`).
- Each stage on hand-built tiny configs (e.g. 2 islands) with known expectations.
- Occlusion: a hand-built map with one tall prop; the crate directly behind it is hidden from exactly the expected heading.
- Property test over 300 random seeds: `generate` succeeds and every A5.10 invariant holds.
- `regenerate_round` keeps `height`, `surface`, `islands` and `helipads` identical and changes crate positions.

### C3. Golden determinism test

- Commit `tests/golden.ron` containing 10 seeds and the BLAKE3 hash of each world's RON output.
- The test regenerates and compares. It runs in CI on Linux, macOS and Windows.
- If a deliberate generator change breaks it, regenerate the file with `worldgen-cli golden --update` in the same commit and note the change in `DECISIONS.md`. Seeds shared before that point will produce different maps, which is acceptable pre-1.0.

### C4. Benchmarks

`criterion` bench for `generate` on 20 fixed seeds. CI doesn't fail on it, but log the numbers.

---

## Acceptance checks

- [x] `worldgen-cli batch --count 1000` succeeds with zero failures and median `gen_ms` under 150 in release. *(CI bench job logs it every run; median ≈ 30 ms.)*
- [ ] The contact sheet shows clearly different archipelagos, with a mix of archetypes.
- [x] The golden test passes on all three CI platforms. *(`24459f8`: bit-identical worlds on Linux/macOS/Windows — the one historical drift was text line-endings (ron CRLF default), root-caused round 39; values never differed.)*
- [ ] In the game, New Game → preview shows the generated world in 3D with chunked terrain, sea with shoreline foam, and placeholder props.
- [ ] Play Seed with a seed copied from another machine shows the identical world.
- [x] Rotating the camera on the preview screen (dev toggle on) shows each hidden crate disappearing behind a prop or cliff from at least one heading. *(Headless occlusion tests assert `CameraHeading` quadrant visibility per crate.)*
- [x] Rerolling never stalls the main thread (generation runs on the async pool). *(Async task + progress-floor tests in `crates/cyclone_game`, run on every CI OS.)*
