//! La séance de dictée de chaque jour : quels mots redemander, et lesquels laisser reposer.
//!
//! À la première connexion de l'enfant chaque jour, quelques coffres sont tirés dans la dictée
//! à préparer, du lendemain de sa confirmation à la veille de la dictée. Un jour sans
//! connexion n'est pas rattrapé : pas de dette de coffres.
//!
//! Chaque trou a une histoire : ses apparitions passées, et ce que l'enfant a écrit devant
//! (`tentatives`). Elle décide de son poids dans le tirage (`dictee::tirer`) :
//!
//! | Histoire du trou | Poids |
//! |---|---|
//! | jamais demandé | 1 |
//! | un coffre de ce trou attend déjà | 0 |
//! | raté à sa dernière apparition, il y a deux jours ou plus | 10 |
//! | raté hier ou aujourd'hui | 0 : il repose |
//! | réussi du premier coup deux fois de suite | 0 : acquis |
//! | autrement (réussi une fois) | 1 |
//!
//! « Raté » : au moins un essai faux avant d'ouvrir. « Chaque jour » se compte dans le fuseau
//! de la maison, pas en UTC.
//!
//! **Révision.** Tant qu'une dictée est en cours, un des coffres de la séance peut reprendre
//! un mot encore raté d'une dictée passée de moins de 30 jours : la voix relit sa phrase.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use chrono_tz::Tz;
use serde_json::json;

use crate::alea::Alea;
use crate::dictees::{
    ErreurDictees, JOURS_GARDES, ecrire_jour, exercice_sonore, phrases_enregistrees, position_de,
    seance,
};
use crate::exercises::base::Exercice;
use crate::exercises::francais::dictee::{Position, candidats};
use crate::storage::sqlite::{Demande, Depot, DicteeStockee, ErreurStockage, Statut};

pub const POIDS_RATE: f64 = 10.0;
/// Un mot raté repose au moins ce nombre de jours avant de revenir.
pub const REPOS_APRES_ECHEC: i64 = 2;
/// Réussi du premier coup autant de fois de suite, un mot est acquis.
pub const REUSSITES_POUR_ACQUIS: usize = 2;

/// Le jour qu'il est à la maison.
pub fn jour_local(quand: DateTime<Utc>, fuseau: Tz) -> NaiveDate {
    quand.with_timezone(&fuseau).date_naive()
}

/// Un coffre passé ou présent pour un trou.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Apparition {
    /// Le jour, à la maison, où le coffre a été posé (ou mis en file s'il attend encore).
    pub jour: NaiveDate,
    pub ouvert: bool,
    /// Ce que l'enfant a écrit de faux, dans l'ordre.
    pub rates: Vec<String>,
}

/// Où en est un mot de la dictée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Etat {
    Nouveau,
    /// Un coffre de ce trou est en file ou posé.
    EnCours,
    /// Raté à sa dernière apparition : les graphies fausses de ce jour-là.
    Fragile(Vec<String>),
    Acquis,
    /// Réussi, mais pas encore deux fois de suite du premier coup.
    AConfirmer,
}

pub fn etat(apparitions: &[Apparition]) -> Etat {
    let Some(derniere) = apparitions.last() else {
        return Etat::Nouveau;
    };
    if !derniere.ouvert {
        return Etat::EnCours;
    }
    if !derniere.rates.is_empty() {
        return Etat::Fragile(derniere.rates.clone());
    }
    let reussites = apparitions
        .iter()
        .rev()
        .take_while(|a| a.ouvert && a.rates.is_empty())
        .count();
    if reussites >= REUSSITES_POUR_ACQUIS {
        Etat::Acquis
    } else {
        Etat::AConfirmer
    }
}

/// Le poids d'un trou dans le tirage d'aujourd'hui (voir le tableau du module).
pub fn poids_de(apparitions: &[Apparition], aujourdhui: NaiveDate) -> f64 {
    match etat(apparitions) {
        Etat::Nouveau | Etat::AConfirmer => 1.0,
        Etat::EnCours | Etat::Acquis => 0.0,
        Etat::Fragile(_) => {
            let depuis = apparitions
                .last()
                .map_or(0, |a| (aujourdhui - a.jour).num_days());
            if depuis >= REPOS_APRES_ECHEC {
                POIDS_RATE
            } else {
                0.0
            }
        }
    }
}

