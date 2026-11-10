//! Outils partagés par les tests.
//!
//! `tests/donnees/oracle/` garde les sorties de l'ancienne version Python de l'agent, relevées
//! pendant le passage à Rust (spec `2026-11-agent-en-rust`) : pour des entrées fixées, les
//! commandes Minecraft, les messages et les rapports qu'elle produisait. Le script qui les
//! écrivait a disparu avec elle ; ces fichiers ne se régénèrent plus, ils se relisent. Un test
//! qui s'en écarte signale un changement de comportement : s'il est voulu, corriger le cas à
//! la main et le dire dans le commit.
#![allow(dead_code)]

pub mod terrain;

use std::path::PathBuf;

pub fn racine() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Les cas de `tests/donnees/oracle/<nom>.json`.
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

/// Copie d'une base de référence écrite par la version Python (`tests/donnees/bases/v<N>.sqlite`) :
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

use std::cell::RefCell;
use std::collections::VecDeque;

use async_trait::async_trait;
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::minecraft::texte::Style;
use serde_json::{Value, json};

/// Les clés de style d'un message, écrites comme les arguments nommés de Python.
pub fn style_json(style: &[(&str, Style<'_>)]) -> Value {
    let objet: serde_json::Map<String, Value> = style
        .iter()
        .map(|(cle, valeur)| {
            let valeur = match valeur {
                Style::Texte(texte) => json!(texte),
                Style::Bool(b) => json!(b),
            };
            ((*cle).to_owned(), valeur)
        })
        .collect();
    Value::Object(objet)
}

/// Rejoue une transcription de la version Python : chaque commande envoyée par Rust doit être
/// la suivante de la transcription, et reçoit la réponse que Python avait reçue.
pub struct Rejeu {
    echanges: RefCell<VecDeque<Value>>,
    contexte: String,
}

impl Rejeu {
    pub fn new(cas: &Value) -> Self {
        let echanges = cas["echanges"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into();
        Self {
            echanges: RefCell::new(echanges),
            contexte: cas["entree"].to_string(),
        }
    }

    fn suivant(&self, envoye: &Value) -> Value {
        self.echanges.borrow_mut().pop_front().unwrap_or_else(|| {
            panic!(
                "{} : Rust envoie {envoye}, Python n'envoyait plus rien",
                self.contexte
            )
        })
    }

    /// À appeler en fin de cas : Rust a envoyé tout ce que Python envoyait.
    pub fn verifier_fin(&self) {
        let reste = self.echanges.borrow();
        assert!(
            reste.is_empty(),
            "{} : Python envoyait encore {} échange(s), dont {}",
            self.contexte,
            reste.len(),
            reste[0]
        );
    }
}

#[async_trait(?Send)]
impl AdaptateurJeu for Rejeu {
    async fn commande(&self, commande: &str) -> Result<String, ErreurRcon> {
        // La lecture du nom de la clé (spec 2026-11-dictee-audio, phase 6) n'existait pas
        // dans la version Python : c'est la seule commande que Rust envoie en plus. Elle ne
        // consomme rien de la transcription et ne trouve pas de clé nommée.
        if commande.starts_with("data get entity ") && commande.contains("mdr_cle") {
            return Ok("Found no elements matching Inventory".to_owned());
        }
        let attendu = self.suivant(&json!(["commande", commande]));
        assert_eq!(
            json!(commande),
            attendu[1],
            "{} : {}",
            self.contexte,
            attendu[0]
        );
        let texte = attendu[2].as_str().unwrap_or_default().to_owned();
        match attendu[0].as_str() {
            Some("commande") => Ok(texte),
            // Le serveur de la transcription était en panne à cet échange.
            Some("panne") => Err(ErreurRcon(texte)),
            _ => panic!(
                "{} : Rust envoie la commande {commande}, Python {attendu}",
                self.contexte
            ),
        }
    }

    async fn dire(
        &self,
        joueur: &str,
        texte: &str,
        style: &[(&str, Style<'_>)],
    ) -> Result<(), ErreurRcon> {
        let envoye = json!(["dire", joueur, texte, style_json(style)]);
        let attendu = self.suivant(&envoye);
        assert_eq!(envoye, attendu, "{}", self.contexte);
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat dans une transcription".to_owned()))
    }
}

/// Compare le résultat d'un cas transcrit, puis vérifie que toute la transcription a servi.
pub fn conclure<T: serde::Serialize, E: std::fmt::Display>(
    cas: &Value,
    rejeu: &Rejeu,
    resultat: Result<T, E>,
) {
    comparer(cas, resultat);
    rejeu.verifier_fin();
}
