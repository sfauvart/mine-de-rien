//! Voix de synthèse, toujours locales.

pub mod base;
pub mod piper;

use std::rc::Rc;

use crate::adapters::voix::base::Voix;
use crate::adapters::voix::piper::Piper;
use crate::config::{ErreurConfig, Voix as ConfigVoix};
use crate::util::repr;

pub fn creer(config: &ConfigVoix) -> Result<Rc<dyn Voix>, ErreurConfig> {
    match config.adaptateur.as_str() {
        "piper" => {
            if config.url.is_empty() {
                return Err(ErreurConfig(
                    "[voix] url manquante pour l'adaptateur piper".to_owned(),
                ));
            }
            Ok(Rc::new(Piper::new(
                &config.url,
                &config.voix,
                config.vitesse_normale,
                config.vitesse_lente,
            )))
        }
        autre => Err(ErreurConfig(format!(
            "[voix] adaptateur inconnu : {}",
            repr(autre)
        ))),
    }
}
