# 01 — Main menu and world preview

**Goal:** the game boots into a title screen with a live 3D island turning slowly behind the menu. From there the player can start a new game on a freshly generated random map, preview and reroll that map before flying, or type in a seed to replay one.

Depends on: spec 00. Uses the generator from spec 02 (build them together; the preview screen is the easiest way to eyeball generator output).

## Screen flow

```
Boot ──> MainMenu ──New Game──> Generating ──> WorldPreview ──Start──> InGame
            │  ▲                    ▲              │   │
            │  └────────Back────────┼──────────────┘   │
            │                       └──Reroll / Seed───┘
            ├──Settings (overlay panel)
            ├──High Scores (overlay panel)
            └──Quit
```

## UI technology

- Build menus with `bevy_ui` plus our own widget layer in `cyclone_ui`. No egui in the shipped game (it's fine behind the `dev` feature for debug panels).
- `cyclone_ui` provides: `MenuButton`, `MenuList` (keyboard/gamepad focus), `Slider`, `Toggle`, `TextInput` (for the seed), `Panel`, and a `Theme` resource (fonts, colours, sizes).
- Every widget works with mouse, keyboard and gamepad. Focus moves with `Action::Up/Down`, activates with `Confirm`, backs out with `Back`.
- Text scales with window height so the menu reads the same at 720p and 4K.

## Visual design

- Bold, clean, modern. One display font for titles (free licence, e.g. from Google Fonts, bundled in `assets/fonts/`) and one plain sans for body text.
- Title wordmark `CYCLONE RESCUE` top-left, menu list under it, left-aligned.
- **Background**: a real island from the generator (a small one-island config, random seed each boot) on the sea shader, with the camera orbiting slowly (one turn every 60 s) and a little helicopter model circling it. Until spec 06 lands, use placeholder primitives (capsule body, box rotor).
- A subtle dark gradient behind the menu text so it stays readable over the scene.
- Button focus: accent colour fill and a short scale-up tween (1.0 → 1.05, 120 ms).
- Version string and seed of the background island bottom-right in small text.

## Main menu items

| Item | Does |
|------|------|
| **New Game** | Draws a fresh random seed (spec 02), goes to `Generating`, then `WorldPreview` |
| **Play Seed…** | Opens a text field. Accepts the seed's display form (see spec 02). Invalid input shakes the field and shows "Not a valid seed". Valid → `Generating` with that seed |
| **Settings** | Panel: master volume, music volume, SFX volume, fullscreen toggle, window size (windowed only), vsync toggle, camera rotation speed. Changes apply live and save on close |
| **High Scores** | Panel: top 10 with name, score, round reached, date, seed. Selecting an entry offers "Play this seed" |
| **Quit** | Exits. Hidden on platforms where quitting isn't expected (none today, but keep the flag) |

`Back` on the main menu does nothing. `Back` in a panel closes it.

## Generating screen

- Generation runs on Bevy's `AsyncComputeTaskPool`, never on the main thread.
- The generator reports progress through a shared `Arc<AtomicU32>` or channel with stage names from spec 02 ("Placing islands", "Shaping terrain", "Placing helipads", …).
- Show a progress bar and the current stage name. Keep the background scene animating.
- If generation takes under 300 ms, still show the screen for 300 ms so it doesn't flicker.
- If the generator returns an error (spec 02 caps retries), show "Couldn't build a world from this seed" with Retry (new random seed) and Back.

## World preview screen

The reason this exists: it lets you see each random map before committing, and it's the main tool for judging the generator.

- **Left two-thirds**: the full 3D world, camera zoomed out to fit the whole archipelago, slowly orbiting. The player can rotate with Q/E and zoom with the wheel.
- **Right third**: an info panel:
  - Seed (display form) with a "Copy" button (clipboard via `arboard`)
  - Island count, Base Island highlighted
  - Helipad count
  - Difficulty estimate from spec 02's metrics (e.g. "Long hops between fuel stops")
- Toggle (`Tab`): switch the 3D view to a flat top-down 2D map, the same one the in-game map screen will reuse in spec 05.
- Crate and refugee positions are **hidden** here by default. A `dev`-feature-only toggle (`F3`) shows them for debugging.

Actions:

| Key | Action |
|-----|--------|
| `Enter` / Start | **Fly** → `InGame` with this world |
| `R` / Y | **Reroll** → new random seed, `Generating` |
| `Esc` / B | **Back** → `MainMenu` |
| `C` | Copy seed |

## Data handoff

- `Generating` inserts the finished world as a `Resource`: `CurrentWorld(Arc<WorldMap>)` (type from spec 02) plus `GameSeed`.
- `WorldPreview` spawns the world's render entities from `CurrentWorld`. When moving to `InGame`, **keep** those entities rather than rebuilding: mark them with an `InWorld` component, not `DespawnOnExit(WorldPreview)`, and despawn them on exit from `InGame` or on `Back`/`Reroll`.

## Audio

- Menu loop music (placeholder silence is fine until the audio spec).
- Focus move tick, confirm click, back click. Short and quiet.

## Tests

- Unit-test `cyclone_ui` focus navigation: wrapping, skipping disabled items.
- Unit-test the seed text parser (lives in `cyclone_worldgen`, spec 02).
- A Bevy headless app test (`MinimalPlugins` + our state plugin) that drives `MainMenu → Generating → WorldPreview → MainMenu` with synthetic action events and asserts the state each step and that no menu entities leak.

## Acceptance checks

- [ ] Boot to a usable main menu in under 3 s in release on an M1 Mac.
- [x] Whole menu usable with keyboard only, mouse only, and gamepad only. *(round 49–54 evidence: flow.rs drives Boot→Menu→Generate→Preview by (a) key messages only, (b) Activated button-click messages only, (c) GamepadButtonStateChangedEvent injections only — each modality end-to-end, all three OSes in CI. Physical-device confirmation is the player's box to close.)*
- [x] New Game produces a different world every time (seed differs, preview differs). *(`new_game_seeds_differ` + CI batch uniqueness.)*
- [x] Play Seed with a copied seed reproduces the exact same world. *(`play_seed_shows_identical_world` asserts terrain/crates/helipads bit-identical; goldens now pass on all three CI OSes.)*
- [x] Reroll ten times in a row with no frame hitch over 50 ms on the main thread. *(round 55: `reroll_hitch_measurement` (flow.rs, --ignored) rerolled 10×, timed all 5789 frames of the offscreen app — worst 3.6 ms, all of it inside the synchronous rebuild frame. GPU upload isn't represented headless; final eyeball remains with the player, margin is ~90%.)*
- [x] Settings changes apply instantly and persist across restarts. *(`settings_panel_opens_edits_live_and_closes` + engine disk round-trip tests.)*
- [x] No entities from MainMenu remain after entering WorldPreview (check with an entity count debug overlay). *(`flow.rs` asserts `leftovers(...) == 0` on both transitions.)*
