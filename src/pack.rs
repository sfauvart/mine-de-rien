//! Le pack de ressources des dictées : les phrases lues, que le jeu de l'enfant télécharge.
//!
//! Un son personnalisé n'existe pour Minecraft que dans un pack de ressources. Le serveur en
//! annonce l'adresse (`resource-pack` de `server.properties`), le jeu le télécharge à la
//! connexion. L'agent construit le pack et le sert lui-même (`service_pack`).
//!
//! Contenu, et rien d'autre :
//!
//! ```text
//! pack.mcmeta
//! assets/mine_de_rien/sounds.json
//! assets/mine_de_rien/sounds/dictee/<n>.ogg
//! ```
//!
//! Le son `<n>` se joue par `playsound mine_de_rien:dictee.<n> voice <pseudo> ~ ~ ~ 1 1`
//! (vérifié sur Minecraft Java 26.3). **Les fichiers portent des numéros, jamais de mots** :
//! le pack reste sur le disque du jeu, l'enfant peut l'ouvrir, il n'y lira aucune réponse.
//!
//! L'archive est écrite à la main, sans compression (l'Ogg ne se compresse pas) et sans date :
//! les mêmes sons donnent le même fichier, à l'octet près.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

/// Format des packs de ressources de Minecraft Java 26.3 (`pack_version.resource_major` de
/// son `version.json`). À ajuster avec le 121 du datapack si `MC_VERSION` change.
pub const FORMAT: u32 = 97;
pub const ESPACE: &str = "mine_de_rien";
pub const NOM_FICHIER: &str = "mine-de-rien.zip";

#[derive(Debug, thiserror::Error)]
pub enum ErreurPack {
    #[error("{0}")]
    Refus(String),
    #[error("{0}")]
    Fichier(#[from] io::Error),
}

/// Le nom du son n°`numero`, tel que l'attend `playsound`.
pub fn evenement(numero: u32) -> String {
    format!("{ESPACE}:dictee.{numero}")
}

fn chemin_du_son(numero: u32) -> String {
    format!("assets/{ESPACE}/sounds/dictee/{numero}.ogg")
}

fn mcmeta() -> String {
    let meta = json!({
        "pack": {
            "description": "Mine de rien : les dictées lues à voix haute",
            "min_format": FORMAT,
            "max_format": FORMAT,
        }
    });
    format!("{meta:#}\n")
}

fn sounds_json<'a>(numeros: impl Iterator<Item = &'a u32>) -> String {
    let mut evenements = Map::new();
    for numero in numeros {
        // Lu au fil de l'eau plutôt que chargé en entier : une phrase dure plusieurs secondes.
        evenements.insert(
            format!("dictee.{numero}"),
            json!({"sounds": [{"name": format!("{ESPACE}:dictee/{numero}"), "stream": true}]}),
        );
    }
    format!("{:#}\n", Value::Object(evenements))
}

