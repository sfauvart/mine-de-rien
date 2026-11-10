//! L'atelier : le lieu fixe, près de la base de l'enfant, où apparaissent les coffres du soir.
//!
//! Plan, vu de dessus, `C` étant les coordonnées données dans `config.toml` (`[atelier]`) :
//!
//! ```text
//!     z-2  . . . . L        L : lanterne
//!     z-1  . . . . .        P : emplacement du panneau d'énoncé (posé par le coffre du soir)
//!     z    . P C E .        C : coffre, ouverture vers le sud (z+)
//!     z+1  . . . . .        E : enclume
//!     z+2  . . . . .        plancher 5 × 5 en pierre lisse, à y-1
//!        x-2     x+2
//! ```
//!
//! Commandes, vérifiées sur Minecraft Java 26.3 (Paper) :
//!
//! ```text
//!     forceload add X-2 Z-2 X+2 Z+2          # sans joueur proche, rien n'est chargé depuis 1.21
//!     execute if block X Y Z <bloc|#tag>     # « Test passed » / « Test failed », bloc par bloc
//!     fill X-2 Y-1 Z-2 X+2 Y-1 Z+2 minecraft:smooth_stone
//!     execute unless block ... run setblock ...   # ne remplace jamais un coffre déjà là
//!     summon minecraft:marker X+0.5 Y Z+0.5 {Tags:["mdr_atelier"]}
//!     data modify storage mine_de_rien:atelier pos set value [X, Y, Z]
//! ```
//!
//! Le `forceload` reste en place : le coffre doit pouvoir être posé et surveillé pendant que
//! l'enfant joue ailleurs.
//!
//! Le marqueur `mdr_atelier` est lu par le datapack (`datapacks/mine_de_rien`), qui met en mode
//! aventure les joueurs en survie autour de lui : en aventure, on ne casse pas le coffre et on ne
//! pose pas d'entonnoir dessous.
//!
//! **Déplacement** (`demonter`, appelé par `/atelier`) : coffre, panneau, enclume et lanterne
//! laissent place à de l'air, le plancher devient de la terre (le sol d'origine n'est pas connu),
//! puis marqueur, `forceload remove` et position enregistrée, en dernier. Interrompu, il laisse
//! un monde sans atelier, que `/atelier` sait reconstruire.
//!
//! Avant de construire, l'agent vérifie que le volume au-dessus du plancher est de l'air ou de
//! l'herbe, et que le plancher est du sol naturel ; sinon il refuse, sans rien casser, en disant
//! où est le bloc qui gêne. Relancé, il répare sans dupliquer : il reconnaît alors ses propres
//! blocs, et ne touche pas au contenu d'un coffre déjà posé.
//!
//! La position écrite dans `mine_de_rien:atelier` fait foi (`lire_atelier`) : l'agent la relit
//! au lieu de garder celle de `config.toml`, qui ne sert plus qu'à la construction. Sans position
//! dans le monde, le coffre du soir ne se pose pas et Pixel ne téléporte pas : l'enfant serait
//! emmuré.

use crate::adapters::game::base::AdaptateurJeu;
use crate::adapters::game::rcon::ErreurRcon;
use crate::util::strip;

pub const TAG_MARQUEUR: &str = "mdr_atelier";
pub const STOCKAGE: &str = "mine_de_rien:atelier";
pub const PLANCHER: &str = "minecraft:smooth_stone";

