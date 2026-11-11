//! Pixel : l'assistant que l'enfant appelle dans le chat, pour revenir au coffre ou être aidé.
//!
//! **Le désir.** L'enfant joue loin de l'atelier, ou il bloque devant le coffre. Il tape
//! « Pixel, ramène-moi au coffre » ou « Pixel, aide-moi » ; il se retrouve devant le coffre, ou
//! reçoit un indice, de plus en plus précis à chaque demande, jamais la réponse.
//!
//! **Déclencheur.** Seuls les messages du pseudo de l'enfant qui commencent par le nom du
//! personnage (casse et accents ignorés), suivi d'une virgule, de deux-points ou d'une espace.
//! Le reste du chat n'est ni lu plus loin, ni envoyé nulle part. Pixel ne parle jamais le
//! premier : seule une ligne de l'annonce du coffre dit qu'il existe.
//!
//! **Classement.** Des mots-clés d'abord (« aide », « ramène »…) : à l'essai du 2026-11-08, le
//! LLM rangeait « aide-moi » dans le bavardage. **Le LLM, s'il est local**, ne classe que ce que
//! les mots-clés ne reconnaissent pas, dans une liste fermée (`teleporter`, `indice`,
//! `bavardage`), et reformule l'indice calculé par `exercises/indices.rs`. Il reçoit le message
//! (200 caractères au plus, sans le nom), l'énoncé et l'indice, jamais la réponse ni le pseudo.
//! Il n'écrit aucune commande : c'est ce module qui agit. Sans LLM, avec un LLM distant ou en
//! erreur, les mots-clés suffisent et l'indice part tel quel.
//!
//! **Garde-fous sur ce que dit le LLM**, appliqués ici, par le code :
//! - une reformulation qui contient la réponse (`contient_la_reponse`), un nombre absent de
//!   l'indice et de l'énoncé (calcul inventé), ou plus de 200 caractères est remplacée par
//!   l'indice brut ;
//! - un bavardage qui contient un chiffre est remplacé par une phrase fixe : Pixel n'est pas une
//!   calculatrice pour les vrais devoirs ;
//! - les émojis sont retirés : le chat de Minecraft ne les affiche pas.
//!
//! **Commande**, vérifiée sur Minecraft Java 26.3 (Paper), `C` = coffre de l'atelier :
//!
//! ```text
//!     execute in minecraft:overworld run tp <pseudo> Cx Cy Cz+2 180 0
//! ```
//!
//! depuis n'importe quelle dimension : l'enfant arrive au centre du bloc, sur le plancher au sud
//! du coffre, face à lui (« Teleported … », ou « No entity was found » s'il est déconnecté).
//! La position est relue dans le monde (`atelier.lire_atelier`) : sans atelier construit, rien ne
//! garantit que ce bloc soit de l'air, et Pixel refuse.
//!
//! **Rapport.** Chaque indice et chaque téléportation est noté dans `evenements` avec le numéro
//! du coffre ; aucun texte du chat n'est gardé.
//!
//! **Contournements.**
//! - *Faire agir Pixel* (objets, heure, mode de jeu) : liste fermée, aucune commande libre.
//! - *Lui faire dire la réponse* : le LLM ne la connaît pas, et le filtre la retire quand même.
//! - *S'en servir pour les vrais devoirs* : il ne répond à aucun calcul (chiffres interdits).
//! - *Se téléporter pour fuir un danger* : seulement quand un coffre attend, une fois toutes les
//!   5 minutes, et toujours au même endroit.
//! - *Enchaîner les indices* : permis (choix des parents), le dernier palier se répète, et les
//!   parents voient le compte.
//! - *Un autre joueur qui appelle Pixel* : ignoré, seul le pseudo de l'enfant compte.

use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::sync::LazyLock;

use chrono::{DateTime, TimeDelta, Utc};
use regex::Regex;
use serde_json::Value;
use tokio::task::JoinHandle;
use unicode_general_category::{GeneralCategory, get_general_category};

