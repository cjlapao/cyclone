use cyclone_worldgen::*;
fn main() {
    let cfg = WorldGenConfig::default();
    let _ = generate(Seed(7), 1, &cfg, &ProgressNoop);
}
