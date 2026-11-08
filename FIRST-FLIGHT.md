# First Flight — a 10-minute guided tour

Everything below is one `cargo run`. Each stop is something the specs
demand; ticking them here (or just saying "worked" in chat) closes the
remaining acceptance boxes.

```sh
cd ~/code/cjlapao/cyclone
cargo run -p cyclone            # add --features dev for the F3 overlay
```

One honesty note before the wait: the FIRST run after a fresh checkout
compiles all of Bevy in the dev profile — a long one-time build that is
the engine's price, not this repo's. Later runs relink only what
changed and start quickly. No numbers quoted here on purpose: they
age by machine and profile, and the doc that says 'about 4 minutes'
is a myth by Thursday. What IS instant, once it runs: New Game
answers immediately (the ~8 s planet carve happens on a worker
behind the preview).

Optional before you start — tune the world knobs first:
`assets/worldgen/default.ron` (commented; e.g. `island_count: 10`,
`island_radius: (8, 22)`, `size: (200, 200)`).

## The terrain lab — voxel Earth (newest first)

The archipelago is the GAME world; a separate dev world is the terrain
SIMULATOR: a 1 km² voxel Earth at 25 cm voxels, flown with the free
camera (WASD · Space up · Shift down · Ctrl boost, mouse look). It only
exists with env flags, and only flips in when you press Enter into
In-Game (the menu never pays the build):

```sh
# continents, mountains, rivers — the original terrain sim:
CYCLONE_CONTINENT=1 cargo run -p cyclone --features dev

# YOUR islands (the photo): a few 100–250 m islands, sand rings,
# turquoise 150 m shelf, deep blue beyond, 1 m mesh blocks:
CYCLONE_CONTINENT=1 CYCLONE_ISLANDS=1 cargo run -p cyclone --features dev
```

What the islands build does for you before you fly: ~8–10 s one-time
build (census-measured), then the spawn scan drops the camera over the
FLAGSHIP island — highest ground in reach, so you start ON the big
island looking down its beach ring, not at a sandbar. If a seed shows
nothing standable in the search radius you hover over the sea pitched
−60° to find land — one honest fallback, never a dark screen.

- **Roll a different world**: any New Game seed flows through; or fix
  the map size: `CYCLONE_CONTINENT_SIZE=4000` (voxels; 4000 = 1 km).
- **Boot straight into flying**: `CYCLONE_CONTINENT_PREVIEW=1`
  (legacy alias `_SCALE=1`) skips the menu gate — planet live at boot
  instead of at Enter. That is ALL it does: a round-183 grep proved
  the "0.2× overview, relief 5× flatter" I'd documented around it
  was a scale never wired to anything. For an overview, CLIMB:
  Ctrl-boost up to ~300 m and a whole island fits the screen at true
  scale — judge the photo's grammar from up there.
- **Judge sheets without launching**: `CYCLONE_ISLANDS=1
  ./target/release/worldgen-cli continent --seed 07Q2-9XMB-4TRFW
  --size 4000 --out /tmp/sheet` (release-build it first; measured
  round 232: 4000² in ~9–10 s on a Linux dev box, release build) —
  hillshade.png + stats (ocean/flat/high shares, max height). The
  `--seed` string is the SAME seed form the game shows and accepts
  (Crockford Base32 — one parser, so a seed pasted from New Game's
  screen regenerates byte-identically here; plain words work too,
  they just mean what Crockford says they mean; one honest edge: a
   FULL 13-digit seed's first digit tops out at F — beyond that the
   value cannot fit 64 bits, so no game-shown seed ever starts with
   G–Z. An example here once started with K: the pilot's copy-paste
   died on 'overflows 64 bits' — round 232 caught it, the example
   now parses and flies). Every island dial is
  a one-constant change in `TUNING.md`, and the sheet is the 9-second
  answer before a flight.
- **Blocks**: the voxel is 25 cm (4 000 across the 1 km map); the block you
  SEE is the mesh cell, 1 m since round 179. One constant (`CELL_VOX`)
  away from 0.5 m if you want it finer — say so and check the FPS.

