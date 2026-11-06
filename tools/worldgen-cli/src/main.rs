//! worldgen-cli — spec 02 Part C: judge the generator without running the
//! game. `generate` dumps a labelled PNG + world.ron + stats.txt, `batch`
//! sweeps seeds into a contact sheet + summary.csv, `golden` maintains the
//! determinism hashes.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use ab_glyph::FontArc;
use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use cyclone_worldgen::config::WorldGenConfig;
use cyclone_worldgen::map::{
    CLIFF_E, CLIFF_N, CLIFF_S, CLIFF_W, Difficulty, PropKind, Surface, WorldMap,
};
use cyclone_worldgen::pipeline::validate;
use cyclone_worldgen::seed::Seed;
use cyclone_worldgen::{ProgressNoop, generate};
use image::{ImageBuffer, Rgba, RgbaImage};
use imageproc::drawing::{draw_text_mut, text_size};
use rayon::prelude::*;

/// Font is bundled with the game so the CLI works from any cwd.
const FONT: &[u8] = include_bytes!("../../../assets/fonts/LiberationSans-Bold.ttf");
const PX: u32 = 4; // pixels per tile in map.png

#[derive(Parser)]
#[command(name = "worldgen-cli", about = "Cyclone worldgen tooling (spec 02 C)")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// One world → map.png + world.ron + stats.txt in --out
    Generate {
        #[arg(long)]
        seed: String,
        #[arg(long, default_value_t = 1)]
        round: u32,
        #[arg(long)]
        out: PathBuf,
        /// WorldGenConfig RON (default: built-in defaults).
        #[arg(long)]
        config: Option<PathBuf>,
    },
    /// N worlds → summary.csv + contact-sheet.png; exits non-zero if any
    /// world fails or breaks an invariant.
    Batch {
        #[arg(long)]
        count: usize,
        /// Deterministic sweep of start+i seeds; omit for random seeds.
        #[arg(long)]
        start_seed: Option<u64>,
        #[arg(long)]
        out: PathBuf,
    },
    /// Verify (or `--update`) the golden determinism file (spec 02 C3).
    Golden {
        #[arg(long)]
        update: bool,
    },
}

