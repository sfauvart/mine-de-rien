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
use std::rc::Rc;
use std::time::Duration;

use chrono::{DateTime, TimeDelta, Utc};

use crate::alea::Alea;
use crate::config::Coffre as ConfigCoffre;
use crate::exercises::base::Exercice;
use crate::games::coffre_du_soir::{Coffre, ErreurCoffre, Notificateur, formater_duree};
use crate::games::paisible::ModePaisible;
use crate::games::pixel::EVENEMENT_INDICE;
use crate::journal;
use crate::rapport::nombre_indices;
use crate::saisie::{ErreurSaisie, Source, interpreter};
use crate::storage::sqlite::{Demande, Depot, ErreurStockage, ExerciceStocke, maintenant};

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
