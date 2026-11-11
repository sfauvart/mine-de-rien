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
        // Le tableau de dictée (phase 9) est effacé avant un déplacement de l'atelier : même
        // tolérance, il n'y en a jamais dans une transcription.
        if commande == mine_de_rien::games::tableau::commande_effacement() {
            return Ok("No entity was found".to_owned());
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

/// Une diffusion qui ne joue rien : elle note ce qu'on lui a fait dire, garder et désigner.
pub struct FausseDiffusion {
    /// Les sons dits à la volée.
    pub dits: std::cell::RefCell<Vec<mine_de_rien::son::Pcm>>,
    pub coupures: std::cell::Cell<usize>,
    /// Ce que répond `relie` : le jeu de l'enfant a le chat vocal, ou non.
    pub reliee: std::cell::Cell<bool>,
    /// Les sons gardés dans le dossier partagé, par nom.
    pub gardes: std::cell::RefCell<std::collections::BTreeMap<String, mine_de_rien::son::Pcm>>,
    /// Le son désigné à l'Écouteur.
    pub designe: std::cell::RefCell<Option<String>>,
    /// Tout ce qui a été demandé, dans l'ordre : « garder phrase-2.pcm », « redire … »,
    /// « désigner … », « désigner rien », « oublier … », « dire », « couper ».
    pub actions: std::cell::RefCell<Vec<String>>,
    /// Le serveur refuse de jouer : `dire` et `redire` échouent.
    pub en_panne: std::cell::Cell<bool>,
}

impl FausseDiffusion {
    pub fn new() -> Self {
        Self {
            dits: std::cell::RefCell::new(Vec::new()),
            coupures: std::cell::Cell::new(0),
            reliee: std::cell::Cell::new(true),
            gardes: std::cell::RefCell::new(std::collections::BTreeMap::new()),
            designe: std::cell::RefCell::new(None),
            actions: std::cell::RefCell::new(Vec::new()),
            en_panne: std::cell::Cell::new(false),
        }
    }

    pub fn actions(&self) -> Vec<String> {
        self.actions.borrow().clone()
    }

    fn note(&self, action: String) {
        self.actions.borrow_mut().push(action);
    }

    fn panne(&self) -> Result<(), mine_de_rien::diffusion::ErreurDiffusion> {
        if self.en_panne.get() {
            Err(mine_de_rien::diffusion::ErreurDiffusion(
                "Le serveur n'a pas joué le son : refusé : sans chat vocal".to_owned(),
            ))
        } else {
            Ok(())
        }
    }
}

impl Default for FausseDiffusion {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait(?Send)]
impl mine_de_rien::diffusion::Diffusion for FausseDiffusion {
    async fn dire(
        &self,
        son: &mine_de_rien::son::Pcm,
    ) -> Result<(), mine_de_rien::diffusion::ErreurDiffusion> {
        self.panne()?;
        self.note("dire".to_owned());
        self.dits.borrow_mut().push(son.clone());
        Ok(())
    }

    async fn couper(&self) -> Result<(), mine_de_rien::diffusion::ErreurDiffusion> {
        self.note("couper".to_owned());
        self.dits.borrow_mut().clear();
        self.coupures.set(self.coupures.get() + 1);
        Ok(())
    }

    async fn relie(&self) -> Result<bool, mine_de_rien::diffusion::ErreurDiffusion> {
        Ok(self.reliee.get())
    }

    async fn garder(
        &self,
        nom: &str,
        son: &mine_de_rien::son::Pcm,
    ) -> Result<(), mine_de_rien::diffusion::ErreurDiffusion> {
        mine_de_rien::diffusion::verifier_fichier(nom)?;
        self.note(format!("garder {nom}"));
        self.gardes.borrow_mut().insert(nom.to_owned(), son.clone());
        Ok(())
    }

    async fn garde(&self, nom: &str) -> bool {
        self.gardes.borrow().contains_key(nom)
    }

    async fn redire(&self, nom: &str) -> Result<(), mine_de_rien::diffusion::ErreurDiffusion> {
        self.panne()?;
        assert!(
            self.gardes.borrow().contains_key(nom),
            "{nom} n'est pas gardé"
        );
        self.note(format!("redire {nom}"));
        Ok(())
    }

    async fn designer(
        &self,
        nom: Option<&str>,
    ) -> Result<(), mine_de_rien::diffusion::ErreurDiffusion> {
        self.note(format!("désigner {}", nom.unwrap_or("rien")));
        *self.designe.borrow_mut() = nom.map(str::to_owned);
        Ok(())
    }

    async fn oublier(&self, nom: &str) -> Result<(), mine_de_rien::diffusion::ErreurDiffusion> {
        self.note(format!("oublier {nom}"));
        self.gardes.borrow_mut().remove(nom);
        Ok(())
    }
}

/// Un WAV PCM 16 bits mono à 22 050 Hz, de `trames` échantillons.
pub fn wav(trames: usize) -> Vec<u8> {
    let donnees: Vec<u8> = (0..trames)
        .flat_map(|i| (((i % 100) as i16 - 50) * 200).to_le_bytes())
        .collect();
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + donnees.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&22_050u32.to_le_bytes());
    w.extend_from_slice(&44_100u32.to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(donnees.len() as u32).to_le_bytes());
    w.extend_from_slice(&donnees);
    w
}