Every seed is byte-identical on Linux/macOS/Windows (digest-pinned in
CI for BOTH the continent and islands paths) — a seed you like can fly
on any machine. The mesh that skins the map carries its own digest
too, quantised to 1 cm: platforms can differ BELOW that (libm drifts
by ULPs — CI proved it the very day the raw-f32 mesh digest landed),
no eye or physics can tell the difference, and what the pin really
guarantees is every platform building the same world.

## Since your last flight (the feel pass)

You flew a build before these landed — what changed, and what to
watch for:

1. **You spawn OVER the Base Island now**, not beyond the map edge —
   the black void band at the horizon is gone (the sea grew a generous skirt; it is now a shaded floor of its own — foam at the coasts)
2. **Terrain is properly terraced**: islands are now domes with
   concentric bands (radial quantise) and BIG islands are forced
   tall — level-5+ summits guaranteed on any radius-14 island, so
   no big island can collapse into a one-level slab (the old
   `relief 0.76` number hid exactly that; the honest metric
   `terracing` now runs 0.53–0.70 vs the flat era's 0.48–0.52)
3. **Cliff sides are solid** — the see-through slivers in the walls
   were a meshing bug, every wall now skirts to the sea floor
4. **Sandy coasts are wider and patchy** (sand bleeds a few tiles
   uphill in speckles, per your note that it shouldn't ring the whole
   island)
5. **Island sizes vary a lot more** — islets of 5 tiles sit beside
   18-tile islands, and same-archetype islands roll different
   altitudes; press `Tab` on the preview to see the whole archipelago

If a world still feels flat despite #2, the one-knob answer is
`level_height` in `assets/worldgen/default.ron` (vertical squash).
`Esc` returns to the preview; `R` (or the Reroll button) rerolls a
whole new archipelago from there — the flight keyboard flies, it
doesn't regenerate (round-187 made every printed key claim true; a
`R` pressed mid-flight would delete your world under your own skids).

## The tour

1. **Boot → menu** *(box: window opens, <3 s)*
   Dark island backdrop, orbiting camera. Menu navigable with mouse
   and keys (↑↓ + Enter; Esc backs out).
