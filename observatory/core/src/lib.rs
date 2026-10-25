//! Schwarzschild Observatory core: a progressive synthetic telescope.
//!
//! Labels used throughout the documentation:
//! *physically derived*, *numerically approximated*, *statistically
//! synthesized*, *artistically directed*, *experimental*.

pub mod annotate;
pub mod disk;
pub mod export;
pub mod geodesic;
pub mod json;
pub mod math;
pub mod render;
pub mod rng;
pub mod scene;
pub mod sensor;
pub mod sky;
pub mod spectrum;
pub mod volgen;
pub mod volume;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
