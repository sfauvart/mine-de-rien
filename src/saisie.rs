//! Ce que tape un parent après `/coffre` devient une liste d'exercices, ou un message d'erreur.
//!
//! Deux formes :
//! - une expression recopiée du cahier (`348 ÷ 12`), recalculée par Mine de rien ;
//! - un nom de compétence suivi d'un nombre d'exercices (`division 3`).

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;

use crate::alea::Alea;
use crate::exercises::base::Exercice;
use crate::exercises::expression::calculer;
use crate::exercises::registre;
use crate::minecraft::texte::lignes_panneau;
use crate::util::{longueur, mots, strip};

pub const NOMBRE_MAX: usize = 10;
const ESSAIS_PAR_EXERCICE: usize = 50;

/// Saisie refusée ; le message s'adresse au parent, tel quel.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurSaisie(pub String);

/// D'où vient un exercice mis en file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Expression,
    Competence,
    Photo,
}

impl Source {
    pub fn nom(self) -> &'static str {
        match self {
            Self::Expression => "expression",
            Self::Competence => "competence",
            Self::Photo => "photo",
        }
    }

    pub fn lire(nom: &str) -> Option<Self> {
        [Self::Expression, Self::Competence, Self::Photo]
            .into_iter()
            .find(|s| s.nom() == nom)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Saisie {
    pub exercices: Vec<Exercice>,
    pub source: Source,
}

// Clé en mots : minuscules (accents compris), espaces et tirets simples, 50 caractères au plus
// comme le champ de l'enclume. Jamais de majuscule : le verrou compare lettre à lettre.
static CLE_MOTS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-zà-öø-ÿœæ]+(?:[ -][a-zà-öø-ÿœæ]+)*$").unwrap());
// Clé en chiffres : exactement ce qu'écrit `ecrire_reponse`. Entier sans zéro en tête, ou
// décimal à virgule, un ou deux chiffres, sans zéro final.
static CLE_NOMBRE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:0|[1-9][0-9]*)(?:,[0-9]?[1-9])?$").unwrap());
pub const LONGUEUR_CLE: usize = 50;

/// Une clé de coffre a une écriture unique : nombre écrit par `ecrire_reponse`, ou mots.
pub fn cle_valide(reponse: &str) -> bool {
    longueur(reponse) <= LONGUEUR_CLE
        && (CLE_MOTS.is_match(reponse) || CLE_NOMBRE.is_match(reponse))
}

// Clé de dictée : un mot du texte, tel quel. Majuscules permises, apostrophe droite et trait
// d'union entre deux lettres, pas d'espace.
static CLE_DICTEE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-zÀ-ÖØ-öø-ÿŒœÆæ]+(?:['-][A-Za-zÀ-ÖØ-öø-ÿŒœÆæ]+)*$").unwrap()
});

/// Une clé de coffre de dictée : le mot exactement comme dans le texte de la maîtresse.
pub fn cle_dictee_valide(reponse: &str) -> bool {
    longueur(reponse) <= LONGUEUR_CLE && CLE_DICTEE.is_match(reponse)
}

/// Garde-fou : la réponse d'un générateur doit avoir l'écriture unique du coffre.
fn verifier_cle(ex: &Exercice) -> Result<(), ErreurSaisie> {
    if !cle_valide(&ex.reponse) {
        return Err(ErreurSaisie(format!(
            "réponse de générateur inutilisable dans un coffre : {ex:?}"
        )));
    }
    if lignes_panneau(&ex.enonce).is_none() {
        return Err(ErreurSaisie(format!(
            "énoncé de générateur trop long pour un panneau : {ex:?}"
        )));
    }
    Ok(())
}

/// Le nom d'une compétence et, s'il est donné, le nombre d'exercices demandé.
fn nom_et_nombre(texte: &str) -> Option<(&str, Option<&str>)> {
    let mut parties = mots(texte);
    let nom = parties.next()?;
    let nombre = parties.next();
    if parties.next().is_some() {
        return None;
    }
    match nombre {
        Some(n) if !n.chars().all(|c| c.is_ascii_digit()) => None,
        autre => Some((nom, autre)),
    }
}

fn competence(texte: &str, alea: &mut Alea) -> Result<Saisie, ErreurSaisie> {
    let lu = nom_et_nombre(texte);
    let Some((entree, nombre)) =
        lu.and_then(|(nom, nombre)| Some((registre::chercher(nom)?, nombre)))
    else {
        return Err(ErreurSaisie(format!(
            "Je ne reconnais ni un calcul ni une compétence. \
             Compétences disponibles : {}. \
             Exemples : « 348 ÷ 12 », « division 3 » ou « anglais 3 ».",
            registre::noms().join(", ")
        )));
    };
    // Un nombre trop grand pour être lu est, à plus forte raison, hors des bornes.
    let nombre = nombre.map_or(Some(1), |n| n.parse::<usize>().ok());
    let Some(nombre) = nombre.filter(|n| (1..=NOMBRE_MAX).contains(n)) else {
        return Err(ErreurSaisie(format!(
            "Entre 1 et {NOMBRE_MAX} exercices à la fois."
        )));
    };
    let mut exercices = Vec::new();
    let mut vus = HashSet::new();
    for _ in 0..nombre * ESSAIS_PAR_EXERCICE {
        if exercices.len() == nombre {
            break;
        }
        let ex = (entree.generer)(alea, entree.niveau);
        if vus.contains(&ex.enonce) {
            continue;
        }
        verifier_cle(&ex)?;
        vus.insert(ex.enonce.clone());
        exercices.push(ex);
    }
    Ok(Saisie {
        exercices,
        source: Source::Competence,
    })
}

pub fn interpreter(texte: &str, alea: &mut Alea) -> Result<Saisie, ErreurSaisie> {
    let texte = strip(texte);
    let Some(premier) = texte.chars().next() else {
        return Err(ErreurSaisie(
            "Que faut-il mettre dans le coffre ? Exemple : « /coffre 348 ÷ 12 ».".to_owned(),
        ));
    };
    // Une saisie qui commence par une lettre est un nom de compétence, sinon un calcul.
    if premier.is_alphabetic() {
        return competence(texte, alea);
    }
    match calculer(texte) {
        Ok(ex) => Ok(Saisie {
            exercices: vec![ex],
            source: Source::Expression,
        }),
        Err(e) => Err(ErreurSaisie(e.to_string())),
    }
}
