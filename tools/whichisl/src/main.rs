fn main() {
    // emulate: scan order (golden rings) vs largest blob per seed
    use cyclone_worldgen::{ContinentConfig, elevation_m};
    let cfg = ContinentConfig {
        size: 4000,
        islands: true,
        ..Default::default()
    };
    for seed in [42u64, 7, 123] {
        // crude largest-blob + its centre via grid BFS at 4 m cells
        let n = 250usize; // 1000 m / 4
        let h = |x: usize, y: usize| elevation_m(x as f64 * 4.0, y as f64 * 4.0, seed, &cfg);
        let mut seen = vec![false; n * n];
        let mut best: (u64, f64, f64, f64) = (0, 0.0, 0.0, 0.0); // cells, cx_m, cy_m, maxh
        for y in 0..n {
            for x in 0..n {
                if h(x, y) <= 0.0 || seen[y * n + x] {
                    continue;
                }
                let mut st = vec![(x, y)];
                seen[y * n + x] = true;
                let (mut cnt, mut sx, mut sy, mut mh) = (0u64, 0u64, 0u64, 0.0f64);
                while let Some((cx, cy)) = st.pop() {
                    cnt += 1;
                    sx += cx as u64;
                    sy += cy as u64;
                    mh = mh.max(h(cx, cy));
                    for (dx, dy) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        let (ax, ay) = (cx as isize + dx, cy as isize + dy);
                        if ax >= 0
                            && ay >= 0
                            && (ax as usize) < n
                            && (ay as usize) < n
                            && !seen[ay as usize * n + ax as usize]
                            && h(ax as usize, ay as usize) > 0.0
                        {
                            seen[ay as usize * n + ax as usize] = true;
                            st.push((ax as usize, ay as usize));
                        }
                    }
                }
                if cnt > best.0 {
                    best = (
                        cnt,
                        (sx as f64 / cnt as f64) * 4.0,
                        (sy as f64 / cnt as f64) * 4.0,
                        mh,
                    );
                }
            }
        }
        let d = ((best.1 - 500.0).powi(2) + (best.2 - 500.0).powi(2)).sqrt();
        println!(
            "seed {seed}: largest island centre {:.0},{:.0} maxh {:.0} m, {d:.0} m from map centre",
            best.1, best.2, best.3,
        );
    }
}