/// L'histoire de chaque trou déjà demandé d'une dictée, de la plus ancienne apparition à la
/// plus récente. Les coffres annulés ne comptent pas.
pub fn historique(
    depot: &Depot,
    dictee: i64,
    fuseau: Tz,
) -> Result<BTreeMap<Position, Vec<Apparition>>, ErreurStockage> {
    let mut histoire: BTreeMap<Position, Vec<Apparition>> = BTreeMap::new();
    for coffre in depot.coffres_de_dictee(dictee)? {
        if coffre.statut == Statut::Annule {
            continue;
        }
        let Some(position) = position_de(&coffre.exercice) else {
            continue;
        };
        let rates = depot
            .tentatives(coffre.id)?
            .into_iter()
            .filter(|t| !t.correcte)
            .map(|t| t.proposition)
            .collect();
        histoire.entry(position).or_default().push(Apparition {
            jour: jour_local(coffre.pose_le.unwrap_or(coffre.cree_le), fuseau),
            ouvert: coffre.statut == Statut::Ouvert,
            rates,
        });
    }
    Ok(histoire)
}

/// Les poids du tirage d'aujourd'hui, pour les trous qui ont une histoire. Les autres valent 1.
pub fn poids(
    depot: &Depot,
    dictee: i64,
    aujourdhui: NaiveDate,
    fuseau: Tz,
) -> Result<HashMap<Position, f64>, ErreurStockage> {
    Ok(historique(depot, dictee, fuseau)?
        .iter()
        .map(|(position, apparitions)| (*position, poids_de(apparitions, aujourdhui)))
        .collect())
}

/// Où en est l'enfant sur une dictée, pour les parents.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bilan {
    /// Nombre de mots qui peuvent être demandés.
    pub mots: usize,
    pub acquis: usize,
    /// Les mots ratés à leur dernière apparition, avec ce que l'enfant avait écrit.
    pub fragiles: Vec<(String, Vec<String>)>,
}

impl Bilan {
    /// « personnes (persone), année (anne) », ou `None` s'il n'y en a pas.
    pub fn fragiles_en_clair(&self) -> Option<String> {
        let mots: Vec<String> = self
            .fragiles
            .iter()
            .map(|(mot, graphies)| format!("{mot} ({})", graphies.join(", ")))
            .collect();
        (!mots.is_empty()).then(|| mots.join(", "))
    }
}

pub fn bilan(depot: &Depot, dictee: i64, fuseau: Tz) -> Result<Bilan, ErreurDictees> {
    let (phrases, _) = phrases_enregistrees(depot, dictee)?;
    let histoire = historique(depot, dictee, fuseau)?;
    let mut acquis = 0;
    let mut fragiles = Vec::new();
    for (position, apparitions) in &histoire {
        // Un coffre qui attend encore ne dit rien de plus : un mot raté lundi et redemandé
        // aujourd'hui reste fragile tant qu'il n'a pas été réussi.
        let faites: Vec<Apparition> = apparitions.iter().filter(|a| a.ouvert).cloned().collect();
        match etat(&faites) {
            Etat::Acquis => acquis += 1,
            Etat::Fragile(graphies) => {
                let mot = phrases
                    .get(position.phrase)
                    .and_then(|p| p.mots.get(position.mot))
                    .map(|m| m.texte.clone());
                if let Some(mot) = mot {
                    fragiles.push((mot, graphies));
                }
            }
            _ => {}
        }
    }
    Ok(Bilan {
        mots: candidats(&phrases).len(),
        acquis,
        fragiles,
    })
}

/// Une dictée est-elle à travailler aujourd'hui ? Du lendemain de sa confirmation à la veille
/// de son jour.
pub fn en_preparation(dictee: &DicteeStockee, aujourdhui: NaiveDate, fuseau: Tz) -> bool {
    !dictee.annulee
        && dictee.jour > aujourdhui
        && jour_local(dictee.confirmee_le, fuseau) < aujourdhui
}

/// La dictée que sert la séance d'aujourd'hui : celle dont le jour est le plus proche.
pub fn dictee_du_jour(
    depot: &Depot,
    aujourdhui: NaiveDate,
    fuseau: Tz,
) -> Result<Option<DicteeStockee>, ErreurStockage> {
    Ok(depot
        .dictees_depuis(aujourdhui)?
        .into_iter()
        .find(|d| en_preparation(d, aujourdhui, fuseau)))
}

/// Une séance de dictée a-t-elle déjà été créée aujourd'hui ?
pub fn deja_faite(depot: &Depot, aujourdhui: NaiveDate) -> Result<bool, ErreurStockage> {
    Ok(depot
        .dictees_depuis(aujourdhui - TimeDelta::days(JOURS_GARDES))?
        .iter()
        .any(|d| d.derniere_seance == Some(aujourdhui)))
}

