use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use unicode_normalization::UnicodeNormalization;

use crate::alea::Alea;

/// Compare les réponses sans tenir compte de la casse, des accents ni des espaces.
pub fn normaliser(texte: &str) -> String {
    texte
        .nfkd()
        .filter(|c| c.is_ascii() && !matches!(c, ' ' | '\t' | '\n' | '\r' | '\u{b}' | '\u{c}'))
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// Ce que l'enfant a écrit sur la clé vaut-il la réponse ? Ni la casse ni les espaces ne
/// comptent : « Plus », « plus » et « pl us » ouvrent le même coffre. Les accents, eux,
/// comptent : « a » n'est pas « à ».
pub fn meme_cle(ecrit: &str, reponse: &str) -> bool {
    let cle = |texte: &str| -> String {
        texte
            .chars()
            .filter(|c| !c.is_whitespace())
            .flat_map(char::to_lowercase)
            .collect()
    };
    cle(ecrit) == cle(reponse)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exercice {
    /// ex. "maths/division-posee"
    pub competence: String,
    /// texte neutre, sans habillage
    pub enonce: String,
    /// la clé exacte attendue (c'est elle qui verrouille le coffre)
    pub reponse: String,
    /// matière pour l'habillage par le LLM
    #[serde(default)]
    pub donnees: Map<String, Value>,
}

impl Exercice {
    pub fn accepte(&self, proposition: &str) -> bool {
        normaliser(proposition) == normaliser(&self.reponse)
    }
}

/// Un générateur : un tirage et un niveau donnent un exercice dont la réponse est calculée.
pub type Generateur = fn(&mut Alea, u32) -> Exercice;
