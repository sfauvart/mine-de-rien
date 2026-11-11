//! Dictée : ce que le coffre du soir fait de plus quand sa clé est un mot de dictée.
//!
//! **Le désir.** Le même que pour tout coffre du soir : des diamants, à l'atelier. Le mot à
//! écrire est la clé. La dictée de l'enfant est une dictée à trous : il entend la phrase
//! entière, il la voit dans le chat avec `_____` à la place du mot, et il tape le mot sur
//! l'enclume. Le coffre lui-même (verrou, preuve, détection, récompense, rangement) est celui
//! de `coffre_du_soir.rs`, inchangé. Ce module porte ce qu'il y a autour.
//!
//! **Le texte entier** de la dictée est affiché derrière le coffre (`tableau.rs`), avec le
//! trou du coffre posé en jaune ; le panneau y renvoie.
//!
//! **Commandes**, vérifiées sur Minecraft Java 26.3 (Paper). `N` est le numéro du son dans le
//! pack de ressources (`pack.rs`) :
//!
//! ```text
//! data modify storage mine_de_rien:dictee son set value "mine_de_rien:dictee.N"
//! data modify storage mine_de_rien:dictee phrase set value "Dans la _____ de Dubaï…"
//! execute as <pseudo> at @s run function mine_de_rien:dictee/jouer {son:"mine_de_rien:dictee.N"}
//! give <pseudo> minecraft:warped_fungus_on_a_stick[minecraft:item_name="Écouteur",
//!     minecraft:item_model="minecraft:note_block",minecraft:lore=[...],
//!     minecraft:custom_data={mdr_ecouteur:1b}] 1
//! data remove storage mine_de_rien:dictee son
//! data remove storage mine_de_rien:dictee phrase
//! ```
//!
//! **L'Écouteur.** Un objet à clic droit, donné avec la clé. Le datapack compte ses clics
//! (objectif `mdr_ecoute`) et, à chaque tick, relit le son et réaffiche la phrase à trou
//! écrits dans le stockage (`function/dictee/redire.mcfunction`) : l'enfant n'attend pas le
//! tour de 3 secondes de l'agent. Sans coffre de dictée actif, le stockage est vide et
//! l'Écouteur ne fait rien. À vérifier avec un joueur : le clic droit lui-même.
//!
//! **Ce que l'enfant ne voit jamais** : le mot, ni une graphie fausse, même la sienne. La
//! phrase à trou porte toujours cinq tirets. Le pack de sons ne contient que des numéros.
//!
//! **La clé.** Sa description disait « en minuscules » : faux pour « Dans » ou « Dubaï ».
//! Devant un coffre de dictée, une clé à l'ancienne description est remplacée, et c'est le
//! chat qui dit comment écrire le mot. Ni la casse ni les espaces ne comptent : quand l'agent
//! lit sur la clé un nom qui vaut la réponse, le verrou prend ce nom-là (`orchestrateur.rs`).
//!
//! **Le pack d'abord.** Le jeu ne charge le pack de sons qu'à la connexion. Un coffre de
//! dictée n'est donc posé que si l'enfant s'est connecté après la fabrication du pack ; sinon
//! il attend, et parents et enfant sont prévenus une fois (`orchestrateur.rs`).
//!
//! **Contournements**, en plus de ceux du coffre du soir.
//! - *Lire le mot sur la feuille de la maîtresse* : c'est de la préparation de dictée ; le
//!   parent garde la feuille s'il le souhaite.
//! - *Ouvrir le pack de sons* sur le disque du jeu : des fichiers numérotés, aucun texte.
//! - *Couper le son* (curseur « Voix » à zéro, ou pack refusé) : le coffre devient muet, pas
//!   plus facile.
//! - *Demander le mot à Pixel* : devant un coffre de dictée, Pixel n'interroge aucun LLM. Ses
//!   indices sont fixes (lecture lente, nombre de lettres, première lettre) et il ne bavarde
//!   pas : un modèle pourrait épeler le mot, ou pire, mal l'écrire.
//! - *Essayer au hasard* : chaque renommage coûte toujours un niveau.

use serde_json::{Value, json};

use crate::exercises::base::Exercice;
use crate::exercises::francais::dictee::COMPETENCE;
use crate::minecraft::texte::{self, LONGUEUR_MAX, PseudoInvalide};
use crate::pack;
use crate::util::json_python;

pub const STOCKAGE: &str = "mine_de_rien:dictee";
pub const ECOUTEUR: &str = "minecraft:warped_fungus_on_a_stick";
pub const NOM_ECOUTEUR: &str = "Écouteur";
const APPARENCE_ECOUTEUR: &str = "minecraft:note_block";
pub const MODE_EMPLOI_ECOUTEUR: [&str; 2] =
    ["Tiens-moi en main", "Clic droit : je relis la phrase"];