/// Un mot encore raté d'une dictée passée de moins de 30 jours, reposé depuis deux jours :
/// le coffre de révision d'une séance. `None` s'il n'y en a pas.
fn revision(
    depot: &Depot,
    aujourdhui: NaiveDate,
    fuseau: Tz,
    alea: &mut Alea,
    demande: Demande<'_>,
    quand: DateTime<Utc>,
) -> Result<Option<(i64, Exercice)>, ErreurDictees> {
    let mut possibles = Vec::new();
    for passee in depot.dictees_depuis(aujourdhui - TimeDelta::days(JOURS_GARDES))? {
        if passee.jour > aujourdhui {
            continue;
        }
        for (position, apparitions) in historique(depot, passee.id, fuseau)? {
            if matches!(etat(&apparitions), Etat::Fragile(_))
                && poids_de(&apparitions, aujourdhui) > 0.0
            {
                possibles.push((passee.id, position));
            }
        }
    }
    let Some(&(dictee, position)) = alea.choisir(&possibles) else {
        return Ok(None);
    };
    let (phrases, numeros) = phrases_enregistrees(depot, dictee)?;
    let Some(&numero) = numeros.get(position.phrase) else {
        return Ok(None);
    };
    let mut ex = exercice_sonore(&phrases, position, numero)?;
    ex.donnees.insert("dictee".to_owned(), json!(dictee));
    let coffre = depot.ajouter(&ex, demande, quand)?;
    Ok(Some((coffre, ex)))
}

/// Ce que la séance du jour a donné.
#[derive(Debug, Clone, PartialEq)]
pub struct SeanceDuJour {
    pub dictee: DicteeStockee,
    /// Numéro de coffre et exercice, dans l'ordre de la file.
    pub coffres: Vec<(i64, Exercice)>,
    /// Demain, c'est la dictée.
    pub veille: bool,
    pub bilan: Bilan,
}

impl SeanceDuJour {
    /// Le message aux parents. Jamais le pseudo de l'enfant.
    pub fn annonce(&self) -> String {
        let jour = ecrire_jour(self.dictee.jour);
        let mut lignes = if self.coffres.is_empty() {
            vec![format!(
                "Pas de séance aujourd'hui : tous les mots de la dictée n°{} ({jour}) sont \
                 acquis, ou reposent après une erreur.",
                self.dictee.id
            )]
        } else {
            vec![format!(
                "Séance de dictée du jour : {} coffre(s), dictée n°{} pour le {jour}.",
                self.coffres.len(),
                self.dictee.id
            )]
        };
        if self.veille {
            lignes.push(match self.bilan.fragiles_en_clair() {
                Some(fragiles) => {
                    format!("La dictée est demain. Mots encore fragiles : {fragiles}.")
                }
                None => "La dictée est demain. Aucun mot fragile relevé.".to_owned(),
            });
        }
        lignes.join("\n")
    }
}

/// Crée la séance d'aujourd'hui s'il y a lieu : une dictée en préparation, et pas déjà de
/// séance ce jour. Rend `None` quand il n'y a rien à faire.
#[allow(clippy::too_many_arguments)]
pub fn creer_seance_du_jour(
    depot: &Depot,
    quand: DateTime<Utc>,
    fuseau: Tz,
    coffres: usize,
    alea: &mut Alea,
    demande: Demande<'_>,
) -> Result<Option<SeanceDuJour>, ErreurDictees> {
    let aujourdhui = jour_local(quand, fuseau);
    if deja_faite(depot, aujourdhui)? {
        return Ok(None);
    }
    let Some(dictee) = dictee_du_jour(depot, aujourdhui, fuseau)? else {
        return Ok(None);
    };
    // Une place pour la révision, si un vieux mot raté attend et que la séance a plus d'un
    // coffre.
    let revision = if coffres > 1 {
        revision(depot, aujourdhui, fuseau, alea, demande, quand)?
    } else {
        None
    };
    let places = coffres - usize::from(revision.is_some());
    let poids = poids(depot, dictee.id, aujourdhui, fuseau)?;
    let mut tires = seance(depot, &dictee, places, alea, &poids, demande, quand)?;
    tires.extend(revision);
    depot.marquer_seance(dictee.id, aujourdhui)?;
    Ok(Some(SeanceDuJour {
        veille: dictee.jour == aujourdhui + TimeDelta::days(1),
        bilan: bilan(depot, dictee.id, fuseau)?,
        dictee,
        coffres: tires,
    }))
}
