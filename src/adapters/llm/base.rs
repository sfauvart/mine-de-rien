//! Contrat d'un fournisseur de LLM.
//!
//! Le LLM recopie ce qu'il lit sur une photo de fiche, classe et reformule ce que dit Pixel. Il
//! ne calcule jamais de réponse et ne reçoit jamais le prénom de l'enfant. Une photo ne se
//! pseudonymise pas, un message de chat non plus : l'une comme l'autre ne partent que vers un
//! fournisseur local (`distant() == false`). Seul le message adressé à Pixel part, jamais le
//! reste du chat.

use std::sync::LazyLock;

use async_trait::async_trait;
use regex::Regex;
use serde_json::{Map, Value};

/// Le LLM n'a pas répondu, ou sa réponse est inutilisable ; le message s'adresse au parent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurLlm(pub String);

#[async_trait(?Send)]
pub trait Llm {
    /// Vrai si le fournisseur est hors de la maison : il ne reçoit alors ni photo ni chat.
    fn distant(&self) -> bool;

    /// Renvoie la réponse textuelle du modèle. `image` est un JPEG.
    async fn completer(
        &self,
        systeme: &str,
        utilisateur: &str,
        image: Option<&[u8]>,
        en_json: bool,
    ) -> Result<String, ErreurLlm>;
}

static BALISES: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^\s*```(?:json)?\s*(.*?)\s*```\s*$").unwrap());

/// Lit la réponse JSON d'un modèle, entourée ou non de balises de code.
pub fn lire_json(texte: &str) -> Result<Map<String, Value>, ErreurLlm> {
    let texte = BALISES
        .captures(texte)
        .and_then(|m| m.get(1))
        .map_or(texte, |m| m.as_str());
    match serde_json::from_str::<Value>(texte) {
        Ok(Value::Object(objet)) => Ok(objet),
        Ok(_) => Err(ErreurLlm(
            "Le LLM a répondu un JSON qui n'est pas un objet.".to_owned(),
        )),
        Err(_) => Err(ErreurLlm(
            "Le LLM a répondu autre chose que du JSON.".to_owned(),
        )),
    }
}
