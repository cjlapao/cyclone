use cyclone_worldgen::{ContinentConfig, elevation_m};
fn main() {
    let cfg = ContinentConfig {
        size: 4000,
        islands: true,
        ..Default::default()
    };
    let h = elevation_m(500.0, 500.0, 42, &cfg);
    println!("centre elevation seed 42 island mode: {h} m");
}
