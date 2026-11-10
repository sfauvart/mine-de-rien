//! Vocabulaire anglais, attendu de fin de CM2 : comprendre et restituer les mots familiers
//! (jours, mois, nombres jusqu'à 20, couleurs, salutations), dans les deux sens.
//!
//! Les paires viennent de `donnees/lexique.toml`, relu par un parent ; aucune n'est produite par
//! un LLM. Chaque exercice tire une paire et un sens :
//!
//! - « mercredi en anglais ? » → `wednesday` ;
//! - « Wednesday en français ? » → `mercredi`.
//!
//! **Écriture unique.** Le verrou compare lettre à lettre : la clé est toujours en minuscules,
//! avec l'orthographe du lexique (accents, tirets, espaces). Le mode d'emploi de la clé le dit.
//! Un sens n'est proposé que si sa réponse est unique dans tout le lexique (« bonjour » →
//! `hello` ou `good morning` : jamais demandé vers l'anglais), et jamais quand les deux mots
//! sont identiques (« six »). Pour les nombres, l'énoncé précise « en lettres ».
//!
//! **Accents.** `ACCENTS_SUR_ENCLUME` dit si l'enfant sait taper un accent sur l'enclume. S'il
//! vaut `false`, une réponse française accentuée n'est jamais demandée ; le mot reste demandé
//! dans l'autre sens.

use std::collections::HashMap;
use std::sync::LazyLock;

use serde_json::json;
use unicode_normalization::UnicodeNormalization;

use crate::alea::Alea;
use crate::exercises::base::{Exercice, normaliser};
use crate::util::repr;

pub const COMPETENCE: &str = "anglais/vocabulaire";

/// Le lexique, tel qu'il était à la compilation.
const LEXIQUE: &str = include_str!("donnees/lexique.toml");

/// Vérifié en jeu le 2026-11-08 : « août » se tape sur l'enclume, et l'accent est exigé.
pub const ACCENTS_SUR_ENCLUME: bool = true;

