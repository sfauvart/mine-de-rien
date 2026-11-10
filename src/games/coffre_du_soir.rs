//! Coffre du soir. Ce fichier ne porte encore que la mise en forme des durées, dont le
//! rapport a besoin : le jeu lui-même arrive avec la phase 7 du passage à Rust.

use chrono::TimeDelta;

use crate::adapters::game::rcon::ErreurRcon;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErreurCoffre {
    /// Le serveur a répondu autre chose que prévu.
    #[error("{0}")]
    Coffre(String),
    /// Ce monde n'a pas d'atelier ; le message va aux parents.
    #[error("{0}")]
    AtelierAbsent(String),
    /// Le serveur n'a pas répondu.
    #[error("{0}")]
    Rcon(#[from] ErreurRcon),
}

/// « 12 min », « 3 h 05 », « 2 jours » : une durée telle qu'on la dit aux parents.
pub fn formater_duree(duree: Option<TimeDelta>) -> String {
    let Some(duree) = duree else {
        return "?".to_owned();
    };
    let minutes = duree.num_seconds().div_euclid(60);
    if minutes < 1 {
        return "moins d'une minute".to_owned();
    }
    if minutes < 60 {
        return format!("{minutes} min");
    }
    let (heures, minutes) = (minutes / 60, minutes % 60);
    if heures < 48 {
        return format!("{heures} h {minutes:02}");
    }
    format!("{} jours", heures / 24)
}
