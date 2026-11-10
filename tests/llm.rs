//! Adaptateurs LLM, contre un faux serveur HTTP local.

mod commun;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use mine_de_rien::adapters::llm::anthropic::Anthropic;
use mine_de_rien::adapters::llm::base::{Llm, lire_json};
use mine_de_rien::adapters::llm::creer;
use mine_de_rien::adapters::llm::openai_compatible::OpenAICompatible;
use mine_de_rien::config::Llm as ConfigLlm;
use serde_json::{Value, json};

/// Une requête reçue par le faux serveur : sa première ligne, ses en-têtes, son corps.
#[derive(Debug, Clone, Default)]
struct Requete {
    ligne: String,
    entetes: Vec<(String, String)>,
    corps: Vec<u8>,
}

impl Requete {
    fn entete(&self, nom: &str) -> Option<&str> {
        self.entetes
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(nom))
            .map(|(_, v)| v.as_str())
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.corps).unwrap()
    }
}

/// Répond à chaque requête par `statut` et `corps`, après `attente`, et garde ce qu'il a reçu.
struct FauxServeur {
    url: String,
    requetes: Arc<Mutex<Vec<Requete>>>,
}

impl FauxServeur {
    fn nouveau(statut: u16, corps: String, attente: Duration) -> Self {
        let ecoute = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let url = format!(
            "http://127.0.0.1:{}/v1",
            ecoute.local_addr().unwrap().port()
        );
        let requetes = Arc::new(Mutex::new(Vec::new()));
        let recues = Arc::clone(&requetes);
        std::thread::spawn(move || {
            for connexion in ecoute.incoming() {
                let Ok(mut connexion) = connexion else { return };
                let mut lecteur = BufReader::new(connexion.try_clone().unwrap());
                let mut requete = Requete::default();
                lecteur.read_line(&mut requete.ligne).unwrap();
                loop {
                    let mut ligne = String::new();
                    lecteur.read_line(&mut ligne).unwrap();
                    let Some((nom, valeur)) = ligne.trim_end().split_once(": ") else {
                        break;
                    };
                    requete.entetes.push((nom.to_owned(), valeur.to_owned()));
                }
                let longueur: usize = requete
                    .entete("content-length")
                    .and_then(|l| l.parse().ok())
                    .unwrap_or(0);
                requete.corps = vec![0; longueur];
                lecteur.read_exact(&mut requete.corps).unwrap();
                recues.lock().unwrap().push(requete);
                std::thread::sleep(attente);
                let _ = write!(
                    connexion,
                    "HTTP/1.1 {statut} X\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n{corps}",
                    corps.len()
                );
            }
        });
        Self { url, requetes }
    }

    fn repondant(contenu: &str) -> Self {
        let corps = json!({"choices": [{"message": {"content": contenu}}]}).to_string();
        Self::nouveau(200, corps, Duration::ZERO)
    }

    fn recues(&self) -> Vec<Requete> {
        self.requetes.lock().unwrap().clone()
    }
}

#[tokio::test]
async fn requete_porte_image_temperature_et_format_json() {
    let serveur = FauxServeur::repondant("{}");
    let llm = OpenAICompatible::new(&format!("{}/", serveur.url), "gemma", "");
    let reponse = llm
        .completer("système", "lis", Some(b"\xff\xd8jpeg"), true)
        .await
        .unwrap();
    assert_eq!(reponse, "{}");
    let requetes = serveur.recues();
    assert_eq!(requetes.len(), 1);
    assert!(requetes[0].ligne.starts_with("POST /v1/chat/completions "));
    assert!(requetes[0].entete("authorization").is_none());
    let corps = requetes[0].json();
    assert_eq!(corps["temperature"], 0);
    assert_eq!(corps["model"], "gemma");
    assert_eq!(corps["response_format"], json!({"type": "json_object"}));
    assert_eq!(
        corps["messages"][0],
        json!({"role": "system", "content": "système"})
    );
    let contenu = &corps["messages"][1]["content"];
    assert_eq!(
        contenu[0]["image_url"]["url"],
        format!(
            "data:image/jpeg;base64,{}",
            STANDARD.encode(b"\xff\xd8jpeg")
        )
    );
    assert_eq!(contenu[1], json!({"type": "text", "text": "lis"}));
}

