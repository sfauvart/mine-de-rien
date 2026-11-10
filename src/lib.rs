//! Mine de rien : les devoirs se font sans que personne ne s'en aperçoive.

pub mod alea;
pub mod cli;
pub mod config;
pub mod datapack;
pub mod exercises;
pub mod minecraft;
mod util;

/// Version du paquet, celle de `Cargo.toml` : ce qu'affiche `mine-de-rien --version`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
