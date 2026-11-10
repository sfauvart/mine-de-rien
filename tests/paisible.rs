//! La difficulté du serveur suit les coffres. Réponses relevées sur Minecraft Java 26.3.

mod commun;

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::games::coffre_du_soir::ErreurCoffre;
use mine_de_rien::games::paisible::{MESSAGE_PAISIBLE, MESSAGE_RETOUR, ModePaisible};
use mine_de_rien::minecraft::texte::Style;

const PSEUDO: &str = "Zorglub42";

#[derive(Clone, Copy, PartialEq)]
enum Humeur {
    Normale,
    /// Refuse d'écrire la difficulté.
    Refus,
    /// Ne répond rien à rien.
    Muette,
}

struct FauxServeur {
    difficulte: RefCell<String>,
    humeur: Humeur,
    commandes: RefCell<Vec<String>>,
    messages: RefCell<Vec<String>>,
}

impl FauxServeur {
    fn nouveau(difficulte: &str, humeur: Humeur) -> Rc<Self> {
        Rc::new(Self {
            difficulte: RefCell::new(difficulte.to_owned()),
            humeur,
            commandes: RefCell::new(Vec::new()),
            messages: RefCell::new(Vec::new()),
        })
    }

    fn ecritures(&self) -> Vec<String> {
        self.commandes
            .borrow()
            .iter()
            .filter(|c| c.starts_with("difficulty "))
            .cloned()
            .collect()
    }
}

fn majuscule(texte: &str) -> String {
    let mut lettres = texte.chars();
    lettres
        .next()
        .map(|c| c.to_uppercase().chain(lettres).collect())
        .unwrap_or_default()
}

#[async_trait(?Send)]
impl AdaptateurJeu for FauxServeur {
    async fn commande(&self, c: &str) -> Result<String, ErreurRcon> {
        self.commandes.borrow_mut().push(c.to_owned());
        if self.humeur == Humeur::Muette {
            return Ok(String::new());
        }
        if c == "difficulty" {
            return Ok(format!("The difficulty is {}", self.difficulte.borrow()));
        }
        if let Some(demandee) = c.strip_prefix("difficulty ") {
            if self.humeur == Humeur::Refus {
                return Ok("Unknown or incomplete command".to_owned());
            }
            let demandee = majuscule(demandee);
            if demandee == *self.difficulte.borrow() {
                return Ok(format!(
                    "The difficulty did not change; it is already set to {demandee}"
                ));
            }
            *self.difficulte.borrow_mut() = demandee.clone();
            return Ok(format!("The difficulty has been set to {demandee}"));
        }
        Ok(String::new())
    }

    async fn dire(&self, _: &str, texte: &str, _: &[(&str, Style<'_>)]) -> Result<(), ErreurRcon> {
        self.messages.borrow_mut().push(texte.to_owned());
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat ici".to_owned()))
    }
}

fn mode(serveur: &Rc<FauxServeur>, normale: &str) -> ModePaisible {
    ModePaisible::new(serveur.clone(), PSEUDO, normale).unwrap()
}

#[tokio::test]
async fn appliquer_renvoie_la_difficulte_quand_elle_change() {
    let serveur = FauxServeur::nouveau("Easy", Humeur::Normale);
    let mode = mode(&serveur, "hard");
    assert_eq!(
        mode.appliquer(true, false).await.unwrap().as_deref(),
        Some("peaceful")
    );
    assert_eq!(*serveur.difficulte.borrow(), "Peaceful");
    assert_eq!(
        mode.appliquer(false, false).await.unwrap().as_deref(),
        Some("hard")
    );
    assert_eq!(*serveur.difficulte.borrow(), "Hard");
}

#[tokio::test]
async fn difficulte_deja_correcte_aucune_ecriture() {
    let serveur = FauxServeur::nouveau("Peaceful", Humeur::Normale);
    assert_eq!(
        mode(&serveur, "easy").appliquer(true, true).await.unwrap(),
        None
    );
    assert!(serveur.ecritures().is_empty());
    assert!(serveur.messages.borrow().is_empty());
}

#[tokio::test]
async fn ecriture_refusee() {
    let serveur = FauxServeur::nouveau("Easy", Humeur::Refus);
    let erreur = mode(&serveur, "easy")
        .appliquer(true, false)
        .await
        .unwrap_err();
    assert!(matches!(erreur, ErreurCoffre::Coffre(_)));
    assert!(erreur.to_string().contains("refusé"));
}

#[tokio::test]
async fn lecture_illisible() {
    let serveur = FauxServeur::nouveau("Easy", Humeur::Muette);
    let erreur = mode(&serveur, "easy")
        .appliquer(true, false)
        .await
        .unwrap_err();
    assert!(erreur.to_string().contains("illisible"));
}

#[tokio::test]
async fn un_message_a_chaque_bascule_enfant_connecte() {
    let serveur = FauxServeur::nouveau("Easy", Humeur::Normale);
    let mode = mode(&serveur, "easy");
    mode.appliquer(true, true).await.unwrap();
    mode.appliquer(true, true).await.unwrap();
    assert_eq!(*serveur.messages.borrow(), [MESSAGE_PAISIBLE]);
    mode.appliquer(false, true).await.unwrap();
    assert_eq!(
        *serveur.messages.borrow(),
        [MESSAGE_PAISIBLE, MESSAGE_RETOUR]
    );
}

#[tokio::test]
async fn aucun_message_enfant_deconnecte() {
    let serveur = FauxServeur::nouveau("Easy", Humeur::Normale);
    let mode = mode(&serveur, "easy");
    assert_eq!(
        mode.appliquer(true, false).await.unwrap().as_deref(),
        Some("peaceful")
    );
    assert_eq!(
        mode.appliquer(false, false).await.unwrap().as_deref(),
        Some("easy")
    );
    assert!(serveur.messages.borrow().is_empty());
}

#[test]
fn les_messages_ne_portent_aucun_chiffre() {
    assert!(
        !format!("{MESSAGE_PAISIBLE}{MESSAGE_RETOUR}")
            .chars()
            .any(char::is_numeric)
    );
}

#[test]
fn un_pseudo_dangereux_est_refuse_a_la_creation() {
    let serveur = FauxServeur::nouveau("Easy", Humeur::Normale);
    assert!(ModePaisible::new(serveur, "@a", "easy").is_err());
}

/// Sur cinq serveurs et plusieurs suites de bascules : mêmes commandes, mêmes messages à
/// l'enfant et mêmes refus que la version Python.
#[tokio::test]
async fn oracle_python() {
    for cas in commun::oracle("paisible") {
        let e = &cas["entree"];
        let rejeu = Rc::new(commun::Rejeu::new(&cas));
        let mode =
            ModePaisible::new(rejeu.clone(), PSEUDO, e["normale"].as_str().unwrap()).unwrap();
        let mut sorties = Vec::new();
        let mut erreur = None;
        for bascule in e["suite"].as_array().unwrap() {
            let (a_faire, en_ligne) =
                (bascule[0].as_bool().unwrap(), bascule[1].as_bool().unwrap());
            match mode.appliquer(a_faire, en_ligne).await {
                Ok(sortie) => sorties.push(sortie),
                Err(e) => {
                    erreur = Some(e);
                    break;
                }
            }
        }
        commun::conclure(&cas, &rejeu, erreur.map_or(Ok(sorties), Err));
    }
}
