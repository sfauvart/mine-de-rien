//! Trouver où construire l'atelier, autour de l'enfant, sans relevé manuel.
//!
//! **Le désir.** Le parent ne connaît pas Minecraft : pas de F3, pas de coordonnées à recopier.
//! L'agent cherche lui-même, près de l'enfant, un endroit où l'atelier tient sans rien casser.
//! `chercher` ne construit rien ; `construire_pres_de` et `deplacer_pres_de` enchaînent position
//! de l'enfant, recherche et construction, pour la commande `/atelier` des parents.
//!
//! **Commandes**, vérifiées sur Minecraft Java 26.3 (Paper) :
//!
//! ```text
//!     kill @e[type=minecraft:marker,tag=mdr_sonde]                # une sonde oubliée
//!     summon minecraft:marker X Y Z {Tags:["mdr_sonde"]}          # à la position de l'enfant
//!     execute positioned X 0 Z positioned over motion_blocking_no_leaves
//!         run tp @e[type=minecraft:marker,tag=mdr_sonde,limit=1] ~ ~ ~
//!     # « Teleported Marker to 141.500000, -59.000000, 150.500000 » : Y est le niveau juste
//!     # au-dessus du sol, donc celui du coffre. « That position is not loaded » hors chunk chargé.
//!     execute if block X Y Z <bloc|#tag>                          # voir `atelier.py`
//!     data get entity <pseudo> Pos
//!     # « Steve has the following entity data: [3.5d, -60.0d, 4.25d] », « No entity was found »
//!     execute in minecraft:overworld if entity @a[name=<pseudo>,distance=0..]
//!     # `distance` ne retient que les entités de la dimension de l'exécution
//! ```
//!
//! La sonde porte son propre tag : le datapack ne regarde que `mdr_atelier`, elle ne crée donc
//! pas de zone en mode aventure. Elle est retirée même en cas d'erreur.
//!
//! **Recherche.** Par couronnes carrées, de 8 à 32 blocs de l'enfant, toujours dans le même ordre :
//! le premier centre valide gagne. Un centre est valide si :
//! - ses 25 colonnes sont exactement au même niveau, sans terrassement ;
//! - ce niveau est à 16 blocs au plus de celui de l'enfant, pour ne pas construire loin au-dessus
//!   d'une mine ;
//! - les blocs passent `atelier.place_libre` : sol naturel, de l'air ou de l'herbe au-dessus. L'eau,
//!   les troncs et les constructions sont écartés par là ou par le niveau des colonnes.
//!
//! 8 blocs au moins : le 5 × 5 ne tombe pas sur l'enfant, et il reste hors de la zone en mode
//! aventure (6 blocs autour du marqueur).
//!
//! Autour d'un joueur connecté, les chunks sont chargés : la recherche ne fait pas de `forceload`,
//! et un chunk non chargé écarte simplement le centre.
//!
//! **Message à l'enfant et rapport aux parents.** Une ligne dans le chat de l'enfant, avec la
//! distance et la direction, calculées ici. Les parents reçoivent les coordonnées et la distance,
//! jamais le pseudo.
//!
//! **Contournements.** L'enfant ne déclenche rien : seuls les parents le peuvent, sur Telegram.
//! Il peut choisir où il se tient au moment de la commande, sans autre effet qu'un atelier à cet
//! endroit, que les parents peuvent déplacer. Une terrasse en terre qu'il aurait posée passe pour du
//! sol : la commande ne distingue pas un bloc posé d'un bloc naturel.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::adapters::game::base::AdaptateurJeu;
use crate::adapters::game::rcon::ErreurRcon;
use crate::games::atelier::{Atelier, ErreurAtelier, construire, demonter, place_libre};
use crate::minecraft::texte::{self, Style};
use crate::util::strip;

pub const DISTANCE_MIN: i32 = 8;
pub const RAYON: i32 = 32;
pub const ECART_VERTICAL: i32 = 16;

pub const TAG_SONDE: &str = "mdr_sonde";
const SONDES: &str = "@e[type=minecraft:marker,tag=mdr_sonde]";
const SONDE: &str = "@e[type=minecraft:marker,tag=mdr_sonde,limit=1]";
const NOMBRE: &str = r"(-?[0-9]+(?:\.[0-9]+)?)";
static TELEPORTE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!("Teleported .* to {NOMBRE}, {NOMBRE}, {NOMBRE}")).unwrap()
});
static POSITION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"\[{NOMBRE}d, {NOMBRE}d, {NOMBRE}d\]")).unwrap());

