//! Tout serveur exposant l'API compatible OpenAI : llama-server (llama.cpp), Ollama, LM Studio.
//!
//! C'est la voie 100 % locale, la seule à qui l'on confie une photo. llama-server ignore le
//! champ `model` quand il ne sert qu'un modèle ; Ollama exige l'identifiant exact que donne
//! `GET {url}/models`.
//!
//! Ni les images ni les réponses ne sont journalisées.

use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde_json::{Value, json};

use crate::adapters::llm::base::{ErreurLlm, Llm};

pub const DELAI: Duration = Duration::from_secs(180);

pub struct OpenAICompatible {
    pub url: String,
    pub modele: String,
    cle: String,
    delai: Duration,
}

impl OpenAICompatible {
    pub fn new(url: &str, modele: &str, cle: &str) -> Self {
        Self {
            url: url.trim_end_matches('/').to_owned(),
            modele: modele.to_owned(),
            cle: cle.to_owned(),
            delai: DELAI,
        }
    }

    /// Un autre délai d'attente que les 180 secondes d'usage, pour les tests.
    pub fn avec_delai(mut self, delai: Duration) -> Self {
        self.delai = delai;
        self
    }

    fn corps(
        &self,
        systeme: &str,
        utilisateur: &str,
        image: Option<&[u8]>,
        en_json: bool,
    ) -> Value {
        let mut contenu = Vec::new();
        if let Some(image) = image {
            let url = format!("data:image/jpeg;base64,{}", STANDARD.encode(image));
            contenu.push(json!({"type": "image_url", "image_url": {"url": url}}));
        }
        contenu.push(json!({"type": "text", "text": utilisateur}));
        let mut corps = json!({
            "model": self.modele,
            "temperature": 0,
            "messages": [
                {"role": "system", "content": systeme},
                {"role": "user", "content": contenu},
            ],
        });
        if en_json {
            corps["response_format"] = json!({"type": "json_object"});
        }
        corps
    }
}

#[async_trait(?Send)]
impl Llm for OpenAICompatible {
    fn distant(&self) -> bool {
        false
    }

    async fn completer(
        &self,
        systeme: &str,
        utilisateur: &str,
        image: Option<&[u8]>,
        en_json: bool,
    ) -> Result<String, ErreurLlm> {
        let injoignable = || ErreurLlm(format!("Le LLM local est injoignable ({}).", self.url));
        let erreur = |e: reqwest::Error| {
            if e.is_timeout() {
                ErreurLlm(format!(
                    "Le LLM local n'a pas répondu en {} s.",
                    self.delai.as_secs()
                ))
            } else {
                injoignable()
            }
        };
        let client = reqwest::Client::builder()
            .timeout(self.delai)
            .build()
            .map_err(|_| injoignable())?;
        let mut requete = client
            .post(format!("{}/chat/completions", self.url))
            .json(&self.corps(systeme, utilisateur, image, en_json));
        if !self.cle.is_empty() {
            requete = requete.bearer_auth(&self.cle);
        }
        let reponse = requete.send().await.map_err(erreur)?;
        let statut = reponse.status();
        if statut.is_client_error() || statut.is_server_error() {
            return Err(ErreurLlm(format!(
                "Le LLM local a répondu l'erreur {}.",
                statut.as_u16()
            )));
        }
        let octets = reponse.bytes().await.map_err(erreur)?;
        let inattendu = || ErreurLlm("Réponse du LLM local dans un format inattendu.".to_owned());
        let corps: Value = serde_json::from_slice(&octets).map_err(|_| inattendu())?;
        match &corps["choices"][0]["message"]["content"] {
            Value::String(contenu) => Ok(contenu.clone()),
            _ => Err(inattendu()),
        }
    }
}
