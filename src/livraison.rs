//! Ce que l'image de l'agent livre au serveur Minecraft avant son démarrage : le datapack,
//! le plugin `mine-de-rien-voix` et les réglages du chat vocal.
//!
//! Un service ponctuel du compose lance `mine-de-rien livrer`, puis le serveur démarre et
//! recopie ces dossiers chez lui. Agent, datapack et plugin viennent de la même image, donc
//! de la même version.
//!
//! Le datapack voyage dans le binaire (`datapack.rs`). Le plugin est un fichier `.jar`
//! construit par la même chaîne que l'image et posé à côté du binaire : l'embarquer dans le
//! binaire ferait dépendre `cargo build` d'une construction Java.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Où l'image garde le plugin (`docker/agent.Dockerfile`).
pub const PLUGIN_DE_L_IMAGE: &str = "/usr/local/share/mine-de-rien/mine-de-rien-voix.jar";
/// Le nom sous lequel le serveur le charge : toujours le même, une version chasse l'autre.
pub const NOM_DU_PLUGIN: &str = "mine-de-rien-voix.jar";

/// Les réglages du chat vocal (Simple Voice Chat) que Mine de rien veut : seules les clés qui
/// s'écartent des valeurs d'usage. Le plugin complète le fichier à son premier démarrage.
pub const REGLAGES_DU_CHAT_VOCAL: &str = "\
# Réglages du chat vocal (Simple Voice Chat), écrits par Mine de rien à la première
# installation. Ce fichier n'est plus touché ensuite : le modifier ici, puis redémarrer le
# serveur, qui le recopie chez lui.

# Port UDP entre le jeu et le serveur : celui que publie docker-compose.yml.
port=24454
# Les joueurs ne forment pas de groupes de discussion : le chat vocal ne sert qu'à faire
# entendre la voix de Mine de rien.
enable_groups=false
# L'adresse que le jeu joint pour le chat vocal. Vide : celle qu'il a tapée pour rejoindre
# le serveur, ce qui convient à la maison.
voice_host=
";

#[derive(Debug, thiserror::Error)]
pub enum ErreurLivraison {
    #[error("{0}")]
    Refus(String),
    #[error("{0}")]
    Fichier(#[from] io::Error),
}

/// Recopie le plugin `source` dans `destination`, sous son nom fixe, et y écrit les réglages
/// du chat vocal s'ils n'y sont pas encore. Rend le chemin du plugin recopié.
///
/// La copie passe par un fichier voisin, renommé une fois complet : le serveur ne charge
/// jamais un `.jar` à moitié écrit.
pub fn recopier_plugin(destination: &Path, source: &Path) -> Result<PathBuf, ErreurLivraison> {
    let octets = fs::read(source).map_err(|e| {
        ErreurLivraison::Refus(format!(
            "plugin introuvable ({}) : {e}. Hors de l'image, le construire (plugin/README.md) \
             et le désigner avec --source-plugin.",
            source.display()
        ))
    })?;
    // Un .jar est une archive zip : un autre fichier ne se chargerait pas, autant le dire ici.
    if !octets.starts_with(b"PK") {
        return Err(ErreurLivraison::Refus(format!(
            "{} n'est pas un fichier .jar.",
            source.display()
        )));
    }
    fs::create_dir_all(destination)?;
    let cible = destination.join(NOM_DU_PLUGIN);
    let provisoire = destination.join(format!(".{NOM_DU_PLUGIN}.tmp"));
    fs::write(&provisoire, &octets)?;
    fs::rename(&provisoire, &cible)?;

    let reglages = destination.join("voicechat/voicechat-server.properties");
    if !reglages.exists() {
        if let Some(dossier) = reglages.parent() {
            fs::create_dir_all(dossier)?;
        }
        fs::write(&reglages, REGLAGES_DU_CHAT_VOCAL)?;
    }
    Ok(cible)
}