use crate::adapters::game::base::{AdaptateurJeu, MessageChat};
use crate::adapters::game::rcon::ErreurRcon;
use crate::adapters::llm::base::{ErreurLlm, Llm, lire_json};
use crate::dictees::Lecteur;
use crate::exercises::base::{Exercice, normaliser};
use crate::exercises::indices::{contient_la_reponse, indices};
use crate::games::atelier::{Atelier, PAS_CONSTRUIT, lire_atelier};
use crate::games::dictee;
use crate::journal;
use crate::minecraft::texte::{self, LONGUEUR_MAX, PseudoInvalide, Style};
use crate::storage::sqlite::{Depot, ErreurStockage};
use crate::util::{debut, est_espace, longueur, mots, repr, strip};

pub const NOM_DEFAUT: &str = "Pixel";
static NOM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-zÀ-ÖØ-öø-ÿ]{2,16}$").unwrap());
pub const LONGUEUR_MESSAGE: usize = 200;
pub const LONGUEUR_REPONSE: usize = 200;
pub const DELAI_TELEPORTATION: TimeDelta = TimeDelta::minutes(5);
pub const COULEUR: &str = "aqua";

/// Ce que l'enfant demande à Pixel : une liste fermée, aucune commande libre.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Intention {
    Teleporter,
    Indice,
    Bavardage,
}

impl Intention {
    pub const TOUTES: [Intention; 3] = [Self::Teleporter, Self::Indice, Self::Bavardage];

    pub fn nom(self) -> &'static str {
        match self {
            Self::Teleporter => "teleporter",
            Self::Indice => "indice",
            Self::Bavardage => "bavardage",
        }
    }

    pub fn lire(nom: &str) -> Option<Self> {
        Self::TOUTES.into_iter().find(|i| i.nom() == nom)
    }
}

/// Genre de l'événement noté à chaque indice donné par Pixel.
pub const EVENEMENT_INDICE: &str = "indice";
pub const EVENEMENT_TELEPORTATION: &str = "teleportation";

const MOTS_TELEPORTER: [&str; 7] = [
    "ramene",
    "teleporte",
    "teleport",
    "tp",
    "amene",
    "emmene",
    "retour",
];
const MOTS_INDICE: [&str; 13] = [
    "aide",
    "indice",
    "bloque",
    "help",
    "comment",
    "sais pas",
    "trouve pas",
    "comprends pas",
    "comprend pas",
    "combien",
    "reponse",
    "veut dire",
    "signifie",
];
// `\d` au sens d'Unicode, comme en Python : un chiffre d'une autre écriture reste un chiffre.
static CHIFFRE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d").unwrap());
static NOMBRE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\d+(?:[.,]\d+)?").unwrap());
static HORS_LETTRES: LazyLock<Regex> = LazyLock::new(|| Regex::new("[^a-z]+").unwrap());

pub const SYSTEME_CLASSEMENT: &str = "Tu ranges le message d'un enfant qui joue à Minecraft dans \
    une seule catégorie : \"teleporter\" s'il veut aller ou revenir au coffre, \"indice\" s'il \
    demande de l'aide pour trouver la réponse du coffre, \"bavardage\" pour tout le reste.";
pub const SYSTEME_REFORMULATION: &str = "Tu es Pixel, un assistant gentil dans Minecraft, qui \
    parle à un enfant de 10 ans. Tu reformules l'indice qu'on te donne en une ou deux phrases \
    courtes, chaleureuses, en français, sans émoji. Tu n'ajoutes aucune information, aucun \
    calcul, aucun résultat, et tu ne dis pas que l'enfant a déjà trouvé quelque chose.";
