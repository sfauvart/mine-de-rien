//! Le tableau de la dictée : le texte entier, affiché derrière le coffre de l'atelier.
//!
//! **Le désir.** Le même que pour le coffre de dictée, dont il est le décor : l'enfant lit sa
//! dictée à trous comme sur la feuille de l'école, au lieu de deux mots sur un panneau. Le
//! trou du coffre posé est en jaune, ceux des coffres qui attendent sont gris, et chaque mot
//! trouvé pendant la séance s'écrit en vert à sa place.
//!
//! **Commandes**, vérifiées sur Minecraft Java 26.3 (Paper). Le texte est assemblé dans le
//! stockage, morceau par morceau, parce qu'une commande RCON ne dépasse pas 1 446 octets ;
//! l'entité le recopie ensuite d'un coup :
//!
//! ```text
//! data modify storage mine_de_rien:dictee tableau set value [{text:""}]
//! data modify storage mine_de_rien:dictee tableau append value {text:"Dans la ",color:"white"}
//! data modify storage mine_de_rien:dictee tableau append value {text:"_____",color:"yellow",bold:true}
//! kill @e[type=minecraft:text_display,tag=mdr_tableau]
//! summon minecraft:text_display X+0.5 Y+1.2 Z-1.5 {Tags:["mdr_tableau"],line_width:220,
//!     alignment:"left",brightness:{sky:15,block:15},text:{text:""}}
//! data modify entity @e[type=minecraft:text_display,tag=mdr_tableau,limit=1] text
//!     set from storage mine_de_rien:dictee tableau
//! ```
//!
//! Le tableau fait face au sud, comme le coffre, et son bas est au-dessus du coffre. Il est
//! refait à chaque pose et à chaque ouverture d'un coffre de dictée ; il disparaît quand un
//! coffre d'une autre matière est posé, quand le coffre de dictée est annulé, et avant un
//! déplacement de l'atelier. Après le dernier coffre d'une séance, il reste : le texte
//! entier, sans trou, à relire.
//!
//! **Ce que l'enfant ne voit jamais** : un mot encore à trouver. Il est masqué partout où il
//! apparaît dans le texte, pas seulement à sa position (« plus » deux fois dans la dictée).
//! Aucune graphie fausse non plus : un mot ne s'écrit que trouvé, tel qu'il est dans le texte.
//!
//! **Contournements.** Le tableau est une entité sans collision ni inventaire : rien à
//! casser, rien à prendre. Lire les autres mots du texte est le principe d'une dictée à trous.

use std::collections::HashMap;

use serde_json::{Map, Value};

use crate::exercises::francais::dictee::{Phrase, Position, TROU};
use crate::games::atelier::Atelier;
use crate::games::dictee::STOCKAGE;
use crate::minecraft::texte;
use crate::util::json_python;

pub const TAG: &str = "mdr_tableau";
/// Largeur du texte, en pixels du jeu : 40 pixels font un bloc.
const LARGEUR: u32 = 220;
/// Un morceau de texte par commande, pour rester loin de la limite de RCON.
const MORCEAU_MAX: usize = 200;

/// Où en est un trou de la séance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trou {
    /// Le mot du coffre posé.
    Actif,
    /// Le mot d'un coffre qui attend dans la file.
    Attente,
    /// Un mot trouvé pendant cette séance.
    Trouve,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Genre {
    Texte,
    TrouActif,
    TrouAttente,
    Trouve,
}

/// Un bout du tableau, d'une seule couleur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Morceau {
    pub texte: String,
    pub genre: Genre,
}

fn tranche(texte: &str, debut: usize, fin: usize) -> String {
    texte
        .chars()
        .skip(debut)
        .take(fin.saturating_sub(debut))
        .collect()
}

