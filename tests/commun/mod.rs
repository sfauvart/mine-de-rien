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

/// Compare un résultat Rust à un cas de l'oracle, qui porte `sortie` ou `erreur`.
pub fn comparer<T: serde::Serialize, E: std::fmt::Display>(
    cas: &serde_json::Value,
    resultat: Result<T, E>,
) {
    let entree = &cas["entree"];
    match (cas.get("erreur"), resultat) {
        (Some(message), Err(e)) => assert_eq!(e.to_string(), message.as_str().unwrap(), "{entree}"),
        (None, Ok(valeur)) => {
            assert_eq!(
                serde_json::to_value(valeur).unwrap(),
                cas["sortie"],
                "{entree}"
            )
        }
        (Some(message), Ok(valeur)) => panic!(
            "{entree} : Python refuse ({message}), Rust accepte {}",
            serde_json::to_value(valeur).unwrap()
        ),
        (None, Err(e)) => panic!(
            "{entree} : Python accepte {}, Rust refuse ({e})",
            cas["sortie"]
        ),
    }
}

/// Copie d'une base de référence écrite par Python (`tests/donnees/bases/v<N>.sqlite`) :
/// l'ouvrir la migre, donc jamais l'original. Le dossier temporaire vit autant que la copie.
pub fn copie_base(nom: &str) -> (tempfile::TempDir, PathBuf) {
    let dossier = tempfile::tempdir().unwrap();
    let copie = dossier.path().join("copie.sqlite");
    std::fs::copy(
        racine()
            .join("tests/donnees/bases")
            .join(format!("{nom}.sqlite")),
        &copie,
    )
    .unwrap();
    (dossier, copie)
}

pub fn instant(iso: &str) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339(iso)
        .unwrap()
        .with_timezone(&chrono::Utc)
}