pub const SYSTEME_BAVARDAGE: &str = "Tu es Pixel, un assistant gentil dans Minecraft, qui parle à \
    un enfant de 10 ans. Tu réponds en une ou deux phrases courtes, en français, sans aucun \
    chiffre ni calcul, sans émoji. Tu ne peux rien faire dans le jeu, à part donner un indice \
    pour le coffre ou ramener l'enfant au coffre : ne prétends jamais avoir fait autre chose.";

fn consigne_classement(message: &str) -> String {
    format!("Réponds en JSON : {{\"intention\": \"...\"}}. Message : « {message} »")
}

pub const ATTENTE: &str = "…";
pub const OCCUPE: &str = "Je réfléchis encore, une question à la fois !";
pub const PAS_DE_COFFRE: &str = "Pas de coffre en ce moment. Je te préviendrai par l'annonce.";
pub const PATIENCE: &str = "Encore un peu de patience : une téléportation toutes les 5 minutes.";
pub const TELEPORTE: &str = "Te voilà devant le coffre !";
pub const HORS_LIGNE: &str = "Je n'arrive pas à te déplacer, réessaie dans un instant.";
pub const ATELIER_ABSENT: &str =
    "L'atelier n'est pas encore construit, je ne peux pas t'y emmener. Préviens tes parents.";

fn phrase_calcul(nom: &str) -> String {
    format!(
        "Je ne fais pas les calculs, mais je peux t'aider pour le coffre : dis « {nom}, aide-moi »."
    )
}

fn incompris(nom: &str) -> String {
    format!("Dis-moi « {nom}, aide-moi » ou « {nom}, ramène-moi au coffre ».")
}

/// Pixel ne peut pas être créé : pseudo ou nom refusé.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct NomInvalide(pub String);

impl From<PseudoInvalide> for NomInvalide {
    fn from(e: PseudoInvalide) -> Self {
        Self(e.to_string())
    }
}

/// Ce qui empêche Pixel de répondre ; il le journalise et attend la demande suivante.
#[derive(Debug, thiserror::Error)]
pub enum ErreurPixel {
    #[error("{0}")]
    Rcon(#[from] ErreurRcon),
    #[error("{0}")]
    Stockage(#[from] ErreurStockage),
}

pub fn verifier_nom(nom: &str) -> Result<&str, NomInvalide> {
    if NOM.is_match(nom) {
        Ok(nom)
    } else {
        Err(NomInvalide(format!(
            "[assistant] nom : 2 à 16 lettres, sans espace ({})",
            repr(nom)
        )))
    }
}

/// Le reste du message s'il commence par `nom` (« Pixel, aide-moi » → « aide-moi »).
pub fn adresse<'a>(message: &'a str, nom: &str) -> Option<&'a str> {
    const SEPARATEURS: [char; 4] = [',', ':', ' ', '\t'];
    let depart = message.trim_start_matches(est_espace);
    let tete = debut(depart, longueur(nom));
    if normaliser(tete) != normaliser(nom) {
        return None;
    }
    let reste = &depart[tete.len()..];
    if reste
        .chars()
        .next()
        .is_some_and(|c| !SEPARATEURS.contains(&c))
    {
        return None;
    }
    Some(strip(reste.trim_start_matches(SEPARATEURS)))
}

fn sans_accents(message: &str) -> String {
    format!(
        " {} ",
        HORS_LETTRES.replace_all(&normaliser_espaces(message), " ")
    )
}

/// Minuscules sans accents, espaces gardées (contrairement à `normaliser`).
pub fn normaliser_espaces(message: &str) -> String {
    mots(message).map(normaliser).collect::<Vec<_>>().join(" ")
}

pub fn classer_par_mots_cles(message: &str) -> Intention {
    let mots = sans_accents(message);
    let commence = |mot: &&str| mots.contains(&format!(" {mot}"));
    if MOTS_TELEPORTER.iter().any(commence) {
        Intention::Teleporter
    } else if MOTS_INDICE.iter().any(commence) {
        Intention::Indice
    } else {
        Intention::Bavardage
    }
}

