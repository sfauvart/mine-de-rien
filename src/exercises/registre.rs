//! Noms de compétences que les parents peuvent taper : `/coffre division 3`.
//!
//! Chaque nom désigne un générateur et le niveau utilisé dans le coffre. Seuls les niveaux dont
//! la réponse a une écriture unique tapable sur une enclume figurent ici (la division avec
//! reste, niveau 2, en est exclue). `anglais` tire dans tout le lexique de vocabulaire.

use crate::exercises::anglais::vocabulaire;
use crate::exercises::base::{Generateur, normaliser};
use crate::exercises::maths::division;

#[derive(Debug, Clone, Copy)]
pub struct Entree {
    pub nom: &'static str,
    pub competence: &'static str,
    pub niveau: u32,
    pub generer: Generateur,
}

pub const ENTREES: [Entree; 2] = [
    Entree {
        nom: "division",
        competence: division::COMPETENCE,
        niveau: 1,
        generer: division::generer,
    },
    Entree {
        nom: "anglais",
        competence: vocabulaire::COMPETENCE,
        niveau: 1,
        generer: vocabulaire::generer,
    },
];

pub fn chercher(nom: &str) -> Option<&'static Entree> {
    let cle = normaliser(nom);
    ENTREES.iter().find(|e| normaliser(e.nom) == cle)
}

pub fn noms() -> Vec<&'static str> {
    ENTREES.iter().map(|e| e.nom).collect()
}