fn main() -> Result<()> {
    let font = FontArc::try_from_slice(FONT).context("bundled font")?;
    match Cli::parse().cmd {
        Cmd::Generate {
            seed,
            round,
            out,
            config,
        } => {
            let seed: Seed = seed.parse().with_context(|| format!("bad seed {seed:?}"))?;
            let cfg = match config {
                Some(path) => {
                    let text = fs::read_to_string(&path)?;
                    WorldGenConfig::from_ron(&text)?
                }
                None => WorldGenConfig::default(),
            };
            let map = generate(seed, round, &cfg, &ProgressNoop)?;
            write_world(&out, &map, &font)?;
            println!(
                "{seed} {}ms {} islands, {:?} → {}",
                map.metrics.gen_ms,
                map.islands.len(),
                map.metrics.difficulty,
                out.display()
            );
        }
        Cmd::Batch {
            count,
            start_seed,
            out,
        } => {
            fs::create_dir_all(&out)?;
            // Attempt budget 24, not the live-game 8: the GAME answers
            // an unlucky roll by rerolling a fresh seed (up to 10x),
            // which a per-seed batch cannot do — patience-per-seed is
            // its honest equivalent (P(exhausted at 24) ~= p^3 given
            // measured p=1.1% at 8; 1000-seed expected failures 0.001).
            // At default-8 the batch exhausts ~1% of seeds (measured,
            // round 112: 11/1000) — a supported outcome, not a bug.
            let cfg = WorldGenConfig {
                max_world_attempts: 24,
                ..WorldGenConfig::default()
            };
            let seeds: Vec<Seed> = match start_seed {
                Some(n) => (0..count as u64).map(|i| Seed(n.wrapping_add(i))).collect(),
                None => (0..count).map(|_| Seed::random()).collect(),
            };
            let results: Vec<(Seed, Result<WorldMap, String>)> = seeds
                .par_iter()
                .map(|s| {
                    let t = Instant::now();
                    match generate(*s, 1, &cfg, &ProgressNoop) {
                        Ok(mut m) => {
                            m.metrics.gen_ms = t.elapsed().as_millis() as u64;
                            let issues = validate(&m, &cfg);
                            if issues.is_empty() {
                                (*s, Ok(m))
                            } else {
                                (*s, Err(format!("invariants: {}", issues.join("; "))))
                            }
                        }
                        Err(e) => (*s, Err(format!("{e:?}"))),
                    }
                })
                .collect();

            let ok: Vec<&WorldMap> = results
                .iter()
                .filter_map(|(_, r)| r.as_ref().ok())
                .collect();
            let fails: Vec<(&Seed, &String)> = results
                .iter()
                .filter_map(|(s, r)| r.as_ref().err().map(|e| (s, e)))
                .collect();

            let mut csv = String::from(
                "seed,attempts,gen_ms,route_len,longest_hop,hidden_crates,land_ratio,terracing,islands,difficulty\n",
            );
            for m in &ok {
                let mt = &m.metrics;
                csv.push_str(&format!(
                    "{},{},{},{:.2},{:.1},{},{:.4},{:.2},{},{:?}\n",
                    m.seed,
                    mt.attempts,
                    mt.gen_ms,
                    mt.route_len,
                    mt.longest_hop,
                    mt.hidden_crates,
                    mt.land_ratio,
                    mt.terracing,
                    m.islands.len(),
                    mt.difficulty
                ));
            }
            fs::write(out.join("summary.csv"), &csv)?;

            contact_sheet(&out.join("contact-sheet.png"), &ok, &font)?;

            let mut ms: Vec<u64> = ok.iter().map(|m| m.metrics.gen_ms).collect();
            ms.sort_unstable();
            let median = ms.get(ms.len() / 2).copied().unwrap_or(0);
            let mut hist: BTreeMap<&str, usize> = BTreeMap::new();
            for m in &ok {
                let k = match m.metrics.difficulty {
                    Difficulty::Easy => "easy",
                    Difficulty::Normal => "normal",
                    Difficulty::Hard => "hard",
                };
                *hist.entry(k).or_default() += 1;
            }
            // Why seeds get re-rolled — the tuning signal (TUNING.md):
            // strip the attempt prefix and bucket by validator verdict.
            let mut rejects: std::collections::BTreeMap<String, u32> = Default::default();
            for m in &ok {
                for r in &m.metrics.rejections {
                    for key in reject_keys(r) {
                        *rejects.entry(key).or_default() += 1;
                    }
                }
            }
            println!("batch rejects: {rejects:?}");
            println!(
                "batch: {}/{} ok, median gen_ms {median} ms, difficulty {hist:?}, sheet+csv in {}",
                ok.len(),
                count,
                out.display()
            );
            for (s, e) in fails.iter().take(5) {
                println!("  FAIL {s}: {e}");
            }
            if !fails.is_empty() {
                std::process::exit(1);
            }
        }
        Cmd::Golden { update } => {
            let rows = golden_rows()?;
            let path = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../crates/cyclone_worldgen/tests/golden.ron");
            let rendered: Vec<String> = rows
                .iter()
                .map(|(s, h)| format!("    (seed: \"{s}\", hash: \"{h}\"),"))
                .collect();
            if update {
                fs::write(
                    &path,
                    format!(
                        "// BLAKE3 of WorldMap::to_ron() per seed (spec 02 C3).\n// Regenerate: cargo run -p worldgen-cli -- golden --update\n(\n{}\n)\n",
                        rendered.join("\n")
                    ),
                )?;
                println!("updated {}", path.display());
            } else {
                let want = fs::read_to_string(&path).with_context(|| {
                    format!("{} missing — run `golden --update`", path.display())
                })?;
                let want_rows: Vec<String> = want
                    .lines()
                    .filter(|l| l.contains("seed:"))
                    .map(|l| l.trim().trim_end_matches(',').to_string())
                    .collect();
                let have: Vec<String> = rows
                    .iter()
                    .map(|(s, h)| format!("(seed: \"{s}\", hash: \"{h}\")"))
                    .collect();
                let mut bad = 0;
                for (i, (h, g)) in have.iter().zip(want_rows.iter()).enumerate() {
                    if h != g {
                        bad += 1;
                        println!("MISMATCH row {i}:\n  now:  {h}\n  file: {g}");
                    }
                }
                if bad == 0 && have.len() == want_rows.len() {
                    println!("golden ok ({} seeds)", have.len());
                } else {
                    bail!(
                        "golden drift ({} rows differ); if the generator changed on purpose, \
                         run `golden --update` in the same commit + note it in DECISIONS.md",
                        bad
                    );
                }
            }
        }
    }
    Ok(())
}