/// Devant le coffre, sur le plancher au sud, regard vers le nord (vers le coffre).
pub fn commande_teleportation(atelier: &Atelier, pseudo: &str) -> Result<String, PseudoInvalide> {
    let (x, y, z) = atelier.coffre();
    Ok(format!(
        "execute in minecraft:overworld run tp {} {x} {y} {} 180 0",
        texte::verifier_pseudo(pseudo)?,
        z + 2
    ))
}

/// Réponse du LLM affichable : sans émojis ni symboles graphiques, espaces simples.
pub fn propre(brut: &str) -> String {
    let garde: String = texte::nettoyer(brut, LONGUEUR_MAX)
        .chars()
        .filter(|&c| {
            let categorie = get_general_category(c);
            !matches!(
                categorie,
                GeneralCategory::OtherSymbol
                    | GeneralCategory::Surrogate
                    | GeneralCategory::PrivateUse
            ) && !('\u{fe00}'..='\u{fe0f}').contains(&c)
        })
        .collect();
    mots(&garde).collect::<Vec<_>>().join(" ")
}

fn nombres(chaine: &str) -> HashSet<String> {
    NOMBRE
        .find_iter(&chaine.replace(' ', ""))
        .map(|n| n.as_str().replace('.', ","))
        .collect()
}

fn reformulation_sure(brut: &str, palier: &str, ex: &Exercice) -> Option<String> {
    let phrase = propre(brut);
    if phrase.is_empty() || longueur(&phrase) > LONGUEUR_REPONSE || contient_la_reponse(&phrase, ex)
    {
        return None;
    }
    let permis: HashSet<String> = nombres(palier)
        .union(&nombres(&ex.enonce))
        .cloned()
        .collect();
    nombres(&phrase).is_subset(&permis).then_some(phrase)
}

pub type Horloge = Box<dyn Fn() -> DateTime<Utc>>;

pub struct Pixel {
    jeu: Rc<dyn AdaptateurJeu>,
    depot: Rc<Depot>,
    pseudo: String,
    nom: String,
    llm: Option<Rc<dyn Llm>>,
    horloge: Horloge,
    tache: RefCell<Option<JoinHandle<()>>>,
    /// Ce qui relit la phrase d'une dictée, lentement : le premier indice.
    lecteur: RefCell<Option<Rc<Lecteur>>>,
}

impl Pixel {
    pub fn new(
        jeu: Rc<dyn AdaptateurJeu>,
        depot: Rc<Depot>,
        pseudo: &str,
        nom: &str,
        llm: Option<Rc<dyn Llm>>,
        horloge: Horloge,
    ) -> Result<Rc<Self>, NomInvalide> {
        Ok(Rc::new(Self {
            jeu,
            depot,
            pseudo: texte::verifier_pseudo(pseudo)?.to_owned(),
            nom: verifier_nom(nom)?.to_owned(),
            // Le message de l'enfant ne part jamais vers un LLM distant.
            llm: llm.filter(|llm| !llm.distant()),
            horloge,
            tache: RefCell::new(None),
            lecteur: RefCell::new(None),
        }))
    }

    /// Avec de quoi relire les phrases de dictée à l'enfant, par le chat vocal.
    pub fn avec_lecteur(&self, lecteur: Rc<Lecteur>) {
        *self.lecteur.borrow_mut() = Some(lecteur);
    }

    /// Vrai si Pixel classe et reformule avec un LLM local ; sinon, mots-clés seuls.
    pub fn avec_llm(&self) -> bool {
        self.llm.is_some()
    }

    async fn dire(&self, phrase: &str, couleur: &str) -> Result<(), ErreurRcon> {
        self.jeu
            .dire(
                &self.pseudo,
                &format!("<{}> {phrase}", self.nom),
                &[("color", Style::Texte(couleur))],
            )
            .await
    }

