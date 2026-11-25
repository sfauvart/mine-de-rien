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

/// La voix de synthèse qui lit les dictées. Section absente : pas de dictée, le reste de
/// l'agent marche comme avant. `adaptateur` reste un texte, comme pour le LLM.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Voix {
    pub adaptateur: String,
    pub url: String,
    /// Nom de la voix chez le fournisseur, ex. « fr_FR-siwis-medium ».
    pub voix: String,
    /// Durée des sons de la voix : 1 pour son débit naturel, davantage pour ralentir.
    pub vitesse_normale: f64,
    /// Vitesse du premier indice de Pixel : la phrase relue lentement.
    pub vitesse_lente: f64,
    /// Le dossier partagé avec le serveur Minecraft : l'agent y dépose les sons que le plugin
    /// `mine-de-rien-voix` joue à l'enfant par le chat vocal. Vide : rien n'est dit en jeu.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub dossier_du_jeu: String,
    /// Les messages que l'agent et Pixel écrivent à l'enfant dans le chat sont aussi dits à
    /// voix haute, par le chat vocal. Demande `dossier_du_jeu`.
    #[serde(skip_serializing_if = "est_vrai")]
    pub dire_le_chat: bool,
}

fn est_vrai(valeur: &bool) -> bool {
    *valeur
}

impl Default for Voix {
    fn default() -> Self {
        Self {
            adaptateur: "piper".to_owned(),
            url: String::new(),
            voix: "fr_FR-siwis-medium".to_owned(),
            // Choix du parent à l'écoute (2026-11-10) : le débit naturel de la voix va trop
            // vite pour une dictée.
            vitesse_normale: 1.4,
            vitesse_lente: 1.8,
            dossier_du_jeu: String::new(),
            dire_le_chat: true,
        }
    }
}

impl Voix {
    pub const VITESSE_MIN: f64 = 0.5;
    pub const VITESSE_MAX: f64 = 3.0;

    fn verifier(&self) -> Result<(), ErreurConfig> {
        for (nom, vitesse) in [
            ("vitesse_normale", self.vitesse_normale),
            ("vitesse_lente", self.vitesse_lente),
        ] {
            if !(Self::VITESSE_MIN..=Self::VITESSE_MAX).contains(&vitesse) {
                return Err(ErreurConfig(format!(
                    "[voix] {nom} doit être entre {} et {} : {vitesse}",
                    Self::VITESSE_MIN,
                    Self::VITESSE_MAX
                )));
            }
        }
        if self.vitesse_lente < self.vitesse_normale {
            return Err(ErreurConfig(
                "[voix] vitesse_lente doit être au moins égale à vitesse_normale".to_owned(),
            ));
        }
        Ok(())
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

/// Un cadeau possible dans un coffre : plus son poids est grand, plus il sort souvent.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Cadeau {
    pub objet: String,
    #[serde(default = "un")]
    pub nombre: i64,
    #[serde(default = "un")]
    pub poids: i64,
    /// Le nom dit aux parents sur Telegram ; sans lui, l'identifiant de l'objet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nom: Option<String>,
}

fn un() -> i64 {
    1
}

/// La liste livrée avec l'agent : de quoi construire et explorer, rien qui casse le jeu.
/// (objet, nombre, poids, nom)
const CADEAUX_PAR_DEFAUT: [(&str, i64, i64, &str); 13] = [
    ("minecraft:diamond", 3, 10, "diamant"),
    ("minecraft:emerald", 5, 10, "émeraude"),
    ("minecraft:iron_ingot", 8, 10, "lingot de fer"),
    ("minecraft:gold_ingot", 4, 8, "lingot d'or"),
    ("minecraft:golden_carrot", 8, 8, "carotte dorée"),
    ("minecraft:cake", 1, 8, "gâteau"),
    ("minecraft:amethyst_shard", 6, 6, "éclat d'améthyste"),
    ("minecraft:lead", 2, 5, "laisse"),
    ("minecraft:name_tag", 1, 5, "étiquette"),
    ("minecraft:saddle", 1, 4, "selle"),
    ("minecraft:spyglass", 1, 3, "longue-vue"),
    ("minecraft:music_disc_cat", 1, 2, "disque de musique"),
    ("minecraft:golden_apple", 1, 2, "pomme dorée"),
];

pub fn cadeaux_par_defaut() -> Vec<Cadeau> {
    CADEAUX_PAR_DEFAUT
        .iter()
        .map(|&(objet, nombre, poids, nom)| Cadeau {
            objet: objet.to_owned(),
            nombre,
            poids,
            nom: Some(nom.to_owned()),
        })
        .collect()
}

fn cadeaux_sont_par_defaut(cadeaux: &[Cadeau]) -> bool {
    cadeaux == cadeaux_par_defaut()
}

fn cadeaux_par_coffre_est_par_defaut(nombre: &i64) -> bool {
    *nombre == Coffre::CADEAUX_PAR_COFFRE
}

/// Les cadeaux des coffres, tirés au hasard dans une liste, et les niveaux offerts pour les
/// essais.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Coffre {
    pub recompense: String,
    pub recompense_nombre: i64,
    pub niveaux_offerts: i64,
    /// Aux valeurs d'usage, ni la liste ni le nombre ne s'écrivent.
    #[serde(skip_serializing_if = "cadeaux_par_coffre_est_par_defaut")]
    pub cadeaux_par_coffre: i64,
    #[serde(skip_serializing_if = "cadeaux_sont_par_defaut")]
    pub cadeaux: Vec<Cadeau>,
}

