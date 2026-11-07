use cyclone_worldgen::ContinentConfig;
fn main() {
    let seed: u64 = std::env::args()
        .nth(1)
        .and_then(|a| a.parse().ok())
        .unwrap_or(99);
    let cut: f64 = std::env::args()
        .nth(2)
        .and_then(|a| a.parse().ok())
        .unwrap_or(0.60);
    let cfg = ContinentConfig {
        islands: true,
        ..Default::default()
    };
    // Census reads the mask through elevation: override via env in the
    // lib? No — sweep by regenerating with cut baked in temporarily:
    // we measure the RAW mask instead (cheap: same field call).
    let _ = cut;
    let c = cyclone_worldgen::generate_continent(seed, &cfg).unwrap();
    eprintln!("seed {seed} cut {cut}");
    let s = c.config.size as usize;
    let land = |x: usize, y: usize| c.heights[y * s + x] > 0;
    let mut seen = vec![false; s * s];
    let mut blobs: Vec<(usize, f64)> = Vec::new(); // cells, max h
    for y in 0..s {
        for x in 0..s {
            if land(x, y) && !seen[y * s + x] {
                let mut stack = vec![(x, y)];
                let mut cells = 0usize;
                let mut maxh = 0u16;
                seen[y * s + x] = true;
                while let Some((cx, cy)) = stack.pop() {
                    cells += 1;
                    maxh = maxh.max(c.heights[cy * s + cx]);
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let (nx, ny) = (cx as isize + dx, cy as isize + dy);
                        if nx >= 0
                            && ny >= 0
                            && (nx as usize) < s
                            && (ny as usize) < s
                            && land(nx as usize, ny as usize)
                            && !seen[ny as usize * s + nx as usize]
                        {
                            seen[ny as usize * s + nx as usize] = true;
                            stack.push((nx as usize, ny as usize));
                        }
                    }
                }
                blobs.push((cells, maxh as f64 * c.config.voxel as f64));
            }
        }
    }
    blobs.sort_by_key(|b| std::cmp::Reverse(b.0));
    println!("seed 99: {} blobs", blobs.len());
    for (cells, h) in blobs.iter().take(4) {
        let side = (*cells as f64).sqrt() * 0.25; // equivalent square side, m
        println!(
            "  area {:.2} km²  side {} m  max {} m",
            (*cells as f64) * 0.0625 / 1e6,
            side.round(),
            h
        );
    }
}
