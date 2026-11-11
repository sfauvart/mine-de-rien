//! Boucle de surveillance : pose les coffres de la file, voit quand l'enfant les ouvre.
//!
//! Toutes les 3 secondes :
//! 1. l'enfant est-il connecté (`list`) ? À sa connexion, un rappel s'il a un coffre actif ;
//! 2. un coffre est actif : s'il est connecté, clé et niveaux s'il ne les a pas eus, puis
//!    recherche de la preuve dans son inventaire ; si le coffre a disparu sans être ouvert, il
//!    est reposé et l'événement noté ;
//! 3. pas de coffre actif : le suivant de la file est posé, 10 secondes au moins après
//!    l'ouverture du précédent, et seulement quand le coffre est vide ;
//! 4. la difficulté suit les coffres : paisible tant qu'il en reste un à faire (`games/paisible.rs`).
//!
//! Un coffre de dictée se lit à voix haute : il n'est posé que si l'enfant est là et son jeu
//! relié au chat vocal. Absent, la dictée attend son retour. Présent sans chat vocal, elle
//! attend aussi, et le premier coffre qui n'est pas une dictée passe devant. Le lien se fait quelques secondes après la connexion : avant de conclure qu'il
//! manque, la boucle attend `DELAI_CHAT_VOCAL`.
//!
//! La position de l'atelier est relue dans le monde à chaque tour. S'il n'y en a pas, aucun coffre
//! n'est posé, et l'enfant n'est pas envoyé vers un coffre inaccessible : les parents sont prévenus
//! une fois, et la boucle réessaie à chaque tour.
//!
//! La boucle ne s'arrête pas sur une erreur du serveur (redémarrage, RCON coupé) : elle la
//! signale et réessaie au tour suivant.
//!
//! Les commandes des parents (`/atelier`, `/coffre`, `/file`, `/annuler`, `/rapport`, une photo de
//! fiche puis `/oui` ou `/non`) arrivent par un canal ; `CommandesParents` les traite,
//! indépendamment de Telegram.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use chrono_tz::Tz;
use regex::Regex;

use crate::adapters::channel::base::{Canal, MessageParent};
use crate::adapters::game::rcon::ErreurRcon;
use crate::adapters::llm::base::Llm;
use crate::adapters::voix::base::Voix;
use crate::alea::Alea;
use crate::config::Coffre as ConfigCoffre;
use crate::dictees::{self, ErreurDictees, Preparation, ecrire_jour, lire_jour};
use crate::exercises::base::{Exercice, meme_cle, normaliser};
use crate::exercises::francais::dictee::imposes_absents;
use crate::games::atelier::{ErreurAtelier, pos};
use crate::games::coffre_du_soir::{
    Coffre, CoffreDuSoir, ErreurCoffre, Notificateur, formater_duree,
};
use crate::games::dictee;
use crate::games::emplacement::{construire_pres_de, deplacer_pres_de};
use crate::games::paisible::ModePaisible;
use crate::games::pixel::EVENEMENT_INDICE;
use crate::rapport::{essais_rates, nombre_indices, rapport};
use crate::saisie::{ErreurSaisie, NOMBRE_MAX, Source, interpreter};
use crate::seance::{self, creer_seance_du_jour, jour_local};
use crate::storage::sqlite::{Demande, Depot, ErreurStockage, ExerciceStocke, Statut, maintenant};
use crate::util::{mots, strip};
use crate::{journal, photo};

pub const INTERVALLE: Duration = Duration::from_secs(3);
pub const DELAI_SUIVANT: TimeDelta = TimeDelta::seconds(10);
/// Le temps laissé au jeu de l'enfant, après sa connexion, pour se relier au chat vocal.
pub const DELAI_CHAT_VOCAL: TimeDelta = TimeDelta::seconds(20);

