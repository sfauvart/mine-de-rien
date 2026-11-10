//! Rapport de la semaine pour les parents, demandé avec `/rapport`.
//!
//! Sur les 7 derniers jours, par compétence : coffres posés, ouverts, durée médiane entre la
//! pose et l'ouverture, et indices demandés à Pixel. Puis une ligne « à retravailler » par
//! exercice resté fermé plus de 48 heures, ouvert en plus de 30 minutes, ou ouvert avec 3
//! indices ou plus. Les coffres annulés ne comptent pas.
//!
//! Le rapport ne contient jamais le pseudo ni le prénom de l'enfant.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, TimeDelta, Utc};

use crate::games::coffre_du_soir::formater_duree;
use crate::games::pixel::EVENEMENT_INDICE;
use crate::storage::sqlite::{Depot, ErreurStockage, ExerciceStocke, Statut};

pub const PERIODE: TimeDelta = TimeDelta::days(7);
pub const FERME_TROP_LONGTEMPS: TimeDelta = TimeDelta::hours(48);
pub const OUVERT_LENTEMENT: TimeDelta = TimeDelta::minutes(30);
pub const INDICES_A_RETRAVAILLER: usize = 3;

pub fn nombre_indices(n: usize) -> String {
    match n {
        0 => "sans indice".to_owned(),
        1 => "1 indice".to_owned(),
        _ => format!("{n} indices"),
    }
}

/// La raison pour laquelle cet exercice est à retravailler, ou `None`.
pub fn a_retravailler(s: &ExerciceStocke, quand: DateTime<Utc>, indices: usize) -> Option<String> {
    if s.statut == Statut::Pose
        && let Some(pose_le) = s.pose_le
        && quand - pose_le > FERME_TROP_LONGTEMPS
    {
        return Some(format!(
            "toujours fermé après {}",
            formater_duree(Some(quand - pose_le))
        ));
    }
    if s.statut != Statut::Ouvert {
        return None;
    }
    let duree = s.duree();
    let lent = duree.is_some_and(|d| d > OUVERT_LENTEMENT);
    let aide = indices >= INDICES_A_RETRAVAILLER;
    match (lent, aide) {
        (true, true) => Some(format!(
            "ouvert en {} avec {}",
            formater_duree(duree),
            nombre_indices(indices)
        )),
        (true, false) => Some(format!("ouvert en {}", formater_duree(duree))),
        (false, true) => Some(format!("ouvert avec {}", nombre_indices(indices))),
        (false, false) => None,
    }
}

/// Médiane de durées, à la milliseconde : la moyenne des deux du milieu s'il y en a un
/// nombre pair.
fn mediane(mut durees: Vec<TimeDelta>) -> Option<TimeDelta> {
    durees.sort();
    let n = durees.len();
    if n == 0 {
        return None;
    }
    if n % 2 == 1 {
        return Some(durees[n / 2]);
    }
    let somme = durees[n / 2 - 1].num_milliseconds() + durees[n / 2].num_milliseconds();
    Some(TimeDelta::milliseconds(somme / 2))
}

pub fn rapport(depot: &Depot, quand: DateTime<Utc>) -> Result<String, ErreurStockage> {
    let debut = quand - PERIODE;
    let poses: Vec<ExerciceStocke> = depot
        .poses_entre(debut, quand)?
        .into_iter()
        .filter(|s| s.statut != Statut::Annule)
        .collect();
    let disparus = depot
        .evenements_entre(debut, quand)?
        .iter()
        .filter(|e| e.genre == "coffre_disparu")
        .count();
    let en_file = depot.en_file()?.len();
    let mut indices: HashMap<i64, usize> = HashMap::new();
    for s in &poses {
        indices.insert(s.id, depot.compter(EVENEMENT_INDICE, s.id)?);
    }

    let mut lignes = vec![format!(
        "Rapport des 7 derniers jours (du {} au {})",
        debut.format("%d/%m"),
        quand.format("%d/%m")
    )];
    if poses.is_empty() {
        lignes.push("Aucun coffre posé.".to_owned());
    }
    let mut par_competence: BTreeMap<&str, Vec<&ExerciceStocke>> = BTreeMap::new();
    for s in &poses {
        par_competence
            .entry(&s.exercice.competence)
            .or_default()
            .push(s);
    }
    for (competence, liste) in &par_competence {
        let durees: Vec<TimeDelta> = liste
            .iter()
            .filter(|s| s.statut == Statut::Ouvert)
            .filter_map(|s| s.duree())
            .collect();
        let mut ligne = format!(
            "• {competence} : {} posé(s), {} ouvert(s)",
            liste.len(),
            durees.len()
        );
        if let Some(mediane) = mediane(durees) {
            ligne += &format!(", durée médiane {}", formater_duree(Some(mediane)));
        }
        let total: usize = liste.iter().map(|s| indices[&s.id]).sum();
        if total > 0 {
            ligne += &format!(", {}", nombre_indices(total));
        }
        lignes.push(ligne);
    }

    let retravailler: Vec<(&ExerciceStocke, String)> = poses
        .iter()
        .filter_map(|s| Some((s, a_retravailler(s, quand, indices[&s.id])?)))
        .collect();
    if !retravailler.is_empty() {
        lignes.push(String::new());
        lignes.push("À retravailler :".to_owned());
        for (s, raison) in retravailler {
            lignes.push(format!("• n°{} {} : {raison}", s.id, s.exercice.enonce));
        }
    }

    if disparus > 0 {
        lignes.push(String::new());
        lignes.push(format!("Coffres disparus puis reposés : {disparus}"));
    }
    if en_file > 0 {
        lignes.push(format!("En attente dans la file : {en_file}"));
    }
    Ok(lignes.join("\n"))
}
