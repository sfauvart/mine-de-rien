//! Piper, par le serveur HTTP qu'il fournit (`python -m piper.http_server`).
//!
//! Piper est sous licence GPL : il tourne dans son propre conteneur et n'est jamais lié au
//! binaire. Le serveur répond à `POST {url}/synthesize` par un WAV. Il sait aussi télécharger
//! des voix (`/download`, `/all-voices`) : l'agent n'appelle jamais ces routes, et le compose
//! le place sur un réseau sans sortie.
//!
//! Le texte lu n'est pas journalisé.

use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;

use crate::adapters::voix::base::{ErreurVoix, Voix};

pub const DELAI: Duration = Duration::from_secs(60);

pub struct Piper {
    pub url: String,
    pub voix: String,
    pub vitesse_normale: f64,
    pub vitesse_lente: f64,
    delai: Duration,
}

impl Piper {
    /// Les vitesses sont le `length_scale` de Piper : 1 pour le débit de la voix, davantage
    /// pour un débit plus lent.
    pub fn new(url: &str, voix: &str, vitesse_normale: f64, vitesse_lente: f64) -> Self {
        Self {
            url: url.trim_end_matches('/').to_owned(),
            voix: voix.to_owned(),
            vitesse_normale,
            vitesse_lente,
            delai: DELAI,
        }
    }

    /// Un autre délai d'attente que les 60 secondes d'usage, pour les tests.
    pub fn avec_delai(mut self, delai: Duration) -> Self {
        self.delai = delai;
        self
    }
}

#[async_trait(?Send)]
impl Voix for Piper {
    async fn dire(&self, texte: &str, lent: bool) -> Result<Vec<u8>, ErreurVoix> {
        let injoignable = || ErreurVoix(format!("La voix locale est injoignable ({}).", self.url));
        let erreur = |e: reqwest::Error| {
            if e.is_timeout() {
                ErreurVoix(format!(
                    "La voix locale n'a pas répondu en {} s.",
                    self.delai.as_secs()
                ))
            } else {
                injoignable()
            }
        };
        if texte.trim().is_empty() {
            return Err(ErreurVoix("Rien à lire : le texte est vide.".to_owned()));
        }
        let vitesse = if lent {
            self.vitesse_lente
        } else {
            self.vitesse_normale
        };
        let client = reqwest::Client::builder()
            .timeout(self.delai)
            .build()
            .map_err(|_| injoignable())?;
        let reponse = client
            .post(format!("{}/synthesize", self.url))
            .json(&json!({"text": texte, "voice": self.voix, "length_scale": vitesse}))
            .send()
            .await
            .map_err(erreur)?;
        let statut = reponse.status();
        if statut.is_client_error() || statut.is_server_error() {
            return Err(ErreurVoix(format!(
                "La voix locale a répondu l'erreur {}.",
                statut.as_u16()
            )));
        }
        let octets = reponse.bytes().await.map_err(erreur)?;
        if !octets.starts_with(b"RIFF") {
            return Err(ErreurVoix(
                "Réponse de la voix locale dans un format inattendu.".to_owned(),
            ));
        }
        Ok(octets.to_vec())
    }
}
