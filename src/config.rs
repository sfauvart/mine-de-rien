//! Lecture de config.toml, avec substitution des ${VARIABLES} d'environnement.

use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::util::repr;

static VARIABLE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\$\{([A-Z0-9_]+)\}").unwrap());
static OBJET: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?:minecraft:)?[a-z0-9_]+$").unwrap());
static NOM_ASSISTANT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-zÀ-ÖØ-öø-ÿ]{2,16}$").unwrap());

/// Configuration illisible ou refusée ; le message s'adresse à la personne qui installe.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ErreurConfig(pub String);

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Enfant {
    pub pseudo_minecraft: String,
    #[serde(default = "alias_par_defaut")]
    pub alias_llm: String,
    #[serde(default = "classe_par_defaut")]
    pub classe: String,
}

fn alias_par_defaut() -> String {
    "l'élève".to_owned()
}

fn classe_par_defaut() -> String {
    "CM2".to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
pub enum AdaptateurJeu {
    #[default]
    #[serde(rename = "java-rcon")]
    JavaRcon,
}

impl AdaptateurJeu {
    pub fn nom(self) -> &'static str {
        match self {
            Self::JavaRcon => "java-rcon",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Jeu {
    pub adaptateur: AdaptateurJeu,
    pub hote: String,
    pub port: u16,
    pub mot_de_passe: String,
    pub log: String,
}

impl Default for Jeu {
    fn default() -> Self {
        Self {
            adaptateur: AdaptateurJeu::JavaRcon,
            hote: "localhost".to_owned(),
            port: 25575,
            mot_de_passe: String::new(),
            log: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
pub enum AdaptateurCanal {
    #[default]
    #[serde(rename = "telegram")]
    Telegram,
}

impl AdaptateurCanal {
    pub fn nom(self) -> &'static str {
        match self {
            Self::Telegram => "telegram",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Canal {
    pub adaptateur: AdaptateurCanal,
    pub jeton: String,
    pub parents: Vec<i64>,
}

/// `adaptateur` reste un texte : une valeur inconnue ne bloque pas le lancement, elle
/// désactive le LLM (`adapters::llm::creer` la refuse, et l'agent tourne sans lui).
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Llm {
    pub adaptateur: String,
    pub modele: String,
    pub cle: String,
    pub url: String,
}

impl Default for Llm {
    fn default() -> Self {
        Self {
            adaptateur: "anthropic".to_owned(),
            modele: String::new(),
            cle: String::new(),
            url: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Stockage {
    pub sqlite: String,
}

impl Default for Stockage {
    fn default() -> Self {
        Self {
            sqlite: "mine-de-rien.sqlite".to_owned(),
        }
    }
}

/// Facultatif : où `mine-de-rien atelier` construit. Ensuite, le monde fait foi.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Atelier {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// Récompense identique pour tous les coffres, et niveaux offerts pour les essais.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Coffre {
    pub recompense: String,
    pub recompense_nombre: i64,
    pub niveaux_offerts: i64,
}

impl Default for Coffre {
    fn default() -> Self {
        Self {
            recompense: "minecraft:diamond".to_owned(),
            recompense_nombre: 3,
            niveaux_offerts: 3,
        }
    }
}

impl Coffre {
    fn verifier(&self) -> Result<(), ErreurConfig> {
        if !OBJET.is_match(&self.recompense) {
            return Err(ErreurConfig(format!(
                "[coffre] recompense invalide : {}",
                repr(&self.recompense)
            )));
        }
        if !(1..=64).contains(&self.recompense_nombre) {
            return Err(ErreurConfig(
                "[coffre] recompense_nombre doit être entre 1 et 64".into(),
            ));
        }
        if !(0..=30).contains(&self.niveaux_offerts) {
            return Err(ErreurConfig(
                "[coffre] niveaux_offerts doit être entre 0 et 30".into(),
            ));
        }
        Ok(())
    }
}

/// Pixel, que l'enfant appelle dans le chat par son nom.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Assistant {
    pub nom: String,
    pub actif: bool,
}

impl Default for Assistant {
    fn default() -> Self {
        Self {
            nom: "Pixel".to_owned(),
            actif: true,
        }
    }
}

impl Assistant {
    fn verifier(&self) -> Result<(), ErreurConfig> {
        if !NOM_ASSISTANT.is_match(&self.nom) {
            return Err(ErreurConfig(format!(
                "[assistant] nom : 2 à 16 lettres, sans espace ({})",
                repr(&self.nom)
            )));
        }
        Ok(())
    }
}

/// Serveur en paisible tant qu'il reste un coffre à faire.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Paisible {
    pub actif: bool,
    pub difficulte_normale: String,
}

impl Default for Paisible {
    fn default() -> Self {
        Self {
            actif: true,
            difficulte_normale: "easy".to_owned(),
        }
    }
}

impl Paisible {
    fn verifier(&self) -> Result<(), ErreurConfig> {
        if !["easy", "normal", "hard"].contains(&self.difficulte_normale.as_str()) {
            return Err(ErreurConfig(format!(
                "[paisible] difficulte_normale doit valoir easy, normal ou hard ({})",
                repr(&self.difficulte_normale)
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub enfant: Enfant,
    #[serde(default)]
    pub jeu: Jeu,
    #[serde(default)]
    pub canal: Canal,
    #[serde(default)]
    pub llm: Llm,
    #[serde(default)]
    pub stockage: Stockage,
    #[serde(default)]
    pub atelier: Option<Atelier>,
    #[serde(default)]
    pub coffre: Coffre,
    #[serde(default)]
    pub assistant: Assistant,
    #[serde(default)]
    pub paisible: Paisible,
}

fn substituer(valeur: &mut toml::Value, variable: &dyn Fn(&str) -> Option<String>) {
    match valeur {
        toml::Value::String(texte) => {
            if texte.contains("${") {
                *texte = VARIABLE
                    .replace_all(texte, |m: &regex::Captures| {
                        variable(&m[1]).unwrap_or_default()
                    })
                    .into_owned();
            }
        }
        toml::Value::Table(table) => {
            for (_, v) in table.iter_mut() {
                substituer(v, variable);
            }
        }
        toml::Value::Array(liste) => {
            for v in liste.iter_mut() {
                substituer(v, variable);
            }
        }
        _ => {}
    }
}

/// Lit un texte TOML. `variable` donne la valeur d'une variable d'environnement ; une
/// variable absente vaut la chaîne vide.
pub fn lire_config(
    texte: &str,
    variable: &dyn Fn(&str) -> Option<String>,
) -> Result<Config, ErreurConfig> {
    let table: toml::Table = texte
        .parse()
        .map_err(|e| ErreurConfig(format!("config illisible : {e}")))?;
    let mut brut = toml::Value::Table(table);
    substituer(&mut brut, variable);
    let config: Config = brut
        .try_into()
        .map_err(|e| ErreurConfig(format!("config refusée : {e}")))?;
    config.coffre.verifier()?;
    config.assistant.verifier()?;
    config.paisible.verifier()?;
    Ok(config)
}

pub fn charger_config(chemin: &Path) -> Result<Config, ErreurConfig> {
    let texte = std::fs::read_to_string(chemin)
        .map_err(|e| ErreurConfig(format!("{} illisible : {e}", chemin.display())))?;
    lire_config(&texte, &|nom| std::env::var(nom).ok())
}
