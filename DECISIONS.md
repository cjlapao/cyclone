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
