//! De la dictée aux sons : ce qui relie le texte (`exercises/francais/dictee.rs`), la voix
//! (`adapters/voix`), la conversion (`son.rs`) et le pack (`pack.rs`).
//!
//! Chaque phrase a deux sons dans le pack, à vitesse normale et à vitesse lente. Leurs
//! numéros se déduisent du numéro de la phrase : `2 × phrase` et `2 × phrase + 1`. Un numéro
//! ne dit rien du texte.

use std::collections::BTreeMap;

use serde_json::json;

use crate::adapters::voix::base::{ErreurVoix, Voix};
use crate::exercises::base::Exercice;
use crate::exercises::francais::dictee::{ErreurDictee, Phrase, Position, exercice};
use crate::son::{ErreurSon, wav_vers_ogg};

#[derive(Debug, thiserror::Error)]
pub enum ErreurDictees {
    #[error("{0}")]
    Dictee(#[from] ErreurDictee),
    #[error("{0}")]
    Voix(#[from] ErreurVoix),
    #[error("{0}")]
    Son(#[from] ErreurSon),
}

/// Le numéro, dans le pack, du son de la phrase n°`phrase` (comptée à partir de 1).
pub fn numero_son(phrase: u32, lent: bool) -> u32 {
    phrase * 2 + u32::from(lent)
}

/// Fait lire `texte` aux deux vitesses et range les deux sons, en Ogg Vorbis, sous les numéros
/// de la phrase n°`phrase`.
pub async fn ajouter_sons(
    sons: &mut BTreeMap<u32, Vec<u8>>,
    voix: &dyn Voix,
    phrase: u32,
    texte: &str,
) -> Result<(), ErreurDictees> {
    for lent in [false, true] {
        let wav = voix.dire(texte, lent).await?;
        sons.insert(numero_son(phrase, lent), wav_vers_ogg(&wav)?);
    }
    Ok(())
}

/// L'exercice d'un trou, avec les numéros des sons de sa phrase, que le jeu lira.
pub fn exercice_sonore(
    phrases: &[Phrase],
    position: Position,
    phrase: u32,
) -> Result<Exercice, ErreurDictee> {
    let mut ex = exercice(phrases, position)?;
    ex.donnees
        .insert("son".to_owned(), json!(numero_son(phrase, false)));
    ex.donnees
        .insert("son_lent".to_owned(), json!(numero_son(phrase, true)));
    Ok(ex)
}