// Pour les parents : pourquoi rien n'a été construit, et quel geste faire.
pub const ENFANT_ABSENT: &str =
    "L'enfant n'est pas connecté : je cherche autour de lui. Réessayer quand il joue.";
pub const AUTRE_DIMENSION: &str =
    "L'enfant est dans le Nether ou l'End : réessayer quand il est revenu dans le monde normal.";
pub const LOIN_DE_LA_SURFACE: &str =
    "L'enfant est sous terre ou très haut : réessayer quand il est à la surface, près de sa base.";
pub const AUCUN_TERRAIN: &str = "Pas de terrain plat et dégagé à moins de 32 blocs de l'enfant. \
    L'emmener vers un terrain dégagé (prairie, plaine), puis relancer /atelier.";

fn refus<T>(message: &str) -> Result<T, ErreurAtelier> {
    Err(ErreurAtelier::Refus(message.to_owned()))
}

/// Les centres possibles, couronne par couronne, du plus proche au plus loin.
pub fn centres(x: i32, z: i32, minimum: i32, maximum: i32) -> Vec<(i32, i32)> {
    let mut sortie = Vec::new();
    for r in minimum..=maximum {
        sortie.extend((-r..=r).map(|d| (x + d, z - r)));
        sortie.extend((-r..=r).map(|d| (x + d, z + r)));
        sortie.extend((-r + 1..r).map(|d| (x - r, z + d)));
        sortie.extend((-r + 1..r).map(|d| (x + r, z + d)));
    }
    sortie
}

pub fn commande_hauteur(x: i32, z: i32) -> String {
    format!(
        "execute positioned {x} 0 {z} positioned over motion_blocking_no_leaves run tp {SONDE} ~ ~ ~"
    )
}

type Hauteurs = HashMap<(i32, i32), Option<i32>>;

/// Le niveau juste au-dessus du sol, gardé d'un centre à l'autre ; `None` si non chargé.
async fn hauteur(
    jeu: &dyn AdaptateurJeu,
    x: i32,
    z: i32,
    connues: &mut Hauteurs,
) -> Result<Option<i32>, ErreurRcon> {
    if let Some(connue) = connues.get(&(x, z)) {
        return Ok(*connue);
    }
    let reponse = jeu.commande(&commande_hauteur(x, z)).await?;
    // Troncature vers zéro, comme `int(float(…))`.
    let lue = TELEPORTE
        .captures(&reponse)
        .and_then(|m| m[2].parse::<f64>().ok())
        .map(|y| y as i32);
    connues.insert((x, z), lue);
    Ok(lue)
}

/// Le niveau des 25 colonnes autour de (x, z), s'il est le même pour toutes.
async fn niveau_commun(
    jeu: &dyn AdaptateurJeu,
    x: i32,
    z: i32,
    connues: &mut Hauteurs,
) -> Result<Option<i32>, ErreurRcon> {
    let Some(niveau) = hauteur(jeu, x, z, connues).await? else {
        return Ok(None);
    };
    for dz in -2..=2 {
        for dx in -2..=2 {
            if hauteur(jeu, x + dx, z + dz, connues).await? != Some(niveau) {
                return Ok(None);
            }
        }
    }
    Ok(Some(niveau))
}

async fn sonder(
    jeu: &dyn AdaptateurJeu,
    x: i32,
    y: i32,
    z: i32,
) -> Result<Option<Atelier>, ErreurAtelier> {
    let mut connues = Hauteurs::new();
    if let Some(ici) = hauteur(jeu, x, z, &mut connues).await?
        && (ici - y).abs() > ECART_VERTICAL
    {
        return refus(LOIN_DE_LA_SURFACE);
    }
    for (cx, cz) in centres(x, z, DISTANCE_MIN, RAYON) {
        let Some(niveau) = niveau_commun(jeu, cx, cz, &mut connues).await? else {
            continue;
        };
        if (niveau - y).abs() > ECART_VERTICAL {
            continue;
        }
        let candidat = Atelier::new(cx, niveau, cz);
        if place_libre(jeu, &candidat).await? {
            return Ok(Some(candidat));
        }
    }
    Ok(None)
}

