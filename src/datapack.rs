//! Recopie du datapack `mine_de_rien` vers le dossier que lit le serveur Minecraft.
//!
//! Le datapack voyage dans le binaire : il y est embarqué à la compilation. Un service
//! ponctuel lance `mine-de-rien datapack /datapacks` avant le serveur, qui le recopie à son
//! tour dans `world/datapacks`. Code et datapack ont donc toujours la même version.
//!
//! `--source` recopie un autre dossier à la place du datapack embarqué.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use include_dir::{Dir, include_dir};

pub const NOM: &str = "mine_de_rien";

/// Le datapack du dépôt, tel qu'il était à la compilation.
pub static EMBARQUE: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/datapacks/mine_de_rien");

#[derive(Debug, thiserror::Error)]
pub enum ErreurDatapack {
    #[error("{0}")]
    Refus(String),
    #[error("{0}")]
    Fichier(#[from] io::Error),
}

fn copier_dossier(source: &Path, cible: &Path) -> io::Result<()> {
    fs::create_dir_all(cible)?;
    for entree in fs::read_dir(source)? {
        let entree = entree?;
        let vers = cible.join(entree.file_name());
        if entree.file_type()?.is_dir() {
            copier_dossier(&entree.path(), &vers)?;
        } else {
            fs::copy(entree.path(), vers)?;
        }
    }
    Ok(())
}

fn ecrire_embarque(dossier: &Dir<'_>, cible: &Path) -> io::Result<()> {
    fs::create_dir_all(cible)?;
    for fichier in dossier.files() {
        // Les chemins embarqués sont relatifs à la racine du datapack.
        if let Some(nom) = fichier.path().file_name() {
            fs::write(cible.join(nom), fichier.contents())?;
        }
    }
    for sous_dossier in dossier.dirs() {
        if let Some(nom) = sous_dossier.path().file_name() {
            ecrire_embarque(sous_dossier, &cible.join(nom))?;
        }
    }
    Ok(())
}

/// Remplace `destination/mine_de_rien` par une copie complète de `source`, ou du datapack
/// embarqué si `source` est `None`.
///
/// Un fichier retiré de la source disparaît aussi de la destination. La copie passe par un
/// dossier temporaire voisin, renommé une fois complet : une copie interrompue ne laisse
/// jamais un datapack à moitié écrit sous le nom attendu.
pub fn recopier(destination: &Path, source: Option<&Path>) -> Result<PathBuf, ErreurDatapack> {
    let source = match source {
        Some(chemin) => {
            let resolue = chemin
                .canonicalize()
                .unwrap_or_else(|_| chemin.to_path_buf());
            if !resolue.join("pack.mcmeta").is_file() {
                return Err(ErreurDatapack::Refus(format!(
                    "{} n'est pas un datapack : pack.mcmeta introuvable.",
                    resolue.display()
                )));
            }
            Some(resolue)
        }
        None => None,
    };
    fs::create_dir_all(destination)?;
    let cible = destination.canonicalize()?.join(NOM);
    if let Some(source) = &source
        && (cible == *source || cible.starts_with(source) || source.starts_with(&cible))
    {
        return Err(ErreurDatapack::Refus(format!(
            "La destination {} recouvre la source {}.",
            cible.display(),
            source.display()
        )));
    }

    let parent = cible.parent().unwrap_or(destination);
    // Supprimé à la sortie de la fonction, quoi qu'il arrive.
    let temporaire = tempfile::Builder::new()
        .prefix(&format!(".{NOM}-"))
        .tempdir_in(parent)?;
    let copie = temporaire.path().join(NOM);
    match &source {
        Some(source) => copier_dossier(source, &copie)?,
        None => ecrire_embarque(&EMBARQUE, &copie)?,
    }
    match fs::symlink_metadata(&cible) {
        Ok(etat) if etat.is_dir() => fs::remove_dir_all(&cible)?,
        Ok(_) => fs::remove_file(&cible)?,
        Err(_) => {}
    }
    fs::rename(&copie, &cible)?;
    Ok(cible)
}