/// The 10 golden seeds and their world hashes (spec 02 C3).
fn golden_rows() -> Result<Vec<(String, String)>> {
    let seeds = [
        1u64,
        2,
        3,
        42,
        777,
        1337,
        24_601,
        31_337,
        0xC0FFEE,
        0x9E37_79B9_7F4A_7C15,
    ];
    // A generous attempt budget: the golden list is FROZEN across eras
    // (its hashes are the determinism contract), so a seed that happens
    // to need luck under current dials must not force a list swap.
    // Exhausted here is a real regression signal, not a shrug.
    let cfg = WorldGenConfig {
        max_world_attempts: 24,
        ..WorldGenConfig::default()
    };
    seeds
        .into_par_iter()
        .map(|n| {
            let map = generate(Seed(n), 1, &cfg, &ProgressNoop).context("golden seed")?;
            Ok((
                map.seed.to_string(),
                blake3::hash(map.to_ron().as_bytes()).to_hex().to_string(),
            ))
        })
        .collect()
}

fn write_world(out: &Path, map: &WorldMap, font: &FontArc) -> Result<()> {
    fs::create_dir_all(out)?;
    render_png(map, font)?.save(out.join("map.png"))?;
    fs::write(out.join("world.ron"), map.to_ron())?;
    let mt = &map.metrics;
    let mut stats = format!(
        "seed       {}\nround      {}\nattempts   {}\ngen_ms     {}\ndifficulty {:?}: {}\n\n\
         route_len    {:.2} tiles\nlongest_hop  {:.2} tiles\nhidden_crates {}/{}\nland_ratio   {:.4}\nterracing    {:.2}  (1 - biggest band share; >0.6 stepped, ~0.25 flat era, 0.58 mesa bug)\nislands    {}\nprops      {}\nrefugees   {}\n\n",
        map.seed,
        map.round,
        mt.attempts,
        mt.gen_ms,
        mt.difficulty,
        mt.difficulty_reason,
        mt.route_len,
        mt.longest_hop,
        mt.hidden_crates,
        map.crates.len(),
        mt.land_ratio,
        mt.terracing,
        map.islands.len(),
        map.props.len(),
        map.refugees.len(),
    );
    if !mt.stage_ms.is_empty() {
        stats.push_str("stages:\n");
        for (name, ms) in &mt.stage_ms {
            stats.push_str(&format!("  {name:12} {ms:>6} ms\n"));
        }
    }
    fs::write(out.join("stats.txt"), stats)?;
    Ok(())
}

fn surface_base(s: Surface) -> [f32; 3] {
    match s {
        Surface::Sea => [18.0, 44.0, 82.0],
        Surface::Shallow => [40.0, 104.0, 148.0],
        Surface::Sand => [214.0, 196.0, 140.0],
        // Grass picks up an elevation tint (low green → high amber-grey).
        Surface::Grass => [82.0, 148.0, 74.0],
        Surface::Rock => [136.0, 132.0, 128.0],
    }
}

/// Tile colour with NW-sun hillshade: relief is what makes archetypes and
/// level bands readable, so multiply by a slope-derived light term.
fn tile_color(map: &WorldMap, x: u32, y: u32, max_h: u8) -> Rgba<u8> {
    let h0 = map.height.raw(x, y);
    let mut base = surface_base(map.surface.raw(x, y));
    if map.surface.raw(x, y) == Surface::Grass {
        let t = (h0.min(max_h) as f32 / max_h.max(1) as f32).clamp(0.0, 1.0);
        // Stronger ramp (mix up to 0.8 at the top): Flat/Hilly/Plateau/Peak
        // must be identifiable from the sheet alone — level colour is the
        // signal that survives thumbnail downscaling.
        let t = t * t.sqrt(); // push mid-levels apart, keep lowlands green
        base = [
            base[0] + (238.0 - base[0]) * t * 0.80,
            base[1] + (230.0 - base[1]) * t * 0.55,
            base[2] + (200.0 - base[2]) * t * 0.50,
        ];
    }
    // Slope toward NW: positive when the NW faces are lit by the sun.
    let hl = map.height.raw(x.saturating_sub(1), y);
    let hr = map.height.raw((x + 1).min(map.size.0 - 1), y);
    let hu = map.height.raw(x, y.saturating_sub(1));
    let hd = map.height.raw(x, (y + 1).min(map.size.1 - 1));
    let slope = (hl as f32 - hr as f32 + hu as f32 - hd as f32) / 4.0;
    let mut shade = 0.82 + 0.32 * slope;
    if h0 == 0 {
        shade = 1.0; // water gets no relief
    }
    shade = shade.clamp(0.45, 1.22);
    Rgba([
        (base[0] * shade).min(255.0) as u8,
        (base[1] * shade).min(255.0) as u8,
        (base[2] * shade).min(255.0) as u8,
        255,
    ])
}