/// Somme de contrôle CRC-32 des archives zip.
fn crc32(octets: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for &octet in octets {
        crc ^= u32::from(octet);
        for _ in 0..8 {
            crc = if crc & 1 == 1 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

// Date fixe des entrées, au format DOS : 1er janvier 1980, minuit.
const DATE_DOS: u16 = 0x0021;
const HEURE_DOS: u16 = 0;
const VERSION_ZIP: u16 = 20;
// Bit 11 : les noms sont en UTF-8. Ils sont en ASCII ici, autant le dire juste.
const DRAPEAUX: u16 = 0x0800;

fn taille_u32(n: usize) -> Result<u32, ErreurPack> {
    u32::try_from(n).map_err(|_| ErreurPack::Refus("Pack de sons trop gros (4 Go).".to_owned()))
}

/// Archive zip sans compression.
fn zip(fichiers: &[(String, &[u8])]) -> Result<Vec<u8>, ErreurPack> {
    let nombre = u16::try_from(fichiers.len())
        .map_err(|_| ErreurPack::Refus("Trop de sons dans le pack (65 535).".to_owned()))?;
    let mut archive = Vec::new();
    let mut repertoire = Vec::new();
    for (nom, contenu) in fichiers {
        let position = taille_u32(archive.len())?;
        let taille = taille_u32(contenu.len())?;
        let longueur_nom = u16::try_from(nom.len())
            .map_err(|_| ErreurPack::Refus(format!("Nom de fichier trop long : {nom}")))?;
        let somme = crc32(contenu);
        // Les champs communs à l'en-tête local et à l'entrée du répertoire central.
        let mut commun = Vec::new();
        commun.extend_from_slice(&VERSION_ZIP.to_le_bytes());
        commun.extend_from_slice(&DRAPEAUX.to_le_bytes());
        commun.extend_from_slice(&0u16.to_le_bytes()); // méthode 0 : stocké tel quel
        commun.extend_from_slice(&HEURE_DOS.to_le_bytes());
        commun.extend_from_slice(&DATE_DOS.to_le_bytes());
        commun.extend_from_slice(&somme.to_le_bytes());
        commun.extend_from_slice(&taille.to_le_bytes()); // taille compressée
        commun.extend_from_slice(&taille.to_le_bytes()); // taille réelle
        commun.extend_from_slice(&longueur_nom.to_le_bytes());
        commun.extend_from_slice(&0u16.to_le_bytes()); // champ supplémentaire

        archive.extend_from_slice(b"PK\x03\x04");
        archive.extend_from_slice(&commun);
        archive.extend_from_slice(nom.as_bytes());
        archive.extend_from_slice(contenu);

        repertoire.extend_from_slice(b"PK\x01\x02");
        repertoire.extend_from_slice(&VERSION_ZIP.to_le_bytes()); // version de l'auteur
        repertoire.extend_from_slice(&commun);
        repertoire.extend_from_slice(&0u16.to_le_bytes()); // commentaire
        repertoire.extend_from_slice(&0u16.to_le_bytes()); // disque
        repertoire.extend_from_slice(&0u16.to_le_bytes()); // attributs internes
        repertoire.extend_from_slice(&0u32.to_le_bytes()); // attributs externes
        repertoire.extend_from_slice(&position.to_le_bytes());
        repertoire.extend_from_slice(nom.as_bytes());
    }
    let debut_repertoire = taille_u32(archive.len())?;
    let taille_repertoire = taille_u32(repertoire.len())?;
    archive.extend_from_slice(&repertoire);
    archive.extend_from_slice(b"PK\x05\x06");
    archive.extend_from_slice(&0u16.to_le_bytes()); // ce disque
    archive.extend_from_slice(&0u16.to_le_bytes()); // disque du répertoire
    archive.extend_from_slice(&nombre.to_le_bytes());
    archive.extend_from_slice(&nombre.to_le_bytes());
    archive.extend_from_slice(&taille_repertoire.to_le_bytes());
    archive.extend_from_slice(&debut_repertoire.to_le_bytes());
    archive.extend_from_slice(&0u16.to_le_bytes()); // commentaire
    Ok(archive)
}

/// Construit le pack : un son Ogg Vorbis par numéro. Sans son, le pack est valide et vide.
pub fn construire(sons: &BTreeMap<u32, Vec<u8>>) -> Result<Vec<u8>, ErreurPack> {
    for (numero, son) in sons {
        if !son.starts_with(b"OggS") {
            return Err(ErreurPack::Refus(format!(
                "Le son n°{numero} n'est pas un fichier Ogg."
            )));
        }
    }
    let meta = mcmeta();
    let index = sounds_json(sons.keys());
    let mut fichiers: Vec<(String, &[u8])> = vec![
        ("pack.mcmeta".to_owned(), meta.as_bytes()),
        (format!("assets/{ESPACE}/sounds.json"), index.as_bytes()),
    ];
    for (numero, son) in sons {
        fichiers.push((chemin_du_son(*numero), son.as_slice()));
    }
    zip(&fichiers)
}

/// Où vit le pack : à côté de la base, seul dossier où l'agent peut écrire dans son image.
pub fn chemin(base_sqlite: &Path) -> PathBuf {
    base_sqlite
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .join(NOM_FICHIER)
}

/// Écrit le pack de façon atomique : un fichier temporaire voisin, puis un renommage. Le
/// service ne peut donc jamais servir un pack à moitié écrit.
pub fn ecrire(chemin: &Path, pack: &[u8]) -> Result<(), ErreurPack> {
    let dossier = chemin
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(dossier)?;
    let mut temporaire = tempfile::NamedTempFile::new_in(dossier)?;
    temporaire.write_all(pack)?;
    temporaire.as_file().sync_all()?;
    temporaire.persist(chemin).map_err(|e| e.error)?;
    Ok(())
}

/// Écrit un pack vide s'il n'y en a pas encore : le jeu trouve toujours un pack à l'adresse
/// annoncée par le serveur, même avant la première dictée.
pub fn assurer(chemin: &Path) -> Result<(), ErreurPack> {
    if chemin.exists() {
        return Ok(());
    }
    ecrire(chemin, &construire(&BTreeMap::new())?)
}