/// Au-dessus du plancher : de l'air ou de l'herbe.
pub const LIBRE: [&str; 12] = [
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:short_grass",
    "minecraft:tall_grass",
    "minecraft:fern",
    "minecraft:large_fern",
    "minecraft:short_dry_grass",
    "minecraft:tall_dry_grass",
    "minecraft:bush",
    "minecraft:dead_bush",
    "minecraft:snow",
    "#minecraft:small_flowers",
];
/// Le plancher remplace du sol naturel, jamais une construction.
pub const SOL: [&str; 10] = [
    "minecraft:air",
    "minecraft:grass_block",
    "minecraft:podzol",
    "minecraft:mycelium",
    "minecraft:moss_block",
    "minecraft:gravel",
    "minecraft:snow_block",
    "#minecraft:dirt",
    "#minecraft:sand",
    "#minecraft:base_stone_overworld",
];

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ErreurAtelier {
    /// Construction refusée ; le message s'adresse au parent.
    #[error("{0}")]
    Refus(String),
    /// Le serveur n'a pas répondu : ce n'est pas un refus, le parent peut réessayer.
    #[error("{0}")]
    Rcon(#[from] ErreurRcon),
}

fn refus<T>(message: impl Into<String>) -> Result<T, ErreurAtelier> {
    Err(ErreurAtelier::Refus(message.into()))
}

pub type Position = (i32, i32, i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Atelier {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Atelier {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }

    pub fn coffre(&self) -> Position {
        (self.x, self.y, self.z)
    }

    pub fn panneau(&self) -> Position {
        (self.x - 1, self.y, self.z)
    }

    pub fn enclume(&self) -> Position {
        (self.x + 1, self.y, self.z)
    }

    pub fn lanterne(&self) -> Position {
        (self.x + 2, self.y, self.z - 2)
    }

    /// Les 5 × 5 × 3 blocs au-dessus du plancher.
    pub fn volume(&self) -> Vec<Position> {
        let mut positions = Vec::with_capacity(75);
        for dy in 0..3 {
            for dz in -2..=2 {
                for dx in -2..=2 {
                    positions.push((self.x + dx, self.y + dy, self.z + dz));
                }
            }
        }
        positions
    }

    pub fn plancher(&self) -> Vec<Position> {
        let mut positions = Vec::with_capacity(25);
        for dz in -2..=2 {
            for dx in -2..=2 {
                positions.push((self.x + dx, self.y - 1, self.z + dz));
            }
        }
        positions
    }

    /// Les blocs que l'atelier pose lui-même, reconnus quand on le relance.
    pub fn blocs_propres(&self) -> [(Position, &'static str); 4] {
        [
            (self.coffre(), "minecraft:chest"),
            (self.panneau(), "#minecraft:standing_signs"),
            (self.enclume(), "#minecraft:anvil"),
            (self.lanterne(), "minecraft:lantern"),
        ]
    }
}

pub fn pos(p: Position) -> String {
    format!("{} {} {}", p.0, p.1, p.2)
}

pub fn commande_forceload(a: &Atelier) -> String {
    format!(
        "forceload add {} {} {} {}",
        a.x - 2,
        a.z - 2,
        a.x + 2,
        a.z + 2
    )
}

fn commande_forceload_retire(a: &Atelier) -> String {
    commande_forceload(a).replace(" add ", " remove ")
}

/// Pour chaque position, les commandes `execute if block` dont une doit passer.
pub fn verifications_de_place(a: &Atelier, deja_construit: bool) -> Vec<(Position, Vec<String>)> {
    let propres = a.blocs_propres();
    let test = |p: Position, bloc: &str| format!("execute if block {} {bloc}", pos(p));
    let mut tests = Vec::with_capacity(100);
    for p in a.volume() {
        let mut permis: Vec<&str> = LIBRE.to_vec();
        if deja_construit
            && let Some((_, bloc)) = propres.iter().find(|(position, _)| *position == p)
        {
            permis.push(bloc);
        }
        tests.push((p, permis.iter().map(|bloc| test(p, bloc)).collect()));
    }
    for p in a.plancher() {
        let mut permis: Vec<&str> = SOL.to_vec();
        if deja_construit {
            permis.push(PLANCHER);
        }
        tests.push((p, permis.iter().map(|bloc| test(p, bloc)).collect()));
    }
    tests
}

/// Commandes de construction, idempotentes : relancées, elles ne dupliquent rien.
pub fn commandes_construction(a: &Atelier) -> Vec<String> {
    let (c, e, lt) = (pos(a.coffre()), pos(a.enclume()), pos(a.lanterne()));
    vec![
        format!(
            "fill {} {} {} {} {} {} {PLANCHER}",
            a.x - 2,
            a.y - 1,
            a.z - 2,
            a.x + 2,
            a.y - 1,
            a.z + 2
        ),
        format!(
            "execute unless block {c} minecraft:chest run setblock {c} minecraft:chest[facing=south]"
        ),
        format!(
            "execute unless block {e} #minecraft:anvil run setblock {e} minecraft:anvil[facing=east]"
        ),
        format!("execute unless block {lt} minecraft:lantern run setblock {lt} minecraft:lantern"),
        format!("kill @e[type=minecraft:marker,tag={TAG_MARQUEUR}]"),
        // Centre du bloc : x + 0,5, ce qui donne -2.5 pour x = -3 (et non « -3.5 »).
        format!(
            "summon minecraft:marker {:.1} {} {:.1} {{Tags:[\"{TAG_MARQUEUR}\"]}}",
            f64::from(a.x) + 0.5,
            a.y,
            f64::from(a.z) + 0.5
        ),
        format!(
            "data modify storage {STOCKAGE} pos set value [{}, {}, {}]",
            a.x, a.y, a.z
        ),
    ]
}

/// Retire tout ce que l'atelier a posé ; la position enregistrée disparaît en dernier.
pub fn commandes_demontage(a: &Atelier) -> Vec<String> {
    let mut commandes: Vec<String> = a
        .blocs_propres()
        .iter()
        .map(|(p, _)| format!("setblock {} minecraft:air", pos(*p)))
        .collect();
    commandes.extend([
        format!(
            "fill {} {} {} {} {} {} minecraft:dirt",
            a.x - 2,
            a.y - 1,
            a.z - 2,
            a.x + 2,
            a.y - 1,
            a.z + 2
        ),
        format!("kill @e[type=minecraft:marker,tag={TAG_MARQUEUR}]"),
        commande_forceload_retire(a),
        format!("data remove storage {STOCKAGE} pos"),
    ]);
    commandes
}

fn refuse(reponse: &str) -> bool {
    reponse.contains("rror") || reponse.contains("nknown") || reponse.contains("Incorrect")
}

fn passe(reponse: &str) -> bool {
    strip(reponse).starts_with("Test passed")
}

fn non_charge(reponse: &str) -> bool {
    reponse.contains("not loaded")
}

/// Ce que disent les tests d'une position : libre, occupée, ou hors des chunks chargés.
enum Place {
    Libre,
    Occupee,
    NonChargee,
}

async fn essayer(jeu: &dyn AdaptateurJeu, tests: &[String]) -> Result<Place, ErreurRcon> {
    for test in tests {
        let reponse = jeu.commande(test).await?;
        if non_charge(&reponse) {
            return Ok(Place::NonChargee);
        }
        if passe(&reponse) {
            return Ok(Place::Libre);
        }
    }
    Ok(Place::Occupee)
}

/// Un atelier neuf tient ici sans rien casser. Un chunk non chargé compte comme non.
pub async fn place_libre(jeu: &dyn AdaptateurJeu, a: &Atelier) -> Result<bool, ErreurRcon> {
    for (_position, tests) in verifications_de_place(a, false) {
        if !matches!(essayer(jeu, &tests).await?, Place::Libre) {
            return Ok(false);
        }
    }
    Ok(true)
}

pub async fn position_enregistree(jeu: &dyn AdaptateurJeu) -> Result<Option<Position>, ErreurRcon> {
    let reponse = jeu
        .commande(&format!("data get storage {STOCKAGE} pos"))
        .await?;
    // « Storage mine_de_rien:atelier has the following contents: [0, -60, 8] »
    let (Some(debut), Some(fin)) = (reponse.rfind('['), reponse.rfind(']')) else {
        return Ok(None);
    };
    if fin < debut {
        return Ok(None);
    }
    let valeurs: Vec<Option<i32>> = reponse[debut + 1..fin]
        .split(',')
        .map(|v| strip(v).trim_end_matches('d').parse().ok())
        .collect();
    Ok(match valeurs[..] {
        [Some(x), Some(y), Some(z)] => Some((x, y, z)),
        _ => None,
    })
}

/// L'atelier construit dans le monde du serveur, s'il y en a un.
pub async fn lire_atelier(jeu: &dyn AdaptateurJeu) -> Result<Option<Atelier>, ErreurRcon> {
    Ok(position_enregistree(jeu)
        .await?
        .map(|(x, y, z)| Atelier::new(x, y, z)))
}

/// Pour les parents : quoi faire quand le monde n'a pas d'atelier.
pub const PAS_CONSTRUIT: &str = "l'atelier n'est pas construit dans ce monde. Taper /atelier ici \
    pendant que l'enfant est connecté, près de sa base.";

async fn executer(jeu: &dyn AdaptateurJeu, commandes: &[String]) -> Result<(), ErreurAtelier> {
    for commande in commandes {
        let reponse = jeu.commande(commande).await?;
        if refuse(&reponse) {
            return refus(format!(
                "Le serveur a refusé « {commande} » : {}",
                strip(&reponse)
            ));
        }
    }
    Ok(())
}

/// Construit ou répare l'atelier. Renvoie un compte rendu, ou le refus.
pub async fn construire(jeu: &dyn AdaptateurJeu, a: &Atelier) -> Result<String, ErreurAtelier> {
    let ancienne = position_enregistree(jeu).await?;
    if let Some(ancienne) = ancienne
        && ancienne != a.coffre()
    {
        return refus(format!(
            "Un atelier existe déjà en {}. Pour le déplacer : /atelier sur \
             Telegram, ou remettre ces coordonnées dans config.toml.",
            pos(ancienne)
        ));
    }
    let deja_construit = ancienne == Some(a.coffre());

    let reponse = jeu.commande(&commande_forceload(a)).await?;
    if !reponse.contains("orce") && !reponse.contains("already") {
        return refus(format!(
            "Impossible de garder l'atelier chargé : {}",
            strip(&reponse)
        ));
    }

    for (position, tests) in verifications_de_place(a, deja_construit) {
        match essayer(jeu, &tests).await? {
            Place::Libre => {}
            Place::NonChargee => {
                return refus(format!("La position {} n'est pas chargée.", pos(position)));
            }
            Place::Occupee => {
                if !deja_construit {
                    jeu.commande(&commande_forceload_retire(a)).await?;
                }
                return refus(format!(
                    "Le bloc en {} gêne : l'atelier a besoin d'un plancher de sol \
                     naturel et, au-dessus, de 5 × 5 × 3 blocs d'air ou d'herbe. \
                     Rien n'a été cassé.",
                    pos(position)
                ));
            }
        }
    }

    executer(jeu, &commandes_construction(a)).await?;

    let verbe = if deja_construit {
        "réparé"
    } else {
        "construit"
    };
    Ok(format!("Atelier {verbe} en {}.", pos(a.coffre())))
}

/// Retire l'atelier du monde. À n'appeler que le coffre vide : son contenu serait perdu.
pub async fn demonter(jeu: &dyn AdaptateurJeu, a: &Atelier) -> Result<(), ErreurAtelier> {
    executer(jeu, &commandes_demontage(a)).await
}
