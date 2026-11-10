//! Outils partagés par les tests : fichiers de référence produits par la version Python.
#![allow(dead_code)]

use std::path::PathBuf;

pub fn racine() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Les cas de `tests/donnees/oracle/<nom>.json`, écrits par `scripts/oracle.py`.
pub fn oracle(nom: &str) -> Vec<serde_json::Value> {
    let chemin = racine()
        .join("tests/donnees/oracle")
        .join(format!("{nom}.json"));
    let texte = std::fs::read_to_string(&chemin)
        .unwrap_or_else(|e| panic!("{} illisible : {e}", chemin.display()));
    serde_json::from_str(&texte).unwrap_or_else(|e| panic!("{} : {e}", chemin.display()))
}