impl Default for Coffre {
    fn default() -> Self {
        Self {
            recompense: "minecraft:diamond".to_owned(),
            recompense_nombre: 3,
            niveaux_offerts: 3,
            cadeaux_par_coffre: Self::CADEAUX_PAR_COFFRE,
            cadeaux: cadeaux_par_defaut(),
        }
    }
}

impl Coffre {
    /// Cadeaux différents par coffre, sans réglage.
    pub const CADEAUX_PAR_COFFRE: i64 = 2;
    /// Les cadeaux tiennent dans les cases 12 à 16 du coffre.
    pub const CADEAUX_MAX: i64 = 5;
    const NOM_MAX: usize = 30;

    fn verifier_cadeaux(&self) -> Result<(), ErreurConfig> {
        if !(1..=Self::CADEAUX_MAX).contains(&self.cadeaux_par_coffre) {
            return Err(ErreurConfig(format!(
                "[coffre] cadeaux_par_coffre doit être entre 1 et {}",
                Self::CADEAUX_MAX
            )));
        }
        if (self.cadeaux.len() as i64) < self.cadeaux_par_coffre {
            return Err(ErreurConfig(format!(
                "[coffre] cadeaux : il en faut au moins {} (cadeaux_par_coffre), la liste en a {}",
                self.cadeaux_par_coffre,
                self.cadeaux.len()
            )));
        }
        let sans_prefixe = |objet: &str| objet.trim_start_matches("minecraft:").to_owned();
        let mut vus = std::collections::HashSet::new();
        for cadeau in &self.cadeaux {
            if !OBJET.is_match(&cadeau.objet) {
                return Err(ErreurConfig(format!(
                    "[coffre] cadeaux : objet invalide : {}",
                    repr(&cadeau.objet)
                )));
            }
            if !(1..=64).contains(&cadeau.nombre) {
                return Err(ErreurConfig(format!(
                    "[coffre] cadeaux : nombre doit être entre 1 et 64 ({})",
                    cadeau.objet
                )));
            }
            if !(1..=100).contains(&cadeau.poids) {
                return Err(ErreurConfig(format!(
                    "[coffre] cadeaux : poids doit être entre 1 et 100 ({})",
                    cadeau.objet
                )));
            }
            if let Some(nom) = &cadeau.nom
                && (nom.trim().is_empty() || nom.chars().count() > Self::NOM_MAX)
            {
                return Err(ErreurConfig(format!(
                    "[coffre] cadeaux : nom de 1 à {} caractères ({})",
                    Self::NOM_MAX,
                    cadeau.objet
                )));
            }
            if !vus.insert(sans_prefixe(&cadeau.objet)) {
                return Err(ErreurConfig(format!(
                    "[coffre] cadeaux : {} figure deux fois dans la liste",
                    cadeau.objet
                )));
            }
        }
        Ok(())
    }

    fn verifier(&self) -> Result<(), ErreurConfig> {
        self.verifier_cadeaux()?;
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

/// Les séances de dictée : chaque jour, à la première connexion de l'enfant, quelques coffres
/// tirés dans la dictée à préparer.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dictee {
    /// Coffres par séance : des séances courtes, de 3 à 5.
    pub coffres_par_seance: usize,
    /// Le fuseau de la maison : « chaque jour » se compte chez soi, pas en UTC.
    pub fuseau: String,
}

impl Default for Dictee {
    fn default() -> Self {
        Self {
            coffres_par_seance: 4,
            fuseau: "Europe/Paris".to_owned(),
        }
    }
}

impl Dictee {
    pub const COFFRES_MIN: usize = 3;
    pub const COFFRES_MAX: usize = 5;

    fn est_par_defaut(&self) -> bool {
        *self == Self::default()
    }

    /// Le fuseau de la maison, déjà vérifié à la lecture de la configuration.
    pub fn tz(&self) -> chrono_tz::Tz {
        self.fuseau.parse().unwrap_or(chrono_tz::Europe::Paris)
    }

    fn verifier(&self) -> Result<(), ErreurConfig> {
        if !(Self::COFFRES_MIN..=Self::COFFRES_MAX).contains(&self.coffres_par_seance) {
            return Err(ErreurConfig(format!(
                "[dictee] coffres_par_seance doit être entre {} et {} : {}",
                Self::COFFRES_MIN,
                Self::COFFRES_MAX,
                self.coffres_par_seance
            )));
        }
        if self.fuseau.parse::<chrono_tz::Tz>().is_err() {
            return Err(ErreurConfig(format!(
                "[dictee] fuseau inconnu : {} (exemple : \"Europe/Paris\")",
                repr(&self.fuseau)
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
    /// Absente, elle ne s'écrit pas : une configuration sans voix se relit comme en 0.1.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voix: Option<Voix>,
    /// La section du pack de sons, retirée avec lui : une configuration qui la porte encore
    /// se charge, son contenu n'est pas lu, et l'agent le dit au démarrage.
    #[serde(default, skip_serializing)]
    pub pack: Option<toml::Value>,
    /// Aux valeurs d'usage, elle ne s'écrit pas : même relecture qu'en 0.1.2.
    #[serde(default, skip_serializing_if = "Dictee::est_par_defaut")]
    pub dictee: Dictee,
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
    if let Some(voix) = &config.voix {
        voix.verifier()?;
    }
    config.dictee.verifier()?;
    Ok(config)
}

pub fn charger_config(chemin: &Path) -> Result<Config, ErreurConfig> {
    let texte = std::fs::read_to_string(chemin)
        .map_err(|e| ErreurConfig(format!("{} illisible : {e}", chemin.display())))?;
    lire_config(&texte, &|nom| std::env::var(nom).ok())
}