    /// Suit le chat aussi longtemps que l'agent tourne.
    pub async fn ecouter(self: &Rc<Self>) -> Result<(), ErreurRcon> {
        let mut flux = self.jeu.chat()?;
        loop {
            let message = flux.suivant().await;
            if let Err(e) = self.recevoir(&message).await {
                journal::avertissement(format!("Pixel n'a pas pu répondre : {e}"));
            }
        }
    }

    /// Filtre, puis traite en tâche de fond : le chat continue d'être lu pendant ce temps.
    pub async fn recevoir(self: &Rc<Self>, message: &MessageChat) -> Result<(), ErreurPixel> {
        let Some(demande) = self.demande(message) else {
            return Ok(());
        };
        let occupe = self
            .tache
            .borrow()
            .as_ref()
            .is_some_and(|tache| !tache.is_finished());
        if occupe {
            self.dire(OCCUPE, "gray").await?;
            return Ok(());
        }
        self.dire(ATTENTE, "gray").await?;
        let pixel = Rc::clone(self);
        let tache = tokio::task::spawn_local(async move {
            if let Err(e) = pixel.repondre(&demande).await {
                journal::avertissement(format!("Pixel n'a pas pu répondre : {e}"));
            }
        });
        *self.tache.borrow_mut() = Some(tache);
        Ok(())
    }

    /// Attend la fin de la réponse en cours, s'il y en a une.
    pub async fn attendre(&self) {
        let tache = self.tache.borrow_mut().take();
        if let Some(tache) = tache {
            let _ = tache.await;
        }
    }

    fn demande(&self, message: &MessageChat) -> Option<String> {
        if message.joueur != self.pseudo {
            return None;
        }
        let reste = adresse(&message.texte, &self.nom)?;
        Some(debut(reste, LONGUEUR_MESSAGE).to_owned())
    }

    pub async fn repondre(&self, demande: &str) -> Result<(), ErreurPixel> {
        if demande.is_empty() {
            self.dire(&incompris(&self.nom), COULEUR).await?;
            return Ok(());
        }
        match self.classer(demande).await {
            Intention::Teleporter => self.teleporter().await,
            Intention::Indice => self.indice(demande).await,
            Intention::Bavardage => self.bavarder(demande).await,
        }
    }

    pub async fn classer(&self, demande: &str) -> Intention {
        let par_mots = classer_par_mots_cles(demande);
        let Some(llm) = self
            .llm
            .as_ref()
            .filter(|_| par_mots == Intention::Bavardage)
        else {
            return par_mots;
        };
        let brut = llm
            .completer(
                SYSTEME_CLASSEMENT,
                &consigne_classement(demande),
                None,
                true,
            )
            .await;
        match brut.and_then(|brut| lire_json(&brut)) {
            Ok(reponse) => reponse
                .get("intention")
                .and_then(Value::as_str)
                .and_then(Intention::lire)
                .unwrap_or(Intention::Bavardage),
            Err(e) => {
                journal::info(format!("classement par mots-clés, LLM indisponible : {e}"));
                Intention::Bavardage
            }
        }
    }

    async fn teleporter(&self) -> Result<(), ErreurPixel> {
        let Some(actif) = self.depot.actif()? else {
            self.dire(PAS_DE_COFFRE, COULEUR).await?;
            return Ok(());
        };
        let maintenant = (self.horloge)();
        let recents = self
            .depot
            .evenements_entre(maintenant - DELAI_TELEPORTATION, maintenant)?;
        if recents.iter().any(|e| e.genre == EVENEMENT_TELEPORTATION) {
            self.dire(PATIENCE, COULEUR).await?;
            return Ok(());
        }
        let Some(atelier) = lire_atelier(self.jeu.as_ref()).await? else {
            journal::avertissement(format!("téléportation refusée : {PAS_CONSTRUIT}"));
            self.dire(ATELIER_ABSENT, COULEUR).await?;
            return Ok(());
        };
        let commande = commande_teleportation(&atelier, &self.pseudo).map_err(ErreurRcon::from)?;
        let reponse = self.jeu.commande(&commande).await?;
        if !reponse.starts_with("Teleported") {
            self.dire(HORS_LIGNE, COULEUR).await?;
            return Ok(());
        }
        self.depot
            .noter(EVENEMENT_TELEPORTATION, Some(actif.id), maintenant)?;
        self.dire(TELEPORTE, COULEUR).await?;
        Ok(())
    }

