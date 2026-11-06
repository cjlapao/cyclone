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

## The tour

1. **Boot → menu** *(box: window opens, <3 s)*
   Dark island backdrop, orbiting camera. Menu navigable with mouse
   and keys (↑↓ + Enter; Esc backs out).
2. **Settings (3rd button)** — Fullscreen, VSync, window size, camera
   speed edit live. The file it writes: `~/.config/rescue/settings.ron`.
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
   Fly low over Base Island: helipad + hangar + the five supply crates
   are there (some hidden from certain headings — circle around).
6. **Same seed, same world**: reroll a world you liked, copy its seed,
   quit, Play Seed, paste — identical world on the spot.
   *(box: cross-machine seed — paste the same seed on another machine)*
7. **Fly the map edges** — you hit soft walls 60 units off-map (never a
   fall-through), ceiling at 600.
8. **Roughly how it feels?** Those impressions are the whole point of
   TUNING.md: 13 numbered dials with file:line that still points true.
   "Dial 1 too slow, dial 3 too twitchy" is the loop.

## When something misbehaves

Tell me what you did and what the screen said — every claim above is
backed by a headless test or a measurement, so a counterexample is a
bug with a known location. The F3 overlay (dev feature) prints FPS,
frame ms, world seed and camera position.
