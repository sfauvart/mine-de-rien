//! Composants texte Minecraft construits à partir de texte venu de l'extérieur.
//!
//! Les énoncés sont écrits par les parents : ils peuvent contenir des guillemets, des
//! antislashs, des accolades ou des crochets. Le texte n'est donc jamais concaténé dans une
//! commande. On construit un objet que l'on sérialise.
//!
//! Depuis la 1.21.5, les composants texte des commandes s'écrivent en SNBT. Le JSON écrit
//! ici en est un sous-ensemble valide : clés entre guillemets doubles, chaînes entre
//! guillemets doubles où seuls `"` et `\` sont échappés (les caractères de contrôle, que JSON
//! échapperait en `\uXXXX`, sont retirés avant). Les accents restent tels quels, RCON
//! transporte de l'UTF-8.

use std::sync::LazyLock;

use regex::Regex;
use serde_json::{Map, Value};

use crate::util::{debut, json_python, longueur, mots, repr};

pub const LONGUEUR_MAX: usize = 256;

// Pseudo Minecraft : ce qui est accepté par la liste blanche. Interdit `@a`, les espaces, etc.
static PSEUDO: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z0-9_]{1,16}$").unwrap());

/// Le pseudo ne peut pas être placé tel quel dans une commande.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("pseudo Minecraft invalide : {0}")]
pub struct PseudoInvalide(String);

/// Valeur d'une clé de style d'un composant : `color` prend un texte, `bold` un booléen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style<'a> {
    Texte(&'a str),
    Bool(bool),
}

impl From<&Style<'_>> for Value {
    fn from(style: &Style<'_>) -> Self {
        match style {
            Style::Texte(texte) => Value::String((*texte).to_owned()),
            Style::Bool(valeur) => Value::Bool(*valeur),
        }
    }
}

/// Retire les caractères de contrôle et le `§` (codes de mise en forme hérités), puis borne.
///
/// Un saut de ligne devient une espace : le chat et les panneaux ne s'en servent pas.
pub fn nettoyer(texte: &str, longueur_max: usize) -> String {
    let propre: String = texte
        .chars()
        .filter(|&c| c != '§' && (matches!(c, '\n' | '\r' | '\t') || !c.is_control()))
        .map(|c| {
            if matches!(c, '\n' | '\r' | '\t') {
                ' '
            } else {
                c
            }
        })
        .collect();
    debut(&propre, longueur_max).to_owned()
}

/// Composant texte sérialisé, prêt à être placé dans une commande.
///
/// `style` accepte les clés de style des composants (`("color", Style::Texte("gold"))`,
/// `("bold", Style::Bool(true))`, ...).
pub fn composant(texte: &str, longueur_max: usize, style: &[(&str, Style<'_>)]) -> String {
    let mut objet = Map::new();
    objet.insert(
        "text".to_owned(),
        Value::String(nettoyer(texte, longueur_max)),
    );
    for (cle, valeur) in style {
        objet.insert((*cle).to_owned(), valeur.into());
    }
    json_python(&Value::Object(objet))
}

/// Renvoie le pseudo s'il est sûr à placer dans une commande.
pub fn verifier_pseudo(joueur: &str) -> Result<&str, PseudoInvalide> {
    if PSEUDO.is_match(joueur) {
        Ok(joueur)
    } else {
        Err(PseudoInvalide(repr(joueur)))
    }
}

/// Commande `tellraw` (sans `/`) qui affiche `texte` à `joueur`.
pub fn tellraw(
    joueur: &str,
    texte: &str,
    style: &[(&str, Style<'_>)],
) -> Result<String, PseudoInvalide> {
    Ok(format!(
        "tellraw {} {}",
        verifier_pseudo(joueur)?,
        composant(texte, LONGUEUR_MAX, style)
    ))
}

pub const LIGNES_PANNEAU: usize = 4;
pub const LARGEUR_PANNEAU: usize = 15;

/// Découpe `texte` en 4 lignes de 15 caractères au plus, coupées sur les espaces.
///
/// Renvoie `None` si le texte ne tient pas : on refuse plutôt que de tronquer un énoncé.
/// Le résultat a toujours exactement 4 lignes (complétées par des chaînes vides).
pub fn lignes_panneau(texte: &str) -> Option<[String; LIGNES_PANNEAU]> {
    let mut lignes: Vec<String> = Vec::new();
    let mut courante = String::new();
    let propre = nettoyer(texte, LONGUEUR_MAX);
    for mot in mots(&propre) {
        if longueur(mot) > LARGEUR_PANNEAU {
            return None;
        }
        if courante.is_empty() {
            courante.push_str(mot);
        } else if longueur(&courante) + 1 + longueur(mot) <= LARGEUR_PANNEAU {
            courante.push(' ');
            courante.push_str(mot);
        } else {
            lignes.push(std::mem::replace(&mut courante, mot.to_owned()));
        }
    }
    if !courante.is_empty() {
        lignes.push(courante);
    }
    if lignes.len() > LIGNES_PANNEAU {
        return None;
    }
    lignes.resize(LIGNES_PANNEAU, String::new());
    lignes.try_into().ok()
}
