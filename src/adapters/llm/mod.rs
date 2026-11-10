//! Fournisseurs de LLM.

pub mod anthropic;
pub mod base;
pub mod openai_compatible;

use std::rc::Rc;

use crate::adapters::llm::anthropic::Anthropic;
use crate::adapters::llm::base::Llm;
use crate::adapters::llm::openai_compatible::OpenAICompatible;
use crate::config::{ErreurConfig, Llm as ConfigLlm};
use crate::util::repr;

pub fn creer(config: &ConfigLlm) -> Result<Rc<dyn Llm>, ErreurConfig> {
    match config.adaptateur.as_str() {
        "openai-compatible" => {
            if config.url.is_empty() {
                return Err(ErreurConfig(
                    "[llm] url manquante pour l'adaptateur openai-compatible".to_owned(),
                ));
            }
            Ok(Rc::new(OpenAICompatible::new(
                &config.url,
                &config.modele,
                &config.cle,
            )))
        }
        "anthropic" => Ok(Rc::new(Anthropic::new(&config.modele, &config.cle))),
        autre => Err(ErreurConfig(format!(
            "[llm] adaptateur inconnu : {}",
            repr(autre)
        ))),
    }
}