/// Le texte de la dictée, découpé en morceaux de couleur. Les phrases se suivent, séparées
/// par une espace.
pub fn morceaux(phrases: &[Phrase], trous: &HashMap<Position, Trou>) -> Vec<Morceau> {
    // Les mots encore à trouver : masqués partout.
    let caches: Vec<String> = trous
        .iter()
        .filter(|(_, etat)| **etat != Trou::Trouve)
        .filter_map(|(position, _)| {
            let mot = phrases.get(position.phrase)?.mots.get(position.mot)?;
            Some(mot.texte.to_lowercase())
        })
        .collect();
    let mut resultat: Vec<Morceau> = Vec::new();
    let mut ajouter = |texte: String, genre: Genre| {
        if texte.is_empty() {
            return;
        }
        match resultat.last_mut() {
            Some(dernier) if dernier.genre == genre && genre == Genre::Texte => {
                dernier.texte.push_str(&texte);
            }
            _ => resultat.push(Morceau { texte, genre }),
        }
    };
    for (rang, phrase) in phrases.iter().enumerate() {
        if rang > 0 {
            ajouter(" ".to_owned(), Genre::Texte);
        }
        let mut curseur = 0;
        for (indice, mot) in phrase.mots.iter().enumerate() {
            ajouter(tranche(&phrase.texte, curseur, mot.debut), Genre::Texte);
            curseur = mot.fin;
            let position = Position {
                phrase: rang,
                mot: indice,
            };
            let cache = caches.contains(&mot.texte.to_lowercase());
            match trous.get(&position) {
                Some(Trou::Actif) => ajouter(TROU.to_owned(), Genre::TrouActif),
                Some(Trou::Attente) => ajouter(TROU.to_owned(), Genre::TrouAttente),
                _ if cache => ajouter(TROU.to_owned(), Genre::TrouAttente),
                Some(Trou::Trouve) => ajouter(mot.texte.clone(), Genre::Trouve),
                None => ajouter(mot.texte.clone(), Genre::Texte),
            }
        }
        let longueur = phrase.texte.chars().count();
        ajouter(tranche(&phrase.texte, curseur, longueur), Genre::Texte);
    }
    resultat
}

fn selecteur() -> String {
    format!("@e[type=minecraft:text_display,tag={TAG}]")
}

fn composant(texte: &str, genre: Genre) -> String {
    let mut objet = Map::new();
    objet.insert(
        "text".to_owned(),
        Value::String(texte::nettoyer(texte, MORCEAU_MAX)),
    );
    let couleur = match genre {
        Genre::Texte => "white",
        Genre::TrouActif => "yellow",
        Genre::TrouAttente => "gray",
        Genre::Trouve => "green",
    };
    objet.insert("color".to_owned(), Value::String(couleur.to_owned()));
    if matches!(genre, Genre::TrouActif | Genre::Trouve) {
        objet.insert("bold".to_owned(), Value::Bool(true));
    }
    json_python(&Value::Object(objet))
}

/// Le tableau disparaît.
pub fn commande_effacement() -> String {
    format!("kill {}", selecteur())
}

/// Commandes qui affichent (ou refont) le tableau derrière le coffre de l'atelier `a`.
pub fn commandes(a: &Atelier, morceaux: &[Morceau]) -> Vec<String> {
    let mut commandes = vec![format!(
        "data modify storage {STOCKAGE} tableau set value [{{text:\"\"}}]"
    )];
    for morceau in morceaux {
        let caracteres: Vec<char> = morceau.texte.chars().collect();
        for bout in caracteres.chunks(MORCEAU_MAX) {
            let bout: String = bout.iter().collect();
            commandes.push(format!(
                "data modify storage {STOCKAGE} tableau append value {}",
                composant(&bout, morceau.genre)
            ));
        }
    }
    commandes.push(commande_effacement());
    commandes.push(format!(
        "summon minecraft:text_display {:.1} {:.1} {:.1} {{Tags:[\"{TAG}\"],line_width:{LARGEUR},\
         alignment:\"left\",brightness:{{sky:15,block:15}},text:{{text:\"\"}}}}",
        f64::from(a.x) + 0.5,
        f64::from(a.y) + 1.2,
        f64::from(a.z) - 1.5,
    ));
    commandes.push(format!(
        "data modify entity {}limit=1] text set from storage {STOCKAGE} tableau",
        selecteur().trim_end_matches(']').to_owned() + ","
    ));
    commandes
}
