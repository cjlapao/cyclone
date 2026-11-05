//! Cyclone Rescue — core crate. Everything except the shell loop lives here
//! as a library so the acceptance tests in tests/ can drive the simulation
//! directly (fixed-step physics, world generation, hazard math).

pub mod audio;
pub mod cyclone;
pub mod font;
pub mod game;
pub mod heli;
pub mod hud;
pub mod iso;
pub mod map;
pub mod planes;
pub mod render;
pub mod state;
pub mod tuning;
pub mod world;
pub mod worldgen;
