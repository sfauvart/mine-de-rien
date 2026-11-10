//! Mode paisible : tant qu'il reste un coffre à faire, les monstres laissent l'enfant tranquille.
//!
//! **Le désir.** Aller à l'atelier et chercher son calcul sans se faire interrompre par un zombie.
//!
//! **Commandes**, vérifiées sur Minecraft Java 26.3 (Paper) :
//!
//! ```text
//!     difficulty            → « The difficulty is Peaceful »
//!     difficulty peaceful   → « The difficulty has been set to Peaceful »
//!                             « The difficulty did not change; it is already set to Peaceful »
//! ```
//!
//! La difficulté est relue dans le monde à chaque tour de surveillance et changée seulement si
//! elle diffère de l'attendue. Rien n'est gardé en base : `/annuler`, un redémarrage du serveur ou
//! de l'agent et un changement de monde sont couverts sans code de plus. Le serveur conserve la
//! difficulté d'un démarrage à l'autre.
//!
//! **Ce que voit l'enfant** : une ligne dans le chat quand la difficulté vient de changer, s'il est
//! connecté. Une bascule survenue pendant son absence n'est pas annoncée plus tard. Rien ne part
//! sur Telegram.
//!
//! **Contournements.**
//! - *Laisser le coffre non résolu pour rester tranquille* : accepté, les parents voient les
//!   coffres en attente avec `/file`.
//! - *Changer la difficulté soi-même* : réservé aux opérateurs, et l'enfant ne doit pas l'être.
//!   Un `difficulty` tapé à la main par un parent est corrigé au tour suivant ; pour garder la main,
//!   `[paisible] actif = false`.
//!
//! **Effets de bord.** La difficulté vaut pour tout le serveur. En paisible, les monstres présents
//! disparaissent et la faim ne descend plus.

use std::rc::Rc;
use std::sync::LazyLock;

use regex::Regex;

use crate::adapters::game::base::AdaptateurJeu;
use crate::games::coffre_du_soir::ErreurCoffre;
use crate::minecraft::texte::{self, PseudoInvalide, Style};
use crate::util::strip;

pub const PAISIBLE: &str = "peaceful";
pub const MESSAGE_PAISIBLE: &str = "Les monstres s'endorment tant qu'un coffre t'attend.";
pub const MESSAGE_RETOUR: &str = "Plus de coffre : les monstres sont de retour !";
static LECTURE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"The difficulty is (\w+)").unwrap());
// « already set » : quelqu'un d'autre l'a réglée entre la lecture et l'écriture.
static ECRITURE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new("has been set to|already set to").unwrap());

pub struct ModePaisible {
    jeu: Rc<dyn AdaptateurJeu>,
    pseudo: String,
    difficulte_normale: String,
}

impl ModePaisible {
    pub fn new(
        jeu: Rc<dyn AdaptateurJeu>,
        pseudo: &str,
        difficulte_normale: &str,
    ) -> Result<Self, PseudoInvalide> {
        Ok(Self {
            jeu,
            pseudo: texte::verifier_pseudo(pseudo)?.to_owned(),
            difficulte_normale: difficulte_normale.to_owned(),
        })
    }

    async fn difficulte(&self) -> Result<String, ErreurCoffre> {
        let reponse = self.jeu.commande("difficulty").await?;
        match LECTURE.captures(&reponse) {
            Some(lecture) => Ok(lecture[1].to_lowercase()),
            None => Err(ErreurCoffre::Coffre(format!(
                "difficulté illisible : {}",
                strip(&reponse)
            ))),
        }
    }

    /// Aligne le serveur sur l'attendu et prévient l'enfant connecté.
    ///
    /// Renvoie la nouvelle difficulté si elle a changé.
    pub async fn appliquer(
        &self,
        coffre_a_faire: bool,
        en_ligne: bool,
    ) -> Result<Option<String>, ErreurCoffre> {
        let attendue = if coffre_a_faire {
            PAISIBLE
        } else {
            self.difficulte_normale.as_str()
        };
        if self.difficulte().await? == attendue {
            return Ok(None);
        }
        let reponse = self.jeu.commande(&format!("difficulty {attendue}")).await?;
        if !ECRITURE.is_match(&reponse) {
            return Err(ErreurCoffre::Coffre(format!(
                "le serveur a refusé « difficulty {attendue} » : {}",
                strip(&reponse)
            )));
        }
        if en_ligne {
            let (message, couleur) = if coffre_a_faire {
                (MESSAGE_PAISIBLE, "gray")
            } else {
                (MESSAGE_RETOUR, "yellow")
            };
            self.jeu
                .dire(&self.pseudo, message, &[("color", Style::Texte(couleur))])
                .await?;
        }
        Ok(Some(attendue.to_owned()))
    }
}