fn render_png(map: &WorldMap, font: &FontArc) -> Result<RgbaImage> {
    let (w, h) = map.size;
    let max_h = map.height.iter().copied().max().unwrap_or(1);
    let mut img = ImageBuffer::from_pixel(w * PX, h * PX, Rgba([0, 0, 0, 255]));
    let put = |img: &mut RgbaImage, x: u32, y: u32, c: Rgba<u8>| {
        if x < img.width() && y < img.height() {
            img.put_pixel(x, y, c);
        }
    };

    for y in 0..h {
        for x in 0..w {
            let c = tile_color(map, x, y, max_h);
            for dy in 0..PX {
                for dx in 0..PX {
                    put(&mut img, x * PX + dx, y * PX + dy, c);
                }
            }
            // Cliff outlines: a dark line on the dropping side.
            let edge = Rgba([10, 8, 6, 255]);
            let bit = map.cliff.raw(x, y);
            if bit & CLIFF_N != 0 {
                for dx in 0..PX {
                    put(&mut img, x * PX + dx, y * PX, edge);
                }
            }
            if bit & CLIFF_S != 0 {
                for dx in 0..PX {
                    put(&mut img, x * PX + dx, (y + 1) * PX - 1, edge);
                }
            }
            if bit & CLIFF_W != 0 {
                for dy in 0..PX {
                    put(&mut img, x * PX, y * PX + dy, edge);
                }
            }
            if bit & CLIFF_E != 0 {
                for dy in 0..PX {
                    put(&mut img, (x + 1) * PX - 1, y * PX + dy, edge);
                }
            }
        }
    }

    // Props: small glyph squares, coloured per kind.
    for p in &map.props {
        let c = match p.kind {
            PropKind::House => Rgba([228, 140, 70, 255]),
            PropKind::Hut => Rgba([160, 120, 80, 255]),
            PropKind::PalmTree => Rgba([90, 200, 110, 255]),
            PropKind::PineTree => Rgba([30, 110, 60, 255]),
            PropKind::Rock => Rgba([110, 110, 112, 255]),
            PropKind::Lighthouse => Rgba([245, 245, 245, 255]),
            PropKind::RadioTower => Rgba([180, 200, 220, 255]),
            PropKind::Hangar => Rgba([140, 150, 160, 255]),
            PropKind::Jetty => Rgba([120, 160, 190, 255]),
        };
        for dy in 0..PX.saturating_sub(2).max(1) {
            for dx in 0..PX.saturating_sub(2).max(1) {
                put(&mut img, p.tile.0 * PX + dx, p.tile.1 * PX + dy, c);
            }
        }
    }

    // Helipads: "H" (base island gold, others white).
    for (i, hp) in map.helipads.iter().enumerate() {
        let c = if i == 0 {
            Rgba([250, 220, 60, 255])
        } else {
            Rgba([240, 240, 240, 255])
        };
        draw_text_mut(
            &mut img,
            c,
            hp.tile.0 as i32 * PX as i32 - 3,
            hp.tile.1 as i32 * PX as i32 - 5,
            13.0,
            font,
            "H",
        );
    }

    // Crates: "X" — red when hidden from at least one heading.
    for cr in &map.crates {
        let c = if cr.hidden_from.iter().any(|hid| *hid) {
            Rgba([235, 60, 50, 255])
        } else {
            Rgba([250, 210, 40, 255])
        };
        draw_text_mut(
            &mut img,
            c,
            cr.tile.0 as i32 * PX as i32 - 3,
            cr.tile.1 as i32 * PX as i32 - 5,
            13.0,
            font,
            "X",
        );
    }

    // Refugees: white dots.
    for r in &map.refugees {
        for dy in 0..2 {
            for dx in 0..2 {
                put(
                    &mut img,
                    r.tile.0 * PX + 1 + dx,
                    r.tile.1 * PX + 1 + dy,
                    Rgba([255, 255, 255, 255]),
                );
            }
        }
    }

    // Island names, centred under each island centre with a shadow.
    for isl in &map.islands {
        let (tw, _) = text_size(14.0, font, &isl.name);
        // Labels live BELOW each island's bbox, never over its centre:
        // centred text painted over small islands made them read as
        // "ghost islands" (labels on blank water — round 90). The
        // island's own tiles stay the truth; the name is a caption.
        let cx = (isl.bbox.x0 + isl.bbox.x1) / 2;
        let x = (cx * PX).saturating_sub(tw / 2) as i32;
        let y = ((isl.bbox.y1 + 1) * PX + 2).min(h * PX - 14) as i32;
        draw_text_mut(
            &mut img,
            Rgba([0, 0, 0, 255]),
            x + 1,
            y + 1,
            14.0,
            font,
            &isl.name,
        );
        draw_text_mut(
            &mut img,
            Rgba([245, 245, 245, 255]),
            x,
            y,
            14.0,
            font,
            &isl.name,
        );
    }

    Ok(img)
}