const AVEC_MAJUSCULE: [&str; 2] = ["jours", "mois"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Sens {
    VersAnglais,
    VersFrancais,
}

impl Sens {
    pub fn nom(self) -> &'static str {
        match self {
            Self::VersAnglais => "vers-anglais",
            Self::VersFrancais => "vers-francais",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paire {
    pub anglais: String,
    pub francais: String,
    pub famille: String,
    pub sens: Vec<Sens>,
}

fn accentue(texte: &str) -> bool {
    texte.nfkd().ne(texte.chars())
}

impl Paire {
    pub fn accent(&self) -> bool {
        accentue(&self.francais)
    }
}

/// Une ligne du lexique : famille, anglais, français, et si le sens vers l'anglais est permis.
pub type Entree = (String, String, String, bool);

/// Lexique mal écrit, ou sens demandé alors qu'il n'est pas autorisé pour la paire.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct ErreurVocabulaire(pub String);

fn brut() -> Result<Vec<Entree>, ErreurVocabulaire> {
    let table: toml::Table = LEXIQUE
        .parse()
        .map_err(|e| ErreurVocabulaire(format!("lexique illisible : {e}")))?;
    let mut entrees = Vec::new();
    for (famille, mots) in &table {
        let Some(mots) = mots.as_table() else {
            continue;
        };
        for (anglais, valeur) in mots {
            let (francais, vers_anglais) = match valeur {
                toml::Value::String(francais) => (francais.clone(), true),
                toml::Value::Table(detail) => (
                    detail
                        .get("francais")
                        .and_then(toml::Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    detail
                        .get("vers_anglais")
                        .and_then(toml::Value::as_bool)
                        .unwrap_or(true),
                ),
                _ => continue,
            };
            entrees.push((famille.clone(), anglais.clone(), francais, vers_anglais));
        }
    }
    Ok(entrees)
}

/// Calcule les sens autorisés de chaque paire. Une paire sans sens autorisé est gardée,
/// pour être reconnue à la lecture d'une photo, mais n'est jamais demandée.
pub fn construire(entrees: &[Entree], accents: bool) -> Result<Vec<Paire>, ErreurVocabulaire> {
    let mut par_anglais: HashMap<String, usize> = HashMap::new();
    let mut par_francais: HashMap<String, usize> = HashMap::new();
    for (_, anglais, francais, _) in entrees {
        *par_anglais.entry(normaliser(anglais)).or_default() += 1;
        *par_francais.entry(normaliser(francais)).or_default() += 1;
    }
    let mut paires = Vec::with_capacity(entrees.len());
    for (famille, anglais, francais, vers_anglais) in entrees {
        if *anglais != anglais.to_lowercase() || *francais != francais.to_lowercase() {
            return Err(ErreurVocabulaire(format!(
                "lexique : majuscule dans {} = {}",
                repr(anglais),
                repr(francais)
            )));
        }
        let mut sens = Vec::new();
        if normaliser(anglais) != normaliser(francais) {
            if *vers_anglais && par_francais[&normaliser(francais)] == 1 {
                sens.push(Sens::VersAnglais);
            }
            if par_anglais[&normaliser(anglais)] == 1 && (accents || !accentue(francais)) {
                sens.push(Sens::VersFrancais);
            }
        }
        paires.push(Paire {
            anglais: anglais.clone(),
            francais: francais.clone(),
            famille: famille.clone(),
            sens,
        });
    }
    Ok(paires)
}

static PAIRES: LazyLock<Vec<Paire>> = LazyLock::new(|| {
    // Le lexique est une constante du binaire : les tests le chargent, il ne peut pas être
    // faux à l'exécution sans l'avoir été à la compilation.
    brut()
        .and_then(|entrees| construire(&entrees, ACCENTS_SUR_ENCLUME))
        .expect("lexique embarqué")
});

pub fn lexique() -> &'static [Paire] {
    &PAIRES
}

/// « Wednesday » : `str.capitalize()` de Python, pour les jours et les mois.
fn affiche_anglais(p: &Paire) -> String {
    if !AVEC_MAJUSCULE.contains(&p.famille.as_str()) {
        return p.anglais.clone();
    }
    let mut lettres = p.anglais.chars();
    match lettres.next() {
        Some(premiere) => premiere
            .to_uppercase()
            .chain(lettres.flat_map(char::to_lowercase))
            .collect(),
        None => String::new(),
    }
}

pub fn exercice(p: &Paire, sens: Sens) -> Result<Exercice, ErreurVocabulaire> {
    if !p.sens.contains(&sens) {
        return Err(ErreurVocabulaire(format!(
            "sens {} non autorisé pour {}",
            sens.nom(),
            repr(&p.anglais)
        )));
    }
    let precision = if p.famille == "nombres" {
        ", en lettres"
    } else {
        ""
    };
    let (enonce, reponse) = match sens {
        Sens::VersAnglais => (
            format!("{} en anglais{precision} ?", p.francais),
            p.anglais.clone(),
        ),
        Sens::VersFrancais => (
            format!("{} en français{precision} ?", affiche_anglais(p)),
            p.francais.clone(),
        ),
    };
    let donnees = json!({
        "anglais": p.anglais, "francais": p.francais, "famille": p.famille, "sens": sens.nom(),
    });
    Ok(Exercice {
        competence: COMPETENCE.to_owned(),
        enonce,
        reponse,
        donnees: donnees.as_object().cloned().unwrap_or_default(),
    })
}

fn tirer(p: &Paire, alea: &mut Alea) -> Option<Exercice> {
    let sens = *alea.choisir(&p.sens)?;
    exercice(p, sens).ok()
}

pub fn generer(alea: &mut Alea, _niveau: u32) -> Exercice {
    let demandables: Vec<&Paire> = lexique().iter().filter(|p| !p.sens.is_empty()).collect();
    let p = alea
        .choisir(&demandables)
        .expect("le lexique embarqué a des paires à demander");
    tirer(p, alea).expect("une paire demandable a un sens autorisé")
}

/// La paire d'un mot lu, en anglais d'abord, puis en français s'il n'est pas ambigu.
pub fn chercher(mot: &str) -> Option<&'static Paire> {
    let cle = normaliser(mot);
    if let Some(p) = lexique().iter().find(|p| normaliser(&p.anglais) == cle) {
        return Some(p);
    }
    let mut trouvees = lexique().iter().filter(|p| normaliser(&p.francais) == cle);
    match (trouvees.next(), trouvees.next()) {
        (Some(p), None) => Some(p),
        _ => None,
    }
}

/// L'exercice d'un mot lu sur une photo, dans un sens autorisé tiré au hasard ;
/// `None` si le mot est hors lexique ou n'a aucun sens autorisé.
pub fn exercice_pour(mot: &str, alea: &mut Alea) -> Option<Exercice> {
    tirer(chercher(mot)?, alea)
}