/// Ce qui interrompt un tour de surveillance ; la boucle le journalise et repart.
#[derive(Debug, thiserror::Error)]
pub enum ErreurTour {
    #[error("{0}")]
    Coffre(#[from] ErreurCoffre),
    #[error("{0}")]
    Stockage(#[from] ErreurStockage),
}

pub type Horloge = Box<dyn Fn() -> DateTime<Utc>>;

pub struct Orchestrateur {
    depot: Rc<Depot>,
    coffre: Rc<dyn Coffre>,
    notificateur: Rc<dyn Notificateur>,
    horloge: Horloge,
    paisible: Option<ModePaisible>,
    en_ligne: Cell<bool>,
    vider_demande: Cell<bool>,
    atelier_signale: RefCell<Option<String>>,
    /// Début de la connexion en cours de l'enfant, telle que l'agent l'a vue ; `None` s'il
    /// est absent.
    connecte_depuis: Cell<Option<DateTime<Utc>>>,
    /// Le jour où le chat vocal manquant a été signalé, à l'enfant et aux parents.
    chat_vocal_signale: Cell<Option<NaiveDate>>,
    /// Le jour où une voix en panne a été signalée aux parents.
    voix_signalee: Cell<Option<NaiveDate>>,
    /// L'enfant vient de se connecter devant un coffre de dictée : le rappel attend que son
    /// jeu soit relié au chat vocal, pour que la phrase s'entende.
    rappel_en_attente: Cell<bool>,
    /// Le fuseau de la maison, pour compter « une fois par jour » ; UTC s'il n'est pas réglé.
    fuseau: Cell<Tz>,
    /// Le dernier nom lu sur la clé, pour le coffre actif : un nom nouveau est un essai.
    cle_vue: RefCell<Option<(i64, Option<String>)>>,
    /// Les séances de dictée quotidiennes, si elles sont réglées.
    seances: Option<Seances>,
}

/// Ce qu'il faut pour créer la séance de dictée du jour à la connexion de l'enfant.
pub struct Seances {
    pub coffres: usize,
    pub fuseau: Tz,
    pub recompense: ConfigCoffre,
    pub alea: RefCell<Alea>,
}

impl Orchestrateur {
    pub fn new(
        depot: Rc<Depot>,
        coffre: Rc<dyn Coffre>,
        notificateur: Rc<dyn Notificateur>,
        horloge: Horloge,
        paisible: Option<ModePaisible>,
    ) -> Self {
        Self {
            depot,
            coffre,
            notificateur,
            horloge,
            paisible,
            en_ligne: Cell::new(false),
            vider_demande: Cell::new(false),
            atelier_signale: RefCell::new(None),
            connecte_depuis: Cell::new(None),
            chat_vocal_signale: Cell::new(None),
            voix_signalee: Cell::new(None),
            rappel_en_attente: Cell::new(false),
            fuseau: Cell::new(chrono_tz::UTC),
            cle_vue: RefCell::new(None),
            seances: None,
        }
    }

    /// Avec les séances de dictée : à la première connexion de chaque jour, quelques coffres
    /// de la dictée à préparer entrent dans la file.
    pub fn avec_seances(
        mut self,
        coffres: usize,
        fuseau: Tz,
        recompense: ConfigCoffre,
        alea: Alea,
    ) -> Self {
        self.fuseau.set(fuseau);
        self.seances = Some(Seances {
            coffres,
            fuseau,
            recompense,
            alea: RefCell::new(alea),
        });
        self
    }

    /// L'enfant vient de se connecter : la séance de dictée du jour, si elle n'existe pas
    /// encore. Une dictée illisible ne doit pas arrêter la surveillance des coffres.
    async fn seance_du_jour(&self) -> Result<(), ErreurTour> {
        let Some(seances) = &self.seances else {
            return Ok(());
        };
        let demande = Demande {
            source: Source::Dictee,
            demande_par: "seance",
            recompense: &seances.recompense.recompense,
            recompense_nombre: seances.recompense.recompense_nombre,
        };
        let creee = creer_seance_du_jour(
            &self.depot,
            (self.horloge)(),
            seances.fuseau,
            seances.coffres,
            &mut seances.alea.borrow_mut(),
            demande,
        );
        match creee {
            Ok(Some(seance)) => self.notificateur.notifier(&seance.annonce()).await,
            Ok(None) => {}
            Err(ErreurDictees::Stockage(e)) => return Err(e.into()),
            Err(e) => journal::avertissement(format!("séance de dictée impossible : {e}")),
        }
        Ok(())
    }

    /// Refait le tableau de la dictée à partir du coffre `reference`, relu dans la base. Un
    /// tableau impossible à dessiner n'arrête pas le coffre : la phrase est dans le chat.
    async fn dessiner_tableau(&self, reference: i64) -> Result<(), ErreurTour> {
        let Some(reference) = self.depot.lire(reference)? else {
            return Ok(());
        };
        match dictees::tableau(&self.depot, &reference) {
            Ok(Some(morceaux)) => self.coffre.afficher_tableau(&morceaux).await?,
            Ok(None) => {}
            Err(ErreurDictees::Stockage(e)) => return Err(e.into()),
            Err(e) => journal::avertissement(format!("tableau de dictée impossible : {e}")),
        }
        Ok(())
    }

    /// Relève ce que l'enfant a écrit sur la clé. Le premier nom lu devant un coffre est le
    /// point de départ (c'est la réponse du coffre précédent, pas un essai) ; ensuite, chaque
    /// nom nouveau est une tentative, juste ou non. Rien n'en est redit dans le jeu ni écrit
    /// dans le journal.
    ///
    /// Limites : deux essais dans le même tour de 3 secondes n'en font qu'un ; un essai fait
    /// avec un autre objet que la clé, ou resté sur l'enclume sans être pris, n'est pas vu.
    async fn relever_essai(&self, actif: &ExerciceStocke) -> Result<(), ErreurTour> {
        let nom = self.coffre.nom_de_la_cle().await?;
        let connue = self
            .cle_vue
            .borrow()
            .as_ref()
            .filter(|(coffre, _)| *coffre == actif.id)
            .map(|(_, nom)| nom.clone());
        let Some(precedent) = connue else {
            *self.cle_vue.borrow_mut() = Some((actif.id, nom));
            return Ok(());
        };
        // Une clé qu'on ne lit plus (sur l'enclume, au bout du curseur) n'efface rien.
        if let Some(nouveau) = nom
            && precedent.as_deref() != Some(nouveau.as_str())
        {
            let juste = meme_cle(&nouveau, &actif.exercice.reponse);
            self.depot
                .noter_tentative(actif.id, &nouveau, juste, (self.horloge)())?;
            if juste {
                // Le verrou ne connaît que le nom exact : il prend celui que l'enfant a écrit.
                self.coffre.accepter_la_cle(&nouveau).await?;
            }
            *self.cle_vue.borrow_mut() = Some((actif.id, Some(nouveau)));
        }
        Ok(())
    }

    pub async fn tour(&self) -> Result<(), ErreurTour> {
        let coffre = Rc::clone(&self.coffre);
        let _verrou = coffre.verrou().lock().await;
        self.tour_verrouille().await
    }

    async fn tour_verrouille(&self) -> Result<(), ErreurTour> {
        let en_ligne = self.coffre.joueur_en_ligne().await?;
        let vient_de_se_connecter = en_ligne && !self.en_ligne.get();
        self.en_ligne.set(en_ligne);
        if !en_ligne {
            self.connecte_depuis.set(None);
            self.rappel_en_attente.set(false);
        } else if vient_de_se_connecter {
            // Déjà là au démarrage de l'agent, il est traité comme s'il arrivait.
            self.connecte_depuis.set(Some((self.horloge)()));
        }
        if vient_de_se_connecter {
            self.seance_du_jour().await?;
        }
        self.coffre.actualiser_atelier().await?;

        let resultat = match self.depot.actif()? {
            Some(actif) => self.surveiller(&actif, vient_de_se_connecter).await,
            None => self.poser_suivant().await,
        };
        match resultat {
            Err(ErreurTour::Coffre(ErreurCoffre::AtelierAbsent(raison))) => {
                let deja_dit = self.atelier_signale.borrow().as_deref() == Some(raison.as_str());
                if !deja_dit {
                    *self.atelier_signale.borrow_mut() = Some(raison.clone());
                    self.notificateur
                        .notifier(&format!("Aucun coffre ne peut être posé : {raison}"))
                        .await;
                }
            }
            Err(autre) => return Err(autre),
            Ok(()) => *self.atelier_signale.borrow_mut() = None,
        }
        self.ajuster_difficulte().await
    }

    /// Relu après la pose ou l'ouverture de ce tour. Sans atelier, aucun coffre n'existe.
    async fn ajuster_difficulte(&self) -> Result<(), ErreurTour> {
        let Some(paisible) = &self.paisible else {
            return Ok(());
        };
        let reste_un_coffre = self.depot.actif()?.is_some() || self.depot.suivant()?.is_some();
        paisible
            .appliquer(
                self.coffre.atelier().is_some() && reste_un_coffre,
                self.en_ligne.get(),
            )
            .await?;
        Ok(())
    }

    async fn surveiller(
        &self,
        actif: &ExerciceStocke,
        vient_de_se_connecter: bool,
    ) -> Result<(), ErreurTour> {
        if self.en_ligne.get() {
            if !actif.niveaux_donnes {
                self.coffre.equiper().await?;
                self.depot.marquer_niveaux_donnes(actif.id)?;
            }
            // Avant la preuve : un essai juste suivi de l'ouverture dans le même tour est noté.
            self.relever_essai(actif).await?;
            if self.coffre.preuve_trouvee(actif).await? {
                let quand = (self.horloge)();
                self.depot.marquer_ouvert(actif.id, quand)?;
                self.coffre.ranger(actif).await?;
                if dictee::est_dictee(&actif.exercice) {
                    // Le mot trouvé s'écrit en vert à sa place.
                    self.dessiner_tableau(actif.id).await?;
                }
                let duree = self.depot.lire(actif.id)?.and_then(|ouvert| ouvert.duree());
                let aide = nombre_indices(self.depot.compter(EVENEMENT_INDICE, actif.id)?);
                let essais = essais_rates(&self.depot.tentatives(actif.id)?)
                    .map(|rates| format!(" Essais : {rates}."))
                    .unwrap_or_default();
                self.notificateur
                    .notifier(&format!(
                        "Coffre n°{} ouvert en {}, {aide} ({} → {}).{essais}",
                        actif.id,
                        formater_duree(duree),
                        actif.exercice.enonce,
                        actif.exercice.reponse
                    ))
                    .await;
                return Ok(());
            }
        }
        self.coffre.verifier_atelier()?;
        if !self.coffre.coffre_present().await? {
            self.coffre.poser(actif).await?;
            self.depot
                .noter("coffre_disparu", Some(actif.id), (self.horloge)())?;
            self.notificateur
                .notifier(&format!(
                    "Coffre n°{} disparu sans être ouvert : il a été reposé.",
                    actif.id
                ))
                .await;
            return Ok(());
        }
        if !dictee::est_dictee(&actif.exercice) {
            if vient_de_se_connecter {
                self.coffre.rappeler(actif).await?;
            }
            return Ok(());
        }
        if vient_de_se_connecter {
            self.dessiner_tableau(actif.id).await?;
            self.rappel_en_attente.set(true);
        }
        if self.rappel_en_attente.get() && self.en_ligne.get() {
            // Le rappel lit la phrase : il attend que le jeu de l'enfant puisse l'entendre.
            let relie = self.coffre.chat_vocal_relie().await?;
            if relie || !self.chat_vocal_attendu() {
                self.rappel_en_attente.set(false);
                self.sons_de_la_dictee(actif).await?;
                self.coffre.rappeler(actif).await?;
                if !relie {
                    self.signaler_le_chat_vocal(actif.id).await?;
                }
            }
        }
        Ok(())
    }

    /// Le jour qu'il est à la maison.
    fn aujourdhui(&self) -> NaiveDate {
        jour_local((self.horloge)(), self.fuseau.get())
    }

    /// Vrai tant que le jeu de l'enfant, tout juste connecté, peut encore être en train de se
    /// relier au chat vocal.
    fn chat_vocal_attendu(&self) -> bool {
        self.connecte_depuis
            .get()
            .is_some_and(|depuis| (self.horloge)() - depuis < DELAI_CHAT_VOCAL)
    }

    /// Prépare les sons de la phrase d'un coffre de dictée. `false` si la voix ne répond pas :
    /// les parents sont prévenus une fois par jour, et rien ne s'arrête.
    async fn sons_de_la_dictee(&self, s: &ExerciceStocke) -> Result<bool, ErreurTour> {
        match self.coffre.preparer_la_dictee(s).await {
            Ok(()) => Ok(true),
            Err(ErreurCoffre::Coffre(raison)) => {
                let aujourdhui = self.aujourdhui();
                if self.voix_signalee.get() != Some(aujourdhui) {
                    self.voix_signalee.set(Some(aujourdhui));
                    self.notificateur
                        .notifier(&format!(
                            "Coffre n°{} : la phrase de la dictée ne peut pas être lue ({raison}). \
                             Les coffres de dictée attendent, les autres continuent.",
                            s.id
                        ))
                        .await;
                }
                Ok(false)
            }
            Err(autre) => Err(autre.into()),
        }
    }

    /// Le jeu de l'enfant n'a pas le chat vocal : il le lit, et les parents sont prévenus, une
    /// fois par jour.
    async fn signaler_le_chat_vocal(&self, coffre: i64) -> Result<(), ErreurTour> {
        let aujourdhui = self.aujourdhui();
        if self.chat_vocal_signale.get() == Some(aujourdhui) {
            return Ok(());
        }
        self.chat_vocal_signale.set(Some(aujourdhui));
        self.coffre.demander_le_chat_vocal().await?;
        self.notificateur
            .notifier(&format!(
                "Coffre n°{coffre} : le jeu de l'enfant n'est pas relié au chat vocal, il \
                 n'entendrait pas la dictée (mod Simple Voice Chat, voir \
                 docs/rejoindre-le-serveur.md). Les coffres de dictée attendent, les autres \
                 continuent."
            ))
            .await;
        Ok(())
    }

    /// Le coffre à poser maintenant : le premier de la file, sauf si c'est une dictée que
    /// l'enfant ne pourrait pas entendre ; alors le premier coffre qui n'en est pas une.
    /// `None` : rien à poser à ce tour.
    async fn choisir_suivant(
        &self,
        premier: ExerciceStocke,
    ) -> Result<Option<ExerciceStocke>, ErreurTour> {
        if !dictee::est_dictee(&premier.exercice) {
            return Ok(Some(premier));
        }
        let prete = self.sons_de_la_dictee(&premier).await?;
        if prete {
            // Absent, on ne sait pas si son jeu a le chat vocal : la dictée attend son retour,
            // ses sons prêts, et garde sa place dans la file.
            if !self.en_ligne.get() {
                return Ok(None);
            }
            if self.coffre.chat_vocal_relie().await? {
                return Ok(Some(premier));
            }
            // Tout juste connecté, son jeu est peut-être encore en train de se relier.
            if self.chat_vocal_attendu() {
                return Ok(None);
            }
            self.signaler_le_chat_vocal(premier.id).await?;
        }
        Ok(self
            .depot
            .en_file()?
            .into_iter()
            .find(|s| !dictee::est_dictee(&s.exercice)))
    }

    async fn poser_suivant(&self) -> Result<(), ErreurTour> {
        let Some(suivant) = self.depot.suivant()? else {
            return Ok(());
        };
        // Avant d'attendre le coffre vide : celui d'un autre monde ne le sera jamais.
        self.coffre.verifier_atelier()?;
        if let Some(ouvert_le) = self
            .depot
            .dernier_ouvert()?
            .and_then(|dernier| dernier.ouvert_le)
            && (self.horloge)() - ouvert_le < DELAI_SUIVANT
        {
            return Ok(());
        }
        if !self.coffre.coffre_vide().await? {
            if self.en_ligne.get() && !self.vider_demande.get() {
                self.coffre.demander_de_vider().await?;
                self.vider_demande.set(true);
            }
            return Ok(());
        }
        self.vider_demande.set(false);
        let Some(suivant) = self.choisir_suivant(suivant).await? else {
            return Ok(());
        };
        // Lu avant la pose : le coffre d'avant était-il une dictée, avec son tableau ?
        let apres_une_dictee = self
            .depot
            .dernier_ouvert()?
            .is_some_and(|dernier| dictee::est_dictee(&dernier.exercice));
        self.coffre.poser(&suivant).await?;
        self.depot.marquer_pose(suivant.id, (self.horloge)())?;
        if dictee::est_dictee(&suivant.exercice) {
            self.dessiner_tableau(suivant.id).await?;
        } else if apres_une_dictee {
            self.coffre.effacer_tableau().await?;
        }
        if self.en_ligne.get() {
            // La clé d'abord : le mode d'emploi de l'annonce en parle.
            self.coffre.equiper().await?;
            self.depot.marquer_niveaux_donnes(suivant.id)?;
            self.coffre.annoncer(&suivant).await?;
            // Le nom que porte la clé à cet instant est le point de départ des essais.
            let depart = self.coffre.nom_de_la_cle().await?;
            *self.cle_vue.borrow_mut() = Some((suivant.id, depart));
        }
        self.notificateur
            .notifier(&format!(
                "Coffre n°{} posé : {} (réponse {}).",
                suivant.id, suivant.exercice.enonce, suivant.exercice.reponse
            ))
            .await;
        Ok(())
    }

    /// Tourne aussi longtemps que l'agent. Une erreur est journalisée, le tour suivant repart.
    pub async fn tourner(&self, intervalle: Duration) {
        loop {
            if let Err(e) = self.tour().await {
                journal::avertissement(format!("tour de surveillance interrompu : {e}"));
            }
            tokio::time::sleep(intervalle).await;
        }
    }
}

/// Ajoute les exercices à la file ; une ligne par coffre.
pub fn ajouter_exercices(
    depot: &Depot,
    coffre: &ConfigCoffre,
    exercices: &[Exercice],
    source: Source,
    demande_par: &str,
) -> Result<Vec<String>, ErreurStockage> {
    let demande = Demande {
        source,
        demande_par,
        recompense: &coffre.recompense,
        recompense_nombre: coffre.recompense_nombre,
    };
    let mut lignes = Vec::with_capacity(exercices.len());
    for ex in exercices {
        let numero = depot.ajouter(ex, demande, maintenant())?;
        lignes.push(format!(
            "Coffre n°{numero} en file : {} (réponse {})",
            ex.enonce, ex.reponse
        ));
    }
    Ok(lignes)
}

/// La saisie est refusée (message pour le parent), ou la base n'a pas voulu l'écrire.
#[derive(Debug, thiserror::Error)]
pub enum ErreurFile {
    #[error("{0}")]
    Saisie(#[from] ErreurSaisie),
    #[error("{0}")]
    Stockage(#[from] ErreurStockage),
}

/// Ajoute à la file les exercices saisis ; une ligne par coffre.
pub fn mettre_en_file(
    depot: &Depot,
    coffre: &ConfigCoffre,
    texte: &str,
    demande_par: &str,
    alea: &mut Alea,
) -> Result<Vec<String>, ErreurFile> {
    let saisie = interpreter(texte, alea)?;
    Ok(ajouter_exercices(
        depot,
        coffre,
        &saisie.exercices,
        saisie.source,
        demande_par,
    )?)
}

pub const NOMBRE_PHOTO: usize = 3;
pub const DUREE_ATTENTE: TimeDelta = TimeDelta::minutes(30);
static LEGENDE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^\s*([0-9]+)?\s*(.*)$").unwrap());
pub const LECTURE_EN_COURS: &str = "Je lis la photo… (jusqu'à deux minutes)";
pub const RECHERCHE_EN_COURS: &str = "Je cherche un emplacement près de l'enfant…";
pub const RAPPEL_FIDELITE: &str = "Compare cette liste à la fiche : si elle ne correspond pas, \
    réponds /non et renvoie la photo à l'endroit.";

#[derive(Debug, Clone, PartialEq)]
pub struct LectureEnAttente {
    pub exercices: Vec<Exercice>,
    pub quand: DateTime<Utc>,
}

pub const PREPARATION_EN_COURS: &str = "Je prépare la dictée… (jusqu'à une minute)";
/// Nombre de coffres d'une séance de dictée.
pub const COFFRES_PAR_SEANCE: usize = 4;
pub const AIDE_DICTEE: &str = "\n\
    /dictee vendredi <texte> : la dictée à préparer pour vendredi (ou 13/11), \
    puis /oui après avoir comparé et écouté\n\
    /dictee vendredi mots : plus, trois : une liste de mots sans texte\n\
    Photo de la feuille, « dictée vendredi » en légende\n\
    /dictee : les dictées en cours\n\
    /dictee seance 3 : une séance de coffres tout de suite, pour la dictée n°3\n\
    /dictee mots 3 plus trois : les mots à faire sortir en premier\n\
    /dictee annuler 3 : retire la dictée n°3";
pub const DICTEES_ABSENTES: &str = "Les dictées ne sont pas configurées : il faut les sections \
    [voix] de config.toml, avec dossier_du_jeu (docs/installation.md).";
pub const RAPPEL_DICTEE: &str = "Compare ce texte à la feuille, au mot près, et écoute l'aperçu : \
    chaque mot peut devenir une réponse. Si tout est juste, /oui ; sinon /non.";
/// À l'essai du 2026-11-10, le LLM local a recopié « noirades » pour « noiraudes » sur une
/// feuille imprimée nette : une lettre fausse deviendrait une réponse fausse.
pub const RAPPEL_PHOTO_DICTEE: &str = "Ce texte vient d'une photo : le modèle se trompe parfois \
    d'une lettre, surtout sur un mot rare ou un nom propre. Relis chaque mot.";
static MOTS_SEULS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)^mots\s*:\s*(.*)$").unwrap());

/// Ce qu'il faut pour préparer des dictées : une voix.
pub struct Dictees {
    pub voix: Rc<dyn Voix>,
}

/// Une dictée lue par la voix, que le parent n'a pas encore confirmée.
#[derive(Debug, Clone, PartialEq)]
pub struct DicteeEnAttente {
    pub preparation: Preparation,
    pub jour: NaiveDate,
    pub quand: DateTime<Utc>,
}

pub const AIDE: &str = "Commandes :\n\
    /atelier : construit l'atelier près de l'enfant connecté, ou l'y déplace\n\
    /coffre 348 ÷ 12 : un calcul recopié du cahier\n\
    /coffre division 3 : trois exercices d'une compétence (division, anglais)\n\
    Photo d'une fiche, « 5 » en légende : cinq exercices tirés au hasard, \
    puis /oui pour les mettre en file ou /non\n\
    /file : le coffre actif et ceux qui attendent\n\
    /annuler 12 : retire le coffre n°12\n\
    /annuler tout : retire le coffre actif et vide la file\n\
    /rapport : les 7 derniers jours";

/// La commande en minuscules et son argument (« /coffre@MonBot 12 » dans certains clients).
fn commande(texte: &str) -> (String, &str) {
    let (commande, argument) = strip(texte).split_once(' ').unwrap_or((strip(texte), ""));
    (
        commande
            .split('@')
            .next()
            .unwrap_or_default()
            .to_lowercase(),
        strip(argument),
    )
}

/// Ce qui empêche de répondre à un parent autrement que par « Erreur : … ».
#[derive(Debug, thiserror::Error)]
pub enum ErreurCommande {
    #[error("{0}")]
    Rcon(#[from] ErreurRcon),
    #[error("{0}")]
    Coffre(#[from] ErreurCoffre),
    #[error("{0}")]
    Stockage(#[from] ErreurStockage),
}

/// Traite les messages des parents. Ne répond rien à un expéditeur non autorisé.
pub struct CommandesParents {
    depot: Rc<Depot>,
    config_coffre: ConfigCoffre,
    coffre: Option<Rc<CoffreDuSoir>>,
    parents: HashSet<i64>,
    horloge: Horloge,
    llm: Option<Rc<dyn Llm>>,
    alea: RefCell<Alea>,
    en_attente: RefCell<HashMap<i64, LectureEnAttente>>,
    dictees: Option<Dictees>,
    dictees_en_attente: RefCell<HashMap<i64, DicteeEnAttente>>,
    /// Fichier audio à envoyer au parent après la réponse en cours : nom et contenu.
    audio: RefCell<HashMap<i64, (String, Vec<u8>)>>,
    /// Le fuseau de la maison, pour dater les dictées ; UTC tant qu'il n'est pas réglé.
    fuseau: Tz,
    coffres_par_seance: usize,
}

impl CommandesParents {
    pub fn new(
        depot: Rc<Depot>,
        config_coffre: ConfigCoffre,
        coffre: Option<Rc<CoffreDuSoir>>,
        parents: &[i64],
        horloge: Horloge,
        llm: Option<Rc<dyn Llm>>,
        alea: Alea,
    ) -> Self {
        Self {
            depot,
            config_coffre,
            coffre,
            parents: parents.iter().copied().collect(),
            horloge,
            llm,
            alea: RefCell::new(alea),
            en_attente: RefCell::new(HashMap::new()),
            dictees: None,
            dictees_en_attente: RefCell::new(HashMap::new()),
            audio: RefCell::new(HashMap::new()),
            fuseau: Tz::UTC,
            coffres_par_seance: COFFRES_PAR_SEANCE,
        }
    }

    /// Le fuseau de la maison et la taille d'une séance de dictée (`[dictee]`).
    pub fn avec_seances(mut self, fuseau: Tz, coffres_par_seance: usize) -> Self {
        self.fuseau = fuseau;
        self.coffres_par_seance = coffres_par_seance;
        self
    }

    /// Avec une voix : les parents peuvent envoyer des dictées.
    pub fn avec_dictees(mut self, voix: Rc<dyn Voix>) -> Self {
        self.dictees = Some(Dictees { voix });
        self
    }

    /// La dictée qu'un parent n'a pas encore confirmée, s'il y en a une.
    pub fn dictee_en_attente(&self, parent: i64) -> Option<DicteeEnAttente> {
        self.dictees_en_attente.borrow().get(&parent).cloned()
    }

    /// Le fichier audio à envoyer à ce parent après la réponse qu'il vient de recevoir.
    pub fn prendre_audio(&self, parent: i64) -> Option<(String, Vec<u8>)> {
        self.audio.borrow_mut().remove(&parent)
    }

    /// Le jour qu'il est à la maison, pour dater les dictées.
    fn aujourdhui(&self) -> NaiveDate {
        jour_local((self.horloge)(), self.fuseau)
    }

    /// Le rapport de la semaine, suivi d'un point sur les dictées en cours.
    fn rapport(&self) -> Result<String, ErreurCommande> {
        let mut lignes = vec![rapport(&self.depot, (self.horloge)())?];
        let en_cours = self.depot.dictees_depuis(self.aujourdhui())?;
        if !en_cours.is_empty() {
            lignes.push(String::new());
            lignes.push("Dictées en cours :".to_owned());
        }
        for dictee in en_cours {
            let bilan = match seance::bilan(&self.depot, dictee.id, self.fuseau) {
                Ok(bilan) => bilan,
                Err(ErreurDictees::Stockage(e)) => return Err(e.into()),
                Err(e) => {
                    lignes.push(format!("• n°{} : {e}", dictee.id));
                    continue;
                }
            };
            let fragiles = bilan
                .fragiles_en_clair()
                .map(|f| format!(" ; fragiles : {f}"))
                .unwrap_or_default();
            lignes.push(format!(
                "• n°{}, {} : {} mot(s) acquis sur {}{fragiles}",
                dictee.id,
                ecrire_jour(dictee.jour),
                bilan.acquis,
                bilan.mots
            ));
        }
        Ok(lignes.join("\n"))
    }

    fn aide(&self) -> String {
        if self.dictees.is_some() {
            format!("{AIDE}{AIDE_DICTEE}")
        } else {
            AIDE.to_owned()
        }
    }

    /// La lecture de photo qu'un parent n'a pas encore confirmée, s'il y en a une.
    pub fn lecture_en_attente(&self, parent: i64) -> Option<LectureEnAttente> {
        self.en_attente.borrow().get(&parent).cloned()
    }

    /// Message à envoyer tout de suite, avant une lecture de photo qui prend du temps.
    pub fn accuse_reception(&self, message: &MessageParent) -> Option<&'static str> {
        if !self.parents.contains(&message.expediteur) {
            return None;
        }
        if message.image.is_some() && self.llm.as_ref().is_some_and(|llm| !llm.distant()) {
            return Some(LECTURE_EN_COURS);
        }
        let (commande, argument) = commande(&message.texte);
        if commande == "/dictee" && self.dictees.is_some() && message.image.is_none() {
            let mut parties = mots(argument);
            let premier = parties.next().map(normaliser);
            let nouvelle = parties.next().is_some()
                && !matches!(premier.as_deref(), Some("annuler" | "mots" | "seance"));
            return nouvelle.then_some(PREPARATION_EN_COURS);
        }
        (commande == "/atelier").then_some(RECHERCHE_EN_COURS)
    }

    pub async fn traiter(&self, message: &MessageParent) -> Result<Option<String>, ErreurCommande> {
        if !self.parents.contains(&message.expediteur) {
            return Ok(None);
        }
        if let Some(image) = &message.image {
            if let Some(jour) = legende_de_dictee(&message.texte) {
                return Ok(Some(
                    self.photo_de_dictee(message.expediteur, jour, image)
                        .await?,
                ));
            }
            return Ok(Some(self.photo(message, image).await?));
        }
        let (commande, argument) = commande(&message.texte);
        let reponse = match commande.as_str() {
            "/atelier" => self.atelier().await?,
            "/coffre" => {
                let demande_par = message.expediteur.to_string();
                let mut alea = self.alea.borrow_mut();
                match mettre_en_file(
                    &self.depot,
                    &self.config_coffre,
                    argument,
                    &demande_par,
                    &mut alea,
                ) {
                    Ok(lignes) => lignes.join("\n"),
                    Err(ErreurFile::Saisie(e)) => format!("Refusé : {e}"),
                    Err(ErreurFile::Stockage(e)) => return Err(e.into()),
                }
            }
            "/file" => self.file()?,
            "/annuler" => self.annuler(argument).await?,
            "/rapport" => self.rapport()?,
            "/dictee" => self.dictee(message.expediteur, argument).await?,
            "/oui" if self.dictee_en_attente(message.expediteur).is_some() => {
                self.confirmer_dictee(message.expediteur)?
            }
            "/oui" => self.confirmer(message.expediteur)?,
            "/non" => {
                let dictee = self
                    .dictees_en_attente
                    .borrow_mut()
                    .remove(&message.expediteur);
                if dictee.is_some() {
                    "Abandonné : la dictée n'a pas été enregistrée.".to_owned()
                } else if self
                    .en_attente
                    .borrow_mut()
                    .remove(&message.expediteur)
                    .is_none()
                {
                    "Aucune lecture de photo en attente.".to_owned()
                } else {
                    "Abandonné : rien n'a été mis en file.".to_owned()
                }
            }
            _ => self.aide(),
        };
        Ok(Some(reponse))
    }

    async fn atelier(&self) -> Result<String, ErreurCommande> {
        let Some(coffre) = &self.coffre else {
            return Ok("Aucun serveur Minecraft configuré.".to_owned());
        };
        let _verrou = coffre.verrou().lock().await;
        coffre.actualiser_atelier().await?;
        let ancien = coffre.atelier();
        let jeu = coffre.jeu.as_ref();
        let construction = match ancien {
            None => construire_pres_de(jeu, &coffre.pseudo).await,
            Some(ancien) => {
                if let Some(refus) = self.refus_de_deplacer(coffre).await? {
                    return Ok(refus);
                }
                // Le tableau d'une séance finie ne suit pas l'atelier.
                coffre.effacer_tableau().await?;
                deplacer_pres_de(jeu, &coffre.pseudo, &ancien).await
            }
        };
        let c = match construction {
            Ok(c) => c,
            Err(ErreurAtelier::Refus(message)) => return Ok(message),
            Err(ErreurAtelier::Rcon(e)) => return Err(e.into()),
        };
        let verbe = if ancien.is_none() {
            "construit"
        } else {
            "déplacé"
        };
        Ok(format!(
            "Atelier {verbe} en {}, à {} blocs de l'enfant.",
            pos(c.atelier.coffre()),
            c.distance
        ))
    }

    async fn refus_de_deplacer(
        &self,
        coffre: &CoffreDuSoir,
    ) -> Result<Option<String>, ErreurCommande> {
        if let Some(actif) = self.depot.actif()? {
            return Ok(Some(format!(
                "Le coffre n°{0} est en cours : l'annuler avec /annuler {0}, \
                 ou attendre qu'il soit ouvert, puis relancer /atelier.",
                actif.id
            )));
        }
        if !coffre.coffre_vide().await? {
            return Ok(Some(
                "Le coffre de l'atelier contient encore des objets : l'enfant doit le vider, \
                 puis relancer /atelier."
                    .to_owned(),
            ));
        }
        Ok(None)
    }

    async fn photo(&self, message: &MessageParent, image: &[u8]) -> Result<String, ErreurCommande> {
        let Some(llm) = &self.llm else {
            return Ok(
                "La lecture de photo n'est pas configurée (section [llm] de config.toml)."
                    .to_owned(),
            );
        };
        if llm.distant() {
            return Ok(
                "La lecture de photo demande un LLM local ([llm] adaptateur = \
                       \"openai-compatible\") : une photo ne part jamais vers un LLM distant."
                    .to_owned(),
            );
        }
        let bornes =
            || format!("Entre 1 et {NOMBRE_MAX} exercices à la fois (le nombre en légende).");
        let legende = LEGENDE.captures(&message.texte);
        let nombre = match legende.as_ref().and_then(|m| m.get(1)) {
            None => NOMBRE_PHOTO,
            // Un nombre trop grand pour être lu est, à plus forte raison, hors des bornes.
            Some(n) => match n.as_str().parse::<usize>() {
                Ok(n) => n,
                Err(_) => return Ok(bornes()),
            },
        };
        if !(1..=NOMBRE_MAX).contains(&nombre) {
            return Ok(bornes());
        }
        let consigne = legende
            .as_ref()
            .and_then(|m| m.get(2))
            .map_or("", |m| m.as_str());
        // Le tirage ne garde pas l'emprunt pendant l'attente du LLM.
        let mut alea = Alea::graine(self.alea.borrow_mut().entier(0, i64::MAX) as u64);
        let lecture = match photo::lire(llm.as_ref(), image, consigne, &mut alea).await {
            Ok(lecture) => lecture,
            Err(e) => return Ok(format!("Lecture impossible : {e}")),
        };
        if lecture.exercices.is_empty() {
            return Ok("Je n'ai rien reconnu. La photo est-elle à l'endroit ?".to_owned());
        }
        let tires = photo::tirer(&lecture, nombre, &mut alea);
        let mut lignes = photo::resumer(&lecture);
        lignes.push(format!("Tirés au hasard ({}) :", tires.len()));
        lignes.extend(
            tires
                .iter()
                .map(|ex| format!("• {} (réponse {})", ex.enonce, ex.reponse)),
        );
        lignes.push(RAPPEL_FIDELITE.to_owned());
        lignes.push("/oui pour les mettre en file, /non pour abandonner.".to_owned());
        // Une seule chose à confirmer à la fois : la fiche remplace une dictée en attente.
        self.dictees_en_attente
            .borrow_mut()
            .remove(&message.expediteur);
        self.en_attente.borrow_mut().insert(
            message.expediteur,
            LectureEnAttente {
                exercices: tires,
                quand: (self.horloge)(),
            },
        );
        Ok(lignes.join("\n"))
    }

    fn confirmer(&self, parent: i64) -> Result<String, ErreurCommande> {
        let attente = self.en_attente.borrow_mut().remove(&parent);
        let Some(attente) = attente.filter(|a| (self.horloge)() - a.quand <= DUREE_ATTENTE) else {
            return Ok("Rien à confirmer : envoie d'abord la photo d'une fiche \
                       (une lecture attend 30 minutes au plus)."
                .to_owned());
        };
        let lignes = ajouter_exercices(
            &self.depot,
            &self.config_coffre,
            &attente.exercices,
            Source::Photo,
            &parent.to_string(),
        )?;
        Ok(lignes.join("\n"))
    }

    /// `/dictee …` : une nouvelle dictée, ou la gestion de celles en cours.
    async fn dictee(&self, parent: i64, argument: &str) -> Result<String, ErreurCommande> {
        if self.dictees.is_none() {
            return Ok(DICTEES_ABSENTES.to_owned());
        }
        let (premier, reste) = argument
            .split_once(char::is_whitespace)
            .unwrap_or((argument, ""));
        let reste = strip(reste);
        match normaliser(premier).as_str() {
            "" => self.liste_des_dictees(),
            "annuler" => self.annuler_dictee(reste),
            "mots" => self.imposer_des_mots(reste),
            "seance" => self.seance_de_dictee(parent, reste),
            _ if reste.is_empty() => Ok(
                "Il manque le texte : /dictee vendredi <texte de la dictée>, \
                 ou /dictee vendredi mots : plus, trois."
                    .to_owned(),
            ),
            _ => match MOTS_SEULS.captures(reste).and_then(|m| m.get(1)) {
                Some(liste) => {
                    self.nouvelle_dictee(parent, premier, liste.as_str(), true, false)
                        .await
                }
                None => {
                    self.nouvelle_dictee(parent, premier, reste, false, false)
                        .await
                }
            },
        }
    }

    /// Fait lire le texte par la voix et le garde en attente du « /oui » du parent.
    async fn nouvelle_dictee(
        &self,
        parent: i64,
        jour: &str,
        texte: &str,
        liste: bool,
        depuis_une_photo: bool,
    ) -> Result<String, ErreurCommande> {
        let Some(dictees) = &self.dictees else {
            return Ok(DICTEES_ABSENTES.to_owned());
        };
        let jour = match lire_jour(jour, self.aujourdhui()) {
            Ok(jour) => jour,
            Err(e) => return Ok(format!("Refusé : {e}")),
        };
        let preparation = match dictees::preparer(dictees.voix.as_ref(), texte, liste).await {
            Ok(preparation) => preparation,
            Err(e) => return Ok(format!("Dictée impossible : {e}")),
        };
        let mut lignes = vec![format!(
            "Dictée du {} : {} phrase(s), {} mot(s) à demander.",
            ecrire_jour(jour),
            preparation.phrases.len(),
            preparation.mots_a_demander()
        )];
        lignes.extend(
            preparation
                .phrases
                .iter()
                .enumerate()
                .map(|(i, phrase)| format!("{}. {}", i + 1, phrase.texte)),
        );
        if depuis_une_photo {
            lignes.push(RAPPEL_PHOTO_DICTEE.to_owned());
        }
        lignes.push(RAPPEL_DICTEE.to_owned());
        self.audio.borrow_mut().insert(
            parent,
            ("dictee-apercu.ogg".to_owned(), preparation.apercu.clone()),
        );
        // Une seule chose à confirmer à la fois : la dictée remplace une fiche en attente.
        self.en_attente.borrow_mut().remove(&parent);
        self.dictees_en_attente.borrow_mut().insert(
            parent,
            DicteeEnAttente {
                preparation,
                jour,
                quand: (self.horloge)(),
            },
        );
        Ok(lignes.join("\n"))
    }

    async fn photo_de_dictee(
        &self,
        parent: i64,
        jour: &str,
        image: &[u8],
    ) -> Result<String, ErreurCommande> {
        if self.dictees.is_none() {
            return Ok(DICTEES_ABSENTES.to_owned());
        }
        let Some(llm) = &self.llm else {
            return Ok(
                "La lecture de photo n'est pas configurée (section [llm] de config.toml). \
                 Le texte tapé reste possible : /dictee vendredi <texte>."
                    .to_owned(),
            );
        };
        if jour.is_empty() {
            return Ok("Il manque le jour dans la légende : « dictée vendredi ».".to_owned());
        }
        // Le jour est vérifié avant la lecture, qui prend du temps.
        if let Err(e) = lire_jour(jour, self.aujourdhui()) {
            return Ok(format!("Refusé : {e}"));
        }
        let texte = match photo::lire_texte(llm.as_ref(), image).await {
            Ok(texte) => texte,
            Err(e) => return Ok(format!("Lecture impossible : {e}")),
        };
        self.nouvelle_dictee(parent, jour, &texte, false, true)
            .await
    }

    fn confirmer_dictee(&self, parent: i64) -> Result<String, ErreurCommande> {
        let attente = self.dictees_en_attente.borrow_mut().remove(&parent);
        let (Some(attente), Some(_)) = (
            attente.filter(|a| (self.horloge)() - a.quand <= DUREE_ATTENTE),
            &self.dictees,
        ) else {
            return Ok(
                "Rien à confirmer : la dictée a attendu plus de 30 minutes, \
                       il faut la renvoyer."
                    .to_owned(),
            );
        };
        let numero = match dictees::enregistrer(
            &self.depot,
            &attente.preparation,
            attente.jour,
            &parent.to_string(),
            (self.horloge)(),
        ) {
            Ok(numero) => numero,
            Err(ErreurDictees::Stockage(e)) => return Err(e.into()),
            Err(e) => return Ok(format!("Dictée impossible à enregistrer : {e}")),
        };
        Ok(format!(
            "Dictée n°{numero} enregistrée pour le {}.\n\
             À partir de demain et jusqu'à la veille, {n} coffres l'attendront à sa première \
             connexion de chaque jour.\n\
             /dictee seance {numero} : une séance de {n} coffres tout de suite.",
            ecrire_jour(attente.jour),
            n = self.coffres_par_seance
        ))
    }

    fn liste_des_dictees(&self) -> Result<String, ErreurCommande> {
        let en_cours = self.depot.dictees_depuis(self.aujourdhui())?;
        if en_cours.is_empty() {
            return Ok("Aucune dictée en cours.".to_owned());
        }
        let mut lignes = vec!["Dictées en cours :".to_owned()];
        for dictee in en_cours {
            let phrases = self.depot.phrases_de(dictee.id)?.len();
            let imposes = if dictee.imposes.is_empty() {
                String::new()
            } else {
                format!(", mots imposés : {}", dictee.imposes.join(", "))
            };
            lignes.push(format!(
                "• n°{}, {} : {phrases} phrase(s){imposes}",
                dictee.id,
                ecrire_jour(dictee.jour)
            ));
        }
        Ok(lignes.join("\n"))
    }

    /// La dictée n°`texte`, si elle existe et n'est pas annulée ; sinon le message de refus.
    fn dictee_en_cours(
        &self,
        texte: &str,
    ) -> Result<Result<crate::storage::sqlite::DicteeStockee, String>, ErreurCommande> {
        let numero = texte.trim_start_matches("n°").parse::<i64>().ok();
        let Some(numero) = numero else {
            return Ok(Err(
                "Préciser le numéro de la dictée (/dictee pour les voir).".to_owned(),
            ));
        };
        Ok(match self.depot.lire_dictee(numero)? {
            Some(dictee) if !dictee.annulee => Ok(dictee),
            _ => Err(format!(
                "La dictée n°{numero} n'existe pas, ou elle est annulée."
            )),
        })
    }

    fn annuler_dictee(&self, argument: &str) -> Result<String, ErreurCommande> {
        let dictee = match self.dictee_en_cours(argument)? {
            Ok(dictee) => dictee,
            Err(refus) => return Ok(refus),
        };
        self.depot.annuler_dictee(dictee.id)?;
        // Ses coffres qui attendent encore dans la file partent avec elle.
        let mut retires = 0;
        for s in self.depot.en_file()? {
            let de_cette_dictee =
                s.exercice.donnees.get("dictee").and_then(|d| d.as_i64()) == Some(dictee.id);
            if de_cette_dictee && self.depot.annuler(s.id)?.is_some() {
                retires += 1;
            }
        }
        Ok(format!(
            "Dictée n°{} annulée, {retires} coffre(s) retiré(s) de la file.",
            dictee.id
        ))
    }

    fn imposer_des_mots(&self, argument: &str) -> Result<String, ErreurCommande> {
        let (numero, liste) = argument
            .split_once(char::is_whitespace)
            .unwrap_or((argument, ""));
        let dictee = match self.dictee_en_cours(numero)? {
            Ok(dictee) => dictee,
            Err(refus) => return Ok(refus),
        };
        let imposes: Vec<String> = liste
            .split([',', ';', ' ', '\n'])
            .map(strip)
            .filter(|mot| !mot.is_empty())
            .map(str::to_owned)
            .collect();
        let phrases = match dictees::phrases_enregistrees(&self.depot, dictee.id) {
            Ok((phrases, _)) => phrases,
            Err(ErreurDictees::Stockage(e)) => return Err(e.into()),
            Err(e) => return Ok(format!("Refusé : {e}")),
        };
        let absents = imposes_absents(&phrases, &imposes);
        if !absents.is_empty() {
            return Ok(format!(
                "Refusé : pas dans le texte, ou impossible à demander : {}.",
                absents.join(", ")
            ));
        }
        self.depot.imposer_mots(dictee.id, &imposes)?;
        Ok(if imposes.is_empty() {
            format!("Dictée n°{} : plus aucun mot imposé.", dictee.id)
        } else {
            format!(
                "Dictée n°{} : ces mots sortiront en premier : {}.",
                dictee.id,
                imposes.join(", ")
            )
        })
    }

    fn seance_de_dictee(&self, parent: i64, argument: &str) -> Result<String, ErreurCommande> {
        let dictee = match self.dictee_en_cours(argument)? {
            Ok(dictee) => dictee,
            Err(refus) => return Ok(refus),
        };
        let demande = Demande {
            source: Source::Dictee,
            demande_par: &parent.to_string(),
            recompense: &self.config_coffre.recompense,
            recompense_nombre: self.config_coffre.recompense_nombre,
        };
        // Les mêmes poids que la séance du jour : un mot acquis ne revient pas, un mot raté
        // hier repose.
        let poids = seance::poids(&self.depot, dictee.id, self.aujourdhui(), self.fuseau)?;
        let coffres = match dictees::seance(
            &self.depot,
            &dictee,
            self.coffres_par_seance,
            &mut self.alea.borrow_mut(),
            &poids,
            demande,
            (self.horloge)(),
        ) {
            Ok(coffres) => coffres,
            Err(ErreurDictees::Stockage(e)) => return Err(e.into()),
            Err(e) => return Ok(format!("Séance impossible : {e}")),
        };
        if coffres.is_empty() {
            return Ok(
                "Aucun mot à demander maintenant : ils sont acquis, déjà en file, ou reposent \
                 après une erreur."
                    .to_owned(),
            );
        }
        Ok(coffres
            .iter()
            .map(|(numero, ex)| {
                format!(
                    "Coffre n°{numero} en file : {} (réponse {})",
                    ex.enonce, ex.reponse
                )
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }

    fn file(&self) -> Result<String, ErreurCommande> {
        let mut lignes = Vec::new();
        match self.depot.actif()? {
            Some(actif) => {
                let depuis = match actif.pose_le {
                    Some(pose_le) => formater_duree(Some((self.horloge)() - pose_le)),
                    None => "?".to_owned(),
                };
                lignes.push(format!(
                    "Coffre actif : n°{} {} (réponse {}), posé depuis {depuis}",
                    actif.id, actif.exercice.enonce, actif.exercice.reponse
                ));
            }
            None => lignes.push("Aucun coffre actif.".to_owned()),
        }
        let attente = self.depot.en_file()?;
        if !attente.is_empty() {
            lignes.push("En attente :".to_owned());
            lignes.extend(
                attente
                    .iter()
                    .map(|s| format!("• n°{} {}", s.id, s.exercice.enonce)),
            );
        }
        Ok(lignes.join("\n"))
    }

    async fn annuler(&self, argument: &str) -> Result<String, ErreurCommande> {
        if argument.to_lowercase() == "tout" {
            return self.annuler_tout().await;
        }
        let chiffres = argument.trim_start_matches(['n', '°']);
        let numero = chiffres
            .parse::<i64>()
            .ok()
            .filter(|_| chiffres.chars().all(|c| c.is_ascii_digit()));
        let Some(numero) = numero else {
            return Ok("Préciser le numéro du coffre (/annuler 12), ou /annuler tout".to_owned());
        };
        let statut = self.depot.lire(numero)?.map(|s| s.statut);
        if !matches!(statut, Some(Statut::EnFile | Statut::Pose)) {
            return Ok(format!(
                "Le coffre n°{numero} n'est ni actif ni en attente."
            ));
        }
        let retire = self.depot.lire(numero)?;
        let dictee = retire
            .as_ref()
            .map(|s| &s.exercice)
            .filter(|ex| dictee::est_dictee(ex));
        if statut == Some(Statut::Pose)
            && let Some(coffre) = &self.coffre
            && let Err(e) = coffre.retirer(dictee).await
        {
            return Ok(format!(
                "Le coffre n°{numero} n'a pas pu être retiré du jeu, réessayer : {e}"
            ));
        }
        self.depot.annuler(numero)?;
        Ok(format!("Coffre n°{numero} annulé."))
    }

    /// La file d'abord, sans attendre le jeu : la boucle ne pourra pas poser le suivant
    /// pendant que le coffre actif est retiré.
    async fn annuler_tout(&self) -> Result<String, ErreurCommande> {
        let mut annules = Vec::new();
        for s in self.depot.en_file()? {
            if self.depot.annuler(s.id)?.is_some() {
                annules.push(format!("n°{}", s.id));
            }
        }
        let mut lignes = Vec::new();
        if !annules.is_empty() {
            lignes.push(format!("En attente annulés : {}", annules.join(", ")));
        }
        if let Some(actif) = self.depot.actif()? {
            if let Some(coffre) = &self.coffre
                && let Err(e) = coffre
                    .retirer(Some(&actif.exercice).filter(|ex| dictee::est_dictee(ex)))
                    .await
            {
                lignes.push(format!(
                    "Le coffre actif n°{} n'a pas pu être retiré du jeu, réessayer : {e}",
                    actif.id
                ));
                return Ok(lignes.join("\n"));
            }
            self.depot.annuler(actif.id)?;
            lignes.insert(0, format!("Coffre actif n°{} annulé.", actif.id));
        }
        if lignes.is_empty() {
            return Ok("Rien à annuler : aucun coffre actif ni en attente.".to_owned());
        }
        Ok(lignes.join("\n"))
    }
}

/// Les notifications de l'orchestrateur partent vers tous les parents du canal.
pub struct NotificateurCanal {
    canal: Rc<dyn Canal>,
}

impl NotificateurCanal {
    pub fn new(canal: Rc<dyn Canal>) -> Self {
        Self { canal }
    }
}

#[async_trait(?Send)]
impl Notificateur for NotificateurCanal {
    async fn notifier(&self, message: &str) {
        self.canal.envoyer(message).await;
    }
}

/// Répond aux messages des parents, un par un, aussi longtemps que le canal est ouvert.
pub async fn servir(canal: &dyn Canal, commandes: &CommandesParents) {
    while let Some(message) = canal.recevoir().await {
        if let Some(accuse) = commandes.accuse_reception(&message) {
            canal.repondre(message.expediteur, accuse).await;
        }
        let reponse = match commandes.traiter(&message).await {
            Ok(reponse) => reponse,
            Err(e) => Some(format!("Erreur : {e}")),
        };
        if let Some(reponse) = reponse.filter(|r| !r.is_empty()) {
            canal.repondre(message.expediteur, &reponse).await;
        }
        if let Some((nom, octets)) = commandes.prendre_audio(message.expediteur) {
            canal.envoyer_audio(message.expediteur, &nom, &octets).await;
        }
    }
}

/// Le jour écrit dans la légende d'une photo de dictée (« dictée vendredi » → « vendredi »),
/// ou `None` si la légende ne commence pas par « dictée » : c'est alors une fiche.
pub fn legende_de_dictee(legende: &str) -> Option<&str> {
    let legende = strip(legende);
    let (premier, reste) = legende
        .split_once(char::is_whitespace)
        .unwrap_or((legende, ""));
    (normaliser(premier) == "dictee").then(|| strip(reste))
}
