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
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::LazyLock;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use regex::Regex;

use crate::adapters::channel::base::{Canal, MessageParent};
use crate::adapters::game::rcon::ErreurRcon;
use crate::adapters::llm::base::Llm;
use crate::alea::Alea;
use crate::config::Coffre as ConfigCoffre;
use crate::exercises::base::Exercice;
use crate::games::atelier::{ErreurAtelier, pos};
use crate::games::coffre_du_soir::{
    Coffre, CoffreDuSoir, ErreurCoffre, Notificateur, formater_duree,
};
use crate::games::dictee;
use crate::games::emplacement::{construire_pres_de, deplacer_pres_de};
use crate::games::paisible::ModePaisible;
use crate::games::pixel::EVENEMENT_INDICE;
use crate::rapport::{nombre_indices, rapport};
use crate::saisie::{ErreurSaisie, NOMBRE_MAX, Source, interpreter};
use crate::storage::sqlite::{Demande, Depot, ErreurStockage, ExerciceStocke, Statut, maintenant};
use crate::util::strip;
use crate::{journal, photo};

pub const INTERVALLE: Duration = Duration::from_secs(3);
pub const DELAI_SUIVANT: TimeDelta = TimeDelta::seconds(10);

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
    /// Le pack de sons des dictées, s'il y en a un : le jeu ne le charge qu'à la connexion.
    pack: Option<PathBuf>,
    /// Début de la connexion en cours de l'enfant ; `None` s'il est absent, ou s'il était déjà
    /// là quand l'agent a démarré (on ne sait pas depuis quand).
    connecte_depuis: Cell<Option<DateTime<Utc>>>,
    premier_tour: Cell<bool>,
    /// Le coffre de dictée pour lequel la reconnexion a déjà été demandée.
    reconnexion_demandee: Cell<Option<i64>>,
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
            pack: None,
            connecte_depuis: Cell::new(None),
            premier_tour: Cell::new(true),
            reconnexion_demandee: Cell::new(None),
        }
    }

    /// Avec un pack de sons : un coffre de dictée attend que le jeu de l'enfant l'ait chargé.
    pub fn avec_pack(mut self, pack: &Path) -> Self {
        self.pack = Some(pack.to_path_buf());
        self
    }

    /// Le jeu de l'enfant a-t-il les sons du pack actuel ? Il ne le charge qu'en se
    /// connectant : il faut qu'il se soit connecté après la dernière écriture du pack.
    fn sons_charges(&self) -> bool {
        let Some(pack) = &self.pack else {
            return true;
        };
        if !self.en_ligne.get() {
            // Il se connectera plus tard, donc après.
            return true;
        }
        let ecrit_le = std::fs::metadata(pack)
            .and_then(|m| m.modified())
            .map(DateTime::<Utc>::from);
        match (self.connecte_depuis.get(), ecrit_le) {
            (Some(depuis), Ok(ecrit_le)) => depuis > ecrit_le,
            _ => false,
        }
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
        } else if vient_de_se_connecter && !self.premier_tour.get() {
            self.connecte_depuis.set(Some((self.horloge)()));
        }
        self.premier_tour.set(false);
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
            if self.coffre.preuve_trouvee(actif).await? {
                let quand = (self.horloge)();
                self.depot.marquer_ouvert(actif.id, quand)?;
                self.coffre.ranger(actif).await?;
                let duree = self.depot.lire(actif.id)?.and_then(|ouvert| ouvert.duree());
                let aide = nombre_indices(self.depot.compter(EVENEMENT_INDICE, actif.id)?);
                self.notificateur
                    .notifier(&format!(
                        "Coffre n°{} ouvert en {}, {aide} ({} → {}).",
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
        if vient_de_se_connecter {
            self.coffre.rappeler(actif).await?;
        }
        Ok(())
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
        if dictee::est_dictee(&suivant.exercice) && !self.sons_charges() {
            if self.reconnexion_demandee.get() != Some(suivant.id) {
                self.reconnexion_demandee.set(Some(suivant.id));
                self.coffre.demander_de_se_reconnecter().await?;
                self.notificateur
                    .notifier(&format!(
                        "Coffre n°{} en attente : l'enfant doit se déconnecter puis se \
                         reconnecter pour que son jeu charge les sons de la dictée.",
                        suivant.id
                    ))
                    .await;
            }
            return Ok(());
        }
        self.coffre.poser(&suivant).await?;
        self.depot.marquer_pose(suivant.id, (self.horloge)())?;
        if self.en_ligne.get() {
            // La clé d'abord : le mode d'emploi de l'annonce en parle.
            self.coffre.equiper().await?;
            self.depot.marquer_niveaux_donnes(suivant.id)?;
            self.coffre.annoncer(&suivant).await?;
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
        (commande(&message.texte).0 == "/atelier").then_some(RECHERCHE_EN_COURS)
    }

    pub async fn traiter(&self, message: &MessageParent) -> Result<Option<String>, ErreurCommande> {
        if !self.parents.contains(&message.expediteur) {
            return Ok(None);
        }
        if let Some(image) = &message.image {
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
            "/rapport" => rapport(&self.depot, (self.horloge)())?,
            "/oui" => self.confirmer(message.expediteur)?,
            "/non" => {
                if self
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
            _ => AIDE.to_owned(),
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
        let est_dictee = retire.is_some_and(|s| dictee::est_dictee(&s.exercice));
        if statut == Some(Statut::Pose)
            && let Some(coffre) = &self.coffre
            && let Err(e) = coffre.retirer(est_dictee).await
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
                && let Err(e) = coffre.retirer(dictee::est_dictee(&actif.exercice)).await
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
    }
}