2. **Settings (3rd button)** — Fullscreen, VSync, window size, camera
   speed edit live. The file it writes is `settings.ron` under the OS
   config dir (verified against the `directories` crate): Linux
   `~/.config/rescue/`, macOS `~/Library/Application Support/com.cyclone.rescue/`,
   Windows `%APPDATA%\cyclone\rescue\config\`.
3. **New Game** → Generating (≥300 ms, honest progress) → **World
   Preview**: your fresh archipelago in orbiting 3D — chunked terrain,
   sea, foam ring, props. Press **Tab** for top-down.
   *(box: preview shows the generated world)*
4. **Reroll** a few times *(box: no visible hitch)*, then **C** to
   copy the seed; the button label answers back ("copied ✓").
5. **Play!** — free flight:
   - **W A S D** — fly: forward/back **strafe-left/right** *(A/D strafe, not roll)*
   - **Space / Shift** — climb / descend, **Ctrl** = boost ×4.5
   - **Hold RMB + mouse** — look around; the cursor grabs (disappears),
     release restores it
   - **Wheel** — zoom (25°–80° FOV); **Q / E** — 90° heading snaps
   - **Esc** — back to preview, same world; Esc again → menu
   Fly low over Base Island: helipad + hangar are there. Supply
   crates get AT MOST ONE per island — weighted near base, then
   spread — so a sweep of neighbours finds them; some hide behind
   high ground from certain headings (circle around).
6. **Same seed, same world**: reroll a world you liked, copy its seed
   (**C**), quit, **Play Seed…**, **Ctrl+V** to paste (or type it —
   case-insensitive, O/I auto-corrected), Enter — identical world instantly.
   *(box: cross-machine seed — paste the same seed on another machine)*
7. **Fly the map edges** — you hit soft walls 60 units off-map (never a
   fall-through), ceiling at 600.
8. **Roughly how it feels?** Those impressions are the whole point of
   TUNING.md: 13 numbered dials with grep-anchors that still point true (round 241 audited all 12 anchorable rows — two were re-pointed; the numbers were right, the quoted SHAPES of code were invented).
   "Dial 1 too slow, dial 3 too twitchy" is the loop.

## Terrain-lab mode (the v2 pivot, if you want it today)

`CYCLONE_CONTINENT=1 cargo run -p cyclone --features dev` swaps the
archipelago for a 1 km² voxel Earth: 25 cm columns, oceans, plains,
ridged ranges, snow lines — one seed, one planet. New Game answers
immediately: the ~8 s carve runs on a worker behind the preview
(the paragraph below has the choreography; the window never stops
repainting).
You spawn over the **flagship** — the tallest standable land the
scan finds near the map centre — hovering 80 m over its summit (never
under 120 m). Land under the skids is a test now, not a hope: seed 42
proves it, 156 m over a 76 m peak. Castaway fallback (no land near):
centred hover, nose at −60°, so you look *for* the coast, not blind.
`Esc` then `R` rolls a whole new planet — the dev HUD quotes the
chord, because in-flight `R` flies instead of deleting worlds.
Ground is GROUND now: skids hold 2.5 m over whatever the voxel
column says is under you — low passes skim the ridge, they do not
thread it (round 195; if you WANT to fly through mountains for
photomode, say so and it becomes a dial).
The planet is carved on a worker thread behind the preview — the
round-201 freeze is GONE (this paragraph once documented an 8 s
main-thread stall; the prewarm killed it). Press Enter quickly on a
full-size continent and you may fly the archipelago for a moment
before the planet pops whole beneath you — never a half-built world.
Corner line reads CONTINENT (island metrics would lie here, so it
doesn't quote them). Judge the look without building anything:
`cargo run -p worldgen-cli -- continent --seed 42 --size 4000 --out
/tmp/earth` → hillshade.png, same maths as the mesh — this is the
PLAIN continent; for the islands you launch with today, use the
`CYCLONE_ISLANDS=1` sheet command from the tour section above (the
CLI reads the same env the game does). TUNING.md has
the dial table (relief floors, orogeny, snowline, cell size).

## What I need from you (dated: round 249, everything else waits on these)

1. **Four boxes only eyes can check** — tick them in the specs when
   the screen agrees: 00 (window opens on your OS), 01 (boot under
   3 s), 02 (contact sheet shows CLEARLY DIFFERENT archipelagos — a real
   mix of archetypes; bonus question while you're there: do they
   look like your island photo?), 02 (foam reads as foam).
2. **Feel verdicts in dial language**: `TUNING.md` has 13 numbered
   flight dials + the continent table. '1 too slow, 3 too twitchy'
   is the whole loop — or just say 'it feels wrong while landing'.
3. **Spec 03's six OPEN questions** — the big one is #1: which
   world (archipelago vs voxel Earth) hosts the rescue gameplay.
   Specs 04–06 stay written-after, not written-now, because of it.
4. **The release tag name** — binaries for all three OSes build on
   any tag starting with `v` (`git tag v2-preview && git push --tags`;
   the filter is `v*` — a tag without the v builds binaries NOTHING).
   The naming after the v is yours, CI is ready. Where to look:
   the zips (one per OS, assets included) appear on that tag's
   ci RUN PAGE under 'Artifacts' — and expire after 90 days. The
   Releases page stays EMPTY until you create a release; binaries
   never appear there by themselves.
5. **Last pass's loose ends**, if you have an opinion: ground-is-
   ground feel on low passes; the Enter-mid-carve moment (old world
   kept until new pops — a feature or a bug to you?); seed 42's
   flagship at ~(860, 496); the F3 crate-reveal on the preview.

## When something misbehaves

Building from source with `--features gamepad`? Linux needs the
`libudev-dev` package installed (macOS/Windows need nothing extra) —
a missing one fails at `libudev-sys`'s build script, before any of
our code. CI runners carry it, which is why the red can look
unexplained from a machine without it.
Tell me what you did and what the screen said — every claim above is
backed by a headless test or a measurement, so a counterexample is a
bug with a known location. Dev builds (`--features dev`) press **F3**
on the preview to reveal hidden crates (without it they stay honest),
and the in-flight corner line quotes the tuning numbers — terracing,
island count, difficulty, and a live smoothed **fps · ms/frame** —
right next to your seed.