pub const ANNONCE: &str = "Un nouveau coffre de dictée est apparu à l'atelier !";
pub const RAPPEL: &str = "Un coffre de dictée t'attend à l'atelier.";
pub const MODE_EMPLOI: &str = "Pour l'ouvrir : pose ta clé à gauche sur l'enclume, remplace son \
    nom par le mot qui manque, \
    clique sur la clé renommée à droite pour la prendre, puis ouvre le coffre en la tenant en \
    main.";
/// Sur le panneau : le texte de la dictée est sur le tableau, derrière le coffre.
pub const PANNEAU: &str = "Dictée : lis le texte derrière le coffre";
pub const REECOUTER: &str =
    "Pour réécouter la phrase : prends l'Écouteur en main et fais un clic droit.";
pub const RECONNEXION: &str = "Une dictée t'attend, mais ton jeu n'a pas encore ses sons : \
    déconnecte-toi puis reconnecte-toi au serveur.";

pub fn est_dictee(ex: &Exercice) -> bool {
    ex.competence == COMPETENCE
}

fn numero(ex: &Exercice, cle: &str) -> Option<u32> {
    ex.donnees
        .get(cle)
        .and_then(Value::as_u64)
        .and_then(|n| u32::try_from(n).ok())
}

/// Le numéro du son de la phrase, à vitesse normale.
pub fn son(ex: &Exercice) -> Option<u32> {
    numero(ex, "son")
}

/// Le numéro du son de la phrase relue lentement.
pub fn son_lent(ex: &Exercice) -> Option<u32> {
    numero(ex, "son_lent")
}

/// La phrase avec `_____` à la place du mot.
pub fn phrase_a_trou(ex: &Exercice) -> Option<&str> {
    ex.donnees.get("a_trou").and_then(Value::as_str)
}

fn snbt(chaine: &str) -> String {
    json_python(&Value::String(texte::nettoyer(chaine, LONGUEUR_MAX)))
}

/// À la pose : ce que l'Écouteur relira. Vide si l'exercice n'a ni son ni phrase.
pub fn commandes_stockage(ex: &Exercice) -> Vec<String> {
    let mut commandes = Vec::new();
    if let Some(n) = son(ex) {
        commandes.push(format!(
            "data modify storage {STOCKAGE} son set value {}",
            snbt(&pack::evenement(n))
        ));
    }
    if let Some(phrase) = phrase_a_trou(ex) {
        commandes.push(format!(
            "data modify storage {STOCKAGE} phrase set value {}",
            snbt(phrase)
        ));
    }
    commandes
}

/// À l'ouverture ou au retrait : l'Écouteur redevient muet.
pub fn commandes_effacement() -> Vec<String> {
    vec![
        format!("data remove storage {STOCKAGE} son"),
        format!("data remove storage {STOCKAGE} phrase"),
    ]
}

/// Lit le son n°`numero` à l'enfant, à la place de celui en cours.
pub fn commande_lecture(pseudo: &str, numero: u32) -> Result<String, PseudoInvalide> {
    Ok(format!(
        "execute as {0} at @s run function mine_de_rien:dictee/jouer {{son:{1}}}",
        texte::verifier_pseudo(pseudo)?,
        snbt(&pack::evenement(numero))
    ))
}

pub fn predicat_ecouteur() -> String {
    format!("{ECOUTEUR}[minecraft:custom_data={{mdr_ecouteur:1b}}]")
}

/// `give` d'un Écouteur, avec son mode d'emploi dans sa description.
pub fn commande_ecouteur(pseudo: &str) -> Result<String, PseudoInvalide> {
    let lore: Vec<String> = MODE_EMPLOI_ECOUTEUR
        .iter()
        .map(|ligne| json_python(&json!({"text": ligne, "italic": false, "color": "gray"})))
        .collect();
    Ok(format!(
        "give {} {ECOUTEUR}[minecraft:item_name={},minecraft:item_model={},minecraft:lore=[{}],minecraft:custom_data={{mdr_ecouteur:1b}}] 1",
        texte::verifier_pseudo(pseudo)?,
        snbt(NOM_ECOUTEUR),
        snbt(APPARENCE_ECOUTEUR),
        lore.join(",")
    ))
}

/// Ce que Pixel dit avant de relire lentement, et ce qu'il dit si le son manque.
pub const SANS_SON: &str = "Je n'arrive pas à relire cette phrase. Demande à un parent.";
/// Devant un coffre de dictée, Pixel ne bavarde pas : il n'a pas le droit d'épeler.
pub const PAS_DE_BAVARDAGE: &str = "Pendant une dictée, je peux te relire la phrase ou te \
    donner un indice : dis-moi « aide-moi ». Le reste, on en parle après !";
