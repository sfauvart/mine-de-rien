//! Contrat d'une voix de synthèse : elle lit une phrase de dictée à voix haute.
//!
//! La voix est locale, sans exception : aucun service de synthèse distant n'est proposé. Elle
//! ne reçoit que le texte d'une dictée fourni par un parent, jamais le prénom ni le pseudo de
//! l'enfant, jamais ce qu'il a tapé.

use async_trait::async_trait;

/// La voix n'a pas répondu, ou sa réponse est inutilisable ; le message s'adresse au parent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurVoix(pub String);

#[async_trait(?Send)]
pub trait Voix {
    /// Lit `texte` et renvoie un fichier WAV (PCM 16 bits). `lent` choisit la vitesse lente,
    /// celle du premier indice : la voix articule plus longuement, le fichier n'est pas étiré.
    async fn dire(&self, texte: &str, lent: bool) -> Result<Vec<u8>, ErreurVoix>;
}