/// Le premier emplacement valide autour de (x, y, z), ou `None`. Ne construit rien.
pub async fn chercher(
    jeu: &dyn AdaptateurJeu,
    x: i32,
    y: i32,
    z: i32,
) -> Result<Option<Atelier>, ErreurAtelier> {
    jeu.commande(&format!("kill {SONDES}")).await?;
    let reponse = jeu
        .commande(&format!(
            "summon minecraft:marker {x} {y} {z} {{Tags:[\"{TAG_SONDE}\"]}}"
        ))
        .await?;
    if !reponse.contains("Summoned") {
        return Err(ErreurAtelier::Refus(format!(
            "Impossible de sonder le terrain : {}",
            strip(&reponse)
        )));
    }
    let resultat = sonder(jeu, x, y, z).await;
    // La sonde est retirée même en cas d'erreur.
    let retrait = jeu.commande(&format!("kill {SONDES}")).await;
    let trouve = resultat?;
    retrait?;
    Ok(trouve)
}

/// Le bloc où se tient le joueur, dans l'Overworld ; un refus sinon.
pub async fn position_de(
    jeu: &dyn AdaptateurJeu,
    pseudo: &str,
) -> Result<(i32, i32, i32), ErreurAtelier> {
    let pseudo = texte::verifier_pseudo(pseudo).map_err(ErreurRcon::from)?;
    let reponse = jeu
        .commande(&format!("data get entity {pseudo} Pos"))
        .await?;
    let lire = |texte: &str| texte.parse::<f64>().ok().map(|v| v.floor() as i32);
    let position = POSITION
        .captures(&reponse)
        .and_then(|m| Some((lire(&m[1])?, lire(&m[2])?, lire(&m[3])?)));
    let Some(position) = position else {
        return refus(ENFANT_ABSENT);
    };
    let reponse = jeu
        .commande(&format!(
            "execute in minecraft:overworld if entity @a[name={pseudo},distance=0..]"
        ))
        .await?;
    if !strip(&reponse).starts_with("Test passed") {
        return refus(AUTRE_DIMENSION);
    }
    Ok(position)
}

/// Le point cardinal dominant : dans Minecraft, x croît vers l'est et z vers le sud.
pub fn direction(dx: i32, dz: i32) -> &'static str {
    if dx.abs() >= dz.abs() {
        if dx > 0 { "l'est" } else { "l'ouest" }
    } else if dz > 0 {
        "le sud"
    } else {
        "le nord"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Construction {
    pub atelier: Atelier,
    pub distance: i64,
    pub direction: &'static str,
}

/// Où construire près du joueur. Ne construit rien ; un refus sinon.
pub async fn trouver_pres_de(
    jeu: &dyn AdaptateurJeu,
    pseudo: &str,
) -> Result<Construction, ErreurAtelier> {
    let (x, y, z) = position_de(jeu, pseudo).await?;
    let Some(a) = chercher(jeu, x, y, z).await? else {
        return refus(AUCUN_TERRAIN);
    };
    let (dx, dz) = (a.x - x, a.z - z);
    let distance = f64::from(dx).hypot(f64::from(dz)).round() as i64;
    Ok(Construction {
        atelier: a,
        distance,
        direction: direction(dx, dz),
    })
}

pub async fn annoncer(
    jeu: &dyn AdaptateurJeu,
    pseudo: &str,
    c: &Construction,
) -> Result<(), ErreurRcon> {
    jeu.dire(
        pseudo,
        &format!(
            "Ton atelier vient d'apparaître, à {} blocs vers {}.",
            c.distance, c.direction
        ),
        &[("color", Style::Texte("gold"))],
    )
    .await
}

/// Construit l'atelier près du joueur et le lui dit.
pub async fn construire_pres_de(
    jeu: &dyn AdaptateurJeu,
    pseudo: &str,
) -> Result<Construction, ErreurAtelier> {
    let c = trouver_pres_de(jeu, pseudo).await?;
    construire(jeu, &c.atelier).await?;
    annoncer(jeu, pseudo, &c).await?;
    Ok(c)
}

/// Déplace l'atelier près du joueur. Recherche d'abord : sans résultat, rien ne bouge.
pub async fn deplacer_pres_de(
    jeu: &dyn AdaptateurJeu,
    pseudo: &str,
    ancien: &Atelier,
) -> Result<Construction, ErreurAtelier> {
    let c = trouver_pres_de(jeu, pseudo).await?;
    demonter(jeu, ancien).await?;
    construire(jeu, &c.atelier).await?;
    annoncer(jeu, pseudo, &c).await?;
    Ok(c)
}
