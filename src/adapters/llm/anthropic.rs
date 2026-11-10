//! Fournisseur Anthropic, distant.
//!
//! Il ne reçoit jamais de photo : une photo de fiche montre souvent le prénom de l'enfant et ne
//! se pseudonymise pas. Le reste n'est pas encore écrit : aucun appel réseau n'est fait ici.

use async_trait::async_trait;

use crate::adapters::llm::base::{ErreurLlm, Llm};

pub struct Anthropic {
    pub modele: String,
    pub cle: String,
}

impl Anthropic {
    pub fn new(modele: &str, cle: &str) -> Self {
        Self {
            modele: modele.to_owned(),
            cle: cle.to_owned(),
        }
    }
}

#[async_trait(?Send)]
impl Llm for Anthropic {
    fn distant(&self) -> bool {
        true
    }

    async fn completer(
        &self,
        _systeme: &str,
        _utilisateur: &str,
        image: Option<&[u8]>,
        _en_json: bool,
    ) -> Result<String, ErreurLlm> {
        if image.is_some() {
            return Err(ErreurLlm(
                "La lecture de photo demande un LLM local ([llm] adaptateur = \
                 \"openai-compatible\") : une photo ne part jamais vers un LLM distant."
                    .to_owned(),
            ));
        }
        Err(ErreurLlm(
            "aucun usage du LLM distant pour l'instant".to_owned(),
        ))
    }
}
