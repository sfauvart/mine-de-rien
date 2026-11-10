//! Adaptateur de voix, contre un faux serveur HTTP local.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mine_de_rien::adapters::voix::base::Voix;
use mine_de_rien::adapters::voix::creer;
use mine_de_rien::adapters::voix::piper::Piper;
use mine_de_rien::config::{Voix as ConfigVoix, lire_config};
use serde_json::Value;

const WAV: &[u8] = b"RIFF\x24\0\0\0WAVEfmt et la suite";

#[derive(Debug, Clone, Default)]
struct Requete {
    ligne: String,
    corps: Vec<u8>,
}

/// Répond à chaque requête par `statut` et `corps`, après `attente`, et garde ce qu'il a reçu.
struct FauxServeur {
    url: String,
    requetes: Arc<Mutex<Vec<Requete>>>,
}

impl FauxServeur {
    fn nouveau(statut: u16, corps: &[u8], attente: Duration) -> Self {
        let ecoute = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let url = format!("http://127.0.0.1:{}", ecoute.local_addr().unwrap().port());
        let requetes = Arc::new(Mutex::new(Vec::new()));
        let recues = Arc::clone(&requetes);
        let corps = corps.to_vec();
        std::thread::spawn(move || {
            for connexion in ecoute.incoming() {
                let Ok(mut connexion) = connexion else { return };
                let mut lecteur = BufReader::new(connexion.try_clone().unwrap());
                let mut requete = Requete::default();
                lecteur.read_line(&mut requete.ligne).unwrap();
                let mut longueur = 0;
                loop {
                    let mut ligne = String::new();
                    lecteur.read_line(&mut ligne).unwrap();
                    let Some((nom, valeur)) = ligne.trim_end().split_once(": ") else {
                        break;
                    };
                    if nom.eq_ignore_ascii_case("content-length") {
                        longueur = valeur.parse().unwrap();
                    }
                }
                requete.corps = vec![0; longueur];
                lecteur.read_exact(&mut requete.corps).unwrap();
                recues.lock().unwrap().push(requete);
                std::thread::sleep(attente);
                let _ = write!(
                    connexion,
                    "HTTP/1.1 {statut} X\r\ncontent-type: audio/wav\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n",
                    corps.len()
                );
                let _ = connexion.write_all(&corps);
            }
        });
        Self { url, requetes }
    }

    fn recues(&self) -> Vec<Requete> {
        self.requetes.lock().unwrap().clone()
    }
}

#[tokio::test]
async fn requete_porte_le_texte_la_voix_et_la_vitesse() {
    let serveur = FauxServeur::nouveau(200, WAV, Duration::ZERO);
    let voix = Piper::new(&format!("{}/", serveur.url), "fr_FR-siwis-medium", 1.0, 1.4);
    let phrase = "Dans la ville de Dubaï s'élève la plus haute tour.";

    assert_eq!(voix.dire(phrase, false).await.unwrap(), WAV);
    assert_eq!(voix.dire(phrase, true).await.unwrap(), WAV);

    let requetes = serveur.recues();
    assert_eq!(requetes.len(), 2);
    for (requete, vitesse) in requetes.iter().zip([1.0, 1.4]) {
        assert!(
            requete.ligne.starts_with("POST /synthesize "),
            "{}",
            requete.ligne
        );
        let corps: Value = serde_json::from_slice(&requete.corps).unwrap();
        assert_eq!(corps["text"], phrase);
        assert_eq!(corps["voice"], "fr_FR-siwis-medium");
        assert_eq!(corps["length_scale"], vitesse);
    }
}

#[tokio::test]
async fn erreur_du_serveur() {
    let serveur = FauxServeur::nouveau(500, b"boum", Duration::ZERO);
    let voix = Piper::new(&serveur.url, "v", 1.0, 1.4);
    let erreur = voix.dire("Bonjour.", false).await.unwrap_err();
    assert_eq!(erreur.0, "La voix locale a répondu l'erreur 500.");
}