#[tokio::test]
async fn sans_image_ni_json() {
    let serveur = FauxServeur::repondant("ok");
    let llm = OpenAICompatible::new(&serveur.url, "m", "secret");
    assert_eq!(llm.completer("s", "u", None, false).await.unwrap(), "ok");
    let requetes = serveur.recues();
    let corps = requetes[0].json();
    assert!(corps.get("response_format").is_none());
    assert_eq!(
        corps["messages"][1]["content"],
        json!([{"type": "text", "text": "u"}])
    );
    assert_eq!(requetes[0].entete("authorization"), Some("Bearer secret"));
}

#[tokio::test]
async fn erreur_http_devient_erreur_llm() {
    let serveur = FauxServeur::nouveau(500, "{}".to_owned(), Duration::ZERO);
    let erreur = OpenAICompatible::new(&serveur.url, "m", "")
        .completer("s", "u", None, false)
        .await;
    assert!(erreur.unwrap_err().to_string().contains("500"));
}

#[tokio::test]
async fn serveur_injoignable() {
    let libre = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let url = format!("http://127.0.0.1:{}/v1", libre.local_addr().unwrap().port());
    drop(libre);
    let erreur = OpenAICompatible::new(&url, "m", "")
        .completer("s", "u", None, false)
        .await;
    assert!(erreur.unwrap_err().to_string().contains("injoignable"));
}

#[tokio::test]
async fn delai_depasse_dit_combien_de_temps() {
    let serveur = FauxServeur::nouveau(200, "{}".to_owned(), Duration::from_secs(3));
    let llm = OpenAICompatible::new(&serveur.url, "m", "").avec_delai(Duration::from_secs(1));
    let erreur = llm.completer("s", "u", None, false).await.unwrap_err();
    assert_eq!(erreur.to_string(), "Le LLM local n'a pas répondu en 1 s.");
}

#[tokio::test]
async fn reponse_dans_un_format_inattendu() {
    for corps in [
        "pas du json",
        "{}",
        r#"{"choices": []}"#,
        r#"{"choices": [{"message": {}}]}"#,
    ] {
        let serveur = FauxServeur::nouveau(200, corps.to_owned(), Duration::ZERO);
        let erreur = OpenAICompatible::new(&serveur.url, "m", "")
            .completer("s", "u", None, false)
            .await;
        assert!(
            erreur.unwrap_err().to_string().contains("format inattendu"),
            "{corps}"
        );
    }
}

#[test]
fn lire_json_avec_ou_sans_balises() {
    assert_eq!(
        Value::Object(lire_json("{\"a\": 1}").unwrap()),
        json!({"a": 1})
    );
    assert_eq!(
        Value::Object(lire_json("```json\n{\"lignes\": []}\n```").unwrap()),
        json!({"lignes": []})
    );
    assert_eq!(
        Value::Object(lire_json("  ```\n{\"b\": 2}\n```  ").unwrap()),
        json!({"b": 2})
    );
}

#[test]
fn lire_json_refuse_le_texte_libre() {
    assert!(lire_json("Voici les lignes : Double de 2").is_err());
    assert!(lire_json("[1, 2]").is_err());
}

#[tokio::test]
async fn anthropic_refuse_toute_image() {
    let llm = Anthropic::new("claude", "cle");
    assert!(llm.distant());
    let erreur = llm
        .completer("s", "u", Some(b"jpeg"), false)
        .await
        .unwrap_err();
    assert!(erreur.to_string().contains("local"));
    // Sans image non plus, aucun appel : le fournisseur distant n'a pas encore d'usage.
    assert!(llm.completer("s", "u", None, false).await.is_err());
}

#[test]
fn fabrique() {
    let config = |adaptateur: &str, url: &str| ConfigLlm {
        adaptateur: adaptateur.to_owned(),
        url: url.to_owned(),
        modele: "m".to_owned(),
        cle: String::new(),
    };
    assert!(
        !creer(&config("openai-compatible", "http://llm/v1"))
            .unwrap()
            .distant()
    );
    assert!(creer(&config("anthropic", "")).unwrap().distant());
    let erreur = |c: ConfigLlm| creer(&c).err().unwrap().to_string();
    assert_eq!(
        erreur(config("openai-compatible", "")),
        "[llm] url manquante pour l'adaptateur openai-compatible"
    );
    assert_eq!(
        erreur(config("autre", "")),
        "[llm] adaptateur inconnu : 'autre'"
    );
}

/// Mêmes lectures et mêmes refus que Python sur des réponses de modèle bien ou mal formées.
#[test]
fn oracle_python_lire_json() {
    for cas in commun::oracle("photo") {
        if cas["entree"]["f"] == "lire_json" {
            commun::comparer(&cas, lire_json(cas["entree"]["texte"].as_str().unwrap()));
        }
    }
}