    async fn indice(&self, demande: &str) -> Result<(), ErreurPixel> {
        let Some(actif) = self.depot.actif()? else {
            self.dire(PAS_DE_COFFRE, COULEUR).await?;
            return Ok(());
        };
        let ex = &actif.exercice;
        let paliers = indices(ex);
        let deja = self.depot.compter(EVENEMENT_INDICE, actif.id)?;
        let rang = deja.min(paliers.len() - 1);
        let palier = &paliers[rang];
        if dictee::est_dictee(ex) {
            // Aucun LLM devant une dictée : il pourrait épeler le mot, ou mal l'écrire.
            self.depot
                .noter(EVENEMENT_INDICE, Some(actif.id), (self.horloge)())?;
            if rang > 0 {
                self.dire(palier, COULEUR).await?;
                return Ok(());
            }
            let lecteur = self.lecteur.borrow().clone();
            let Some(lecteur) = lecteur.filter(|_| dictee::son(ex).is_some()) else {
                self.dire(dictee::SANS_SON, COULEUR).await?;
                return Ok(());
            };
            self.dire(palier, COULEUR).await?;
            if let Err(e) = lecteur.dire(ex, true).await {
                journal::avertissement(format!("phrase de dictée non relue : {e}"));
                self.dire(dictee::SANS_SON, COULEUR).await?;
            }
            return Ok(());
        }
        let mut phrase = palier.clone();
        if let Some(llm) = &self.llm {
            let consigne = format!(
                "Énoncé du coffre : {}\nIndice à reformuler : {palier}\nMessage de l'enfant : {demande}",
                ex.enonce
            );
            match llm
                .completer(SYSTEME_REFORMULATION, &consigne, None, false)
                .await
            {
                Ok(brut) => {
                    if let Some(sure) = reformulation_sure(&brut, palier, ex) {
                        phrase = sure;
                    }
                }
                Err(e) => journal::info(format!("indice brut, LLM indisponible : {e}")),
            }
        }
        self.depot
            .noter(EVENEMENT_INDICE, Some(actif.id), (self.horloge)())?;
        self.dire(&phrase, COULEUR).await?;
        Ok(())
    }

    async fn bavarder(&self, demande: &str) -> Result<(), ErreurPixel> {
        if self
            .depot
            .actif()?
            .is_some_and(|actif| dictee::est_dictee(&actif.exercice))
        {
            self.dire(dictee::PAS_DE_BAVARDAGE, COULEUR).await?;
            return Ok(());
        }
        let Some(llm) = &self.llm else {
            self.dire(&incompris(&self.nom), COULEUR).await?;
            return Ok(());
        };
        let brut = match llm.completer(SYSTEME_BAVARDAGE, demande, None, false).await {
            Ok(brut) => brut,
            Err(e) => {
                let e: ErreurLlm = e;
                journal::info(format!("bavardage sans LLM : {e}"));
                self.dire(&incompris(&self.nom), COULEUR).await?;
                return Ok(());
            }
        };
        let mut phrase = propre(&brut);
        let actif = self.depot.actif()?;
        if phrase.is_empty()
            || CHIFFRE.is_match(&phrase)
            || longueur(&phrase) > LONGUEUR_REPONSE
            || actif.is_some_and(|actif| contient_la_reponse(&phrase, &actif.exercice))
        {
            phrase = phrase_calcul(&self.nom);
        }
        self.dire(&phrase, COULEUR).await?;
        Ok(())
    }
}