fn contact_sheet(path: &Path, maps: &[&WorldMap], font: &FontArc) -> Result<()> {
    const THUMB: u32 = 160;
    const GRID: u32 = 8;
    let cell = THUMB + 20;
    let mut sheet: RgbaImage =
        ImageBuffer::from_pixel(cell * GRID, (cell + 6) * GRID, Rgba([16, 18, 22, 255]));
    for (i, m) in maps.iter().take((GRID * GRID) as usize).enumerate() {
        let gx = (i as u32 % GRID) * cell;
        let gy = (i as u32 / GRID) * (cell + 6);
        // Nearest-neighbour downscale straight into the sheet.
        let (w, h) = m.size;
        let sx = w as f32 / THUMB as f32;
        let sy = h as f32 / THUMB as f32;
        let max_h = m.height.iter().copied().max().unwrap_or(1);
        for ty in 0..THUMB {
            for tx in 0..THUMB {
                let x = ((tx as f32 * sx) as u32).min(w - 1);
                let y = ((ty as f32 * sy) as u32).min(h - 1);
                sheet.put_pixel(gx + tx, gy + ty, tile_color(m, x, y, max_h));
            }
        }
        let seed = m.seed.to_string();
        let (tw, _) = text_size(11.0, font, &seed);
        draw_text_mut(
            &mut sheet,
            Rgba([220, 220, 220, 255]),
            (gx + THUMB / 2).saturating_sub(tw / 2) as i32,
            (gy + THUMB + 2) as i32,
            11.0,
            font,
            &seed,
        );
    }
    sheet.save(path)?;
    Ok(())
}

/// One recorded rejection -> the histogram keys it contributes.
///
/// Rejection strings arrive shaped: "aN: <verdict>" or
/// "aN: validation: A; B; C" (validate() joins its issues). Unwrap
/// both wrappers, split joined issues, normalise instance text
/// (digits -> `#`, trailing "(instances)" cut), and collapse the
/// families whose messages embed island NAMES (per-seed, would make
/// every seed its own bucket): "… out of fuel reach" and
/// "… closer than min_gap" each become one stable key.
fn reject_keys(rejection: &str) -> Vec<String> {
    let body = rejection
        .split_once(": ")
        .map_or(rejection, |x| x.1)
        .strip_prefix("validation: ")
        .unwrap_or(rejection.split_once(": ").map_or(rejection, |x| x.1));
    body.split("; ")
        .map(|issue| {
            let key: String = issue
                .chars()
                .map(|c| if c.is_ascii_digit() { '#' } else { c })
                .collect();
            let key = key.replace("##", "#").replace("##", "#");
            // Trailing-instance cut ONLY when the message ends with ')'
            // ("… sites fit (style 2)"): mid-sentence parens like
            // "land at (#,#) inside the sea margin" must survive whole.
            let key = if issue.ends_with(')') {
                key.split(" (").next().unwrap_or(&key).to_owned()
            } else {
                key
            };
            if key.contains("out of fuel reach") {
                "island out of fuel reach".to_owned()
            } else if key.contains("closer than min_gap") {
                "pair closer than min_gap".to_owned()
            } else {
                key
            }
        })
        .collect()
}

#[cfg(test)]
mod reject_tests {
    use super::reject_keys;

    #[test]
    fn attempt_prefix_and_validation_wrapper_unwrap() {
        assert_eq!(
            reject_keys("a3: no room for the base hangar"),
            ["no room for the base hangar"]
        );
        let k = reject_keys(
            "a1: validation: Base Island has no helipad; land at (2,9) inside the sea margin",
        );
        assert_eq!(
            k,
            [
                "Base Island has no helipad",
                "land at (#,#) inside the sea margin"
            ]
        );
    }

    #[test]
    fn names_collapse_families() {
        assert_eq!(
            reject_keys(
                "a0: validation: island Nukesi Bay out of fuel reach (51); island Zephi Cay out of fuel reach (47)"
            ),
            ["island out of fuel reach", "island out of fuel reach"]
        );
        assert_eq!(
            reject_keys("a2: pair X Y closer than min_gap 6"),
            ["pair closer than min_gap"]
        );
    }

    #[test]
    fn digit_runs_collapse_once() {
        assert_eq!(
            reject_keys("a7: only 11 of 14 island sites fit (style 2)"),
            ["only # of # island sites fit"]
        );
    }
}
