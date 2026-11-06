# First Flight — a 10-minute guided tour

Everything below is one `cargo run`. Each stop is something the specs
demand; ticking them here (or just saying "worked" in chat) closes the
remaining acceptance boxes.

```sh
cd ~/code/cjlapao/cyclone
cargo run -p cyclone            # add --features dev for the F3 overlay
```

Optional before you start — tune the world knobs first:
`assets/worldgen/default.ron` (commented; e.g. `island_count: 10`,
`island_radius: (8, 22)`, `size: (200, 200)`).

## Since your last flight (the feel pass)

You flew a build before these landed — what changed, and what to
watch for:

1. **You spawn OVER the Base Island now**, not beyond the map edge —
   the black void band at the horizon is gone (sea plane got bigger)
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
`R` rerolls a whole new archipelago at any time.

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
   TUNING.md: 13 numbered dials with file:line that still points true.
   "Dial 1 too slow, dial 3 too twitchy" is the loop.

## When something misbehaves

Tell me what you did and what the screen said — every claim above is
backed by a headless test or a measurement, so a counterexample is a
bug with a known location. Dev builds (`--features dev`) press **F3**
on the preview to reveal hidden crates (without it they stay honest),
and the in-flight corner line quotes the tuning numbers — terracing,
island count, difficulty, and a live smoothed **fps · ms/frame** —
right next to your seed.