#[tokio::test]
async fn reponse_qui_n_est_pas_un_wav() {
    let serveur = FauxServeur::nouveau(200, b"<html>pas du son</html>", Duration::ZERO);
    let voix = Piper::new(&serveur.url, "v", 1.0, 1.4);
    let erreur = voix.dire("Bonjour.", false).await.unwrap_err();
    assert_eq!(
        erreur.0,
        "Réponse de la voix locale dans un format inattendu."
    );
}

#[tokio::test]
async fn delai_depasse() {
    let serveur = FauxServeur::nouveau(200, WAV, Duration::from_secs(2));
    let voix = Piper::new(&serveur.url, "v", 1.0, 1.4).avec_delai(Duration::from_millis(200));
    let erreur = voix.dire("Bonjour.", false).await.unwrap_err();
    assert_eq!(erreur.0, "La voix locale n'a pas répondu en 0 s.");
}

#[tokio::test]
async fn serveur_injoignable() {
    // Un port qui vient d'être libéré : personne n'y écoute.
    let port = TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let url = format!("http://127.0.0.1:{port}");
    let voix = Piper::new(&url, "v", 1.0, 1.4);
    let erreur = voix.dire("Bonjour.", false).await.unwrap_err();
    assert_eq!(erreur.0, format!("La voix locale est injoignable ({url})."));
}

#[tokio::test]
async fn texte_vide_refuse_sans_appel() {
    let serveur = FauxServeur::nouveau(200, WAV, Duration::ZERO);
    let voix = Piper::new(&serveur.url, "v", 1.0, 1.4);
    assert!(voix.dire("  \n", false).await.is_err());
    assert!(serveur.recues().is_empty());
}

#[test]
fn creer_refuse_un_adaptateur_inconnu_ou_sans_url() {
    let sans_url = ConfigVoix::default();
    assert_eq!(
        creer(&sans_url).err().unwrap().0,
        "[voix] url manquante pour l'adaptateur piper"
    );
    let inconnu = ConfigVoix {
        adaptateur: "nuage".to_owned(),
        url: "http://voix:5000".to_owned(),
        ..ConfigVoix::default()
    };
    assert_eq!(
        creer(&inconnu).err().unwrap().0,
        "[voix] adaptateur inconnu : 'nuage'"
    );
    let bon = ConfigVoix {
        url: "http://voix:5000".to_owned(),
        ..ConfigVoix::default()
    };
    assert!(creer(&bon).is_ok());
}

const MINIMALE: &str = "[enfant]\npseudo_minecraft = \"Steve\"\n";

fn lire(section: &str) -> Result<Option<ConfigVoix>, String> {
    lire_config(&format!("{MINIMALE}{section}"), &|_| None)
        .map(|c| c.voix)
        .map_err(|e| e.0)
}

#[test]
fn section_voix_absente_ou_lue() {
    assert_eq!(lire("").unwrap(), None);
    let voix = lire("[voix]\nurl = \"http://voix:5000\"\n")
        .unwrap()
        .unwrap();
    assert_eq!(voix.adaptateur, "piper");
    assert_eq!(voix.voix, "fr_FR-siwis-medium");
    assert_eq!((voix.vitesse_normale, voix.vitesse_lente), (1.0, 1.4));
}

#[test]
fn vitesses_verifiees() {
    assert_eq!(
        lire("[voix]\nvitesse_normale = 0.1\n").unwrap_err(),
        "[voix] vitesse_normale doit être entre 0.5 et 3 : 0.1"
    );
    assert_eq!(
        lire("[voix]\nvitesse_lente = 9.0\n").unwrap_err(),
        "[voix] vitesse_lente doit être entre 0.5 et 3 : 9"
    );
    assert_eq!(
        lire("[voix]\nvitesse_normale = 1.5\nvitesse_lente = 1.2\n").unwrap_err(),
        "[voix] vitesse_lente doit être au moins égale à vitesse_normale"
    );
    assert!(
        lire("[voix]\ndebit = 2\n")
            .unwrap_err()
            .starts_with("config refusée")
    );
}