/// Une voix qui note ce qu'on lui fait lire, et rend un son deux fois plus long à vitesse lente.
#[derive(Default)]
pub struct VoixDeTest {
    pub en_panne: std::cell::Cell<bool>,
    pub lus: std::cell::RefCell<Vec<(String, bool)>>,
}

#[async_trait::async_trait(?Send)]
impl mine_de_rien::adapters::voix::base::Voix for VoixDeTest {
    async fn dire(
        &self,
        texte: &str,
        lent: bool,
    ) -> Result<Vec<u8>, mine_de_rien::adapters::voix::base::ErreurVoix> {
        if self.en_panne.get() {
            return Err(mine_de_rien::adapters::voix::base::ErreurVoix(
                "La voix locale est injoignable (essai).".to_owned(),
            ));
        }
        self.lus.borrow_mut().push((texte.to_owned(), lent));
        Ok(wav(if lent { 4_410 } else { 2_205 }))
    }
}

/// Enregistre `texte` comme une dictée confirmée et rend l'exercice du trou `mot`, tel qu'une
/// séance le mettrait en file : avec les numéros des sons de sa phrase et celui de la dictée.
pub fn trou_en_base(
    depot: &mine_de_rien::storage::sqlite::Depot,
    texte: &str,
    mot: &str,
) -> mine_de_rien::exercises::base::Exercice {
    use mine_de_rien::exercises::francais::dictee::{candidats, decouper};
    let sonores: Vec<mine_de_rien::storage::sqlite::PhraseSonore> = decouper(texte)
        .iter()
        .map(|p| mine_de_rien::storage::sqlite::PhraseSonore {
            texte: p.texte.clone(),
            son: b"OggS".to_vec(),
            son_lent: b"OggS".to_vec(),
        })
        .collect();
    let jour = chrono::NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
    let dictee = depot
        .ajouter_dictee(
            jour,
            texte,
            &sonores,
            "111",
            instant("2026-11-08T18:00:00+00:00"),
        )
        .unwrap();
    let (phrases, numeros) = mine_de_rien::dictees::phrases_enregistrees(depot, dictee).unwrap();
    let position = candidats(&phrases)
        .into_iter()
        .find(|p| phrases[p.phrase].mots[p.mot].texte == mot)
        .unwrap_or_else(|| panic!("{mot} absent"));
    let mut ex =
        mine_de_rien::dictees::exercice_sonore(&phrases, position, numeros[position.phrase])
            .unwrap();
    ex.donnees
        .insert("dictee".to_owned(), serde_json::json!(dictee));
    ex
}
