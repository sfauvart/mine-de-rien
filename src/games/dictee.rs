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
//! **La phrase s'entend par le chat vocal** (Simple Voice Chat), tout de suite, sans que le
//! jeu se recharge : à la pose, au rappel de connexion, à l'indice lent de Pixel et à chaque
//! clic droit de l'Écouteur. L'agent garde les deux sons de la phrase (normal, lent) dans le
//! dossier partagé avec le serveur (`dictees::Lecteur`, `diffusion.rs`) tant que le coffre
//! attend, et les efface à l'ouverture.
//!
//! **Commandes**, vérifiées sur Minecraft Java 26.3 (Paper) avec le plugin
//! `mine-de-rien-voix`. `N` est le numéro du son :
//!
//! ```text
//! mdrvoix etat <pseudo>                          relié, sans chat vocal ou absent
//! mdrvoix phrase <pseudo> phrase-N.pcm           ce que l'Écouteur rejouera
//! mdrvoix jouer <pseudo> phrase-N.pcm            la phrase, maintenant
//! data modify storage mine_de_rien:dictee phrase set value "Dans la _____ de Dubaï…"
//! give <pseudo> minecraft:warped_fungus_on_a_stick[minecraft:item_name="Écouteur",
//!     minecraft:item_model="minecraft:note_block",minecraft:lore=[...],
//!     minecraft:custom_data={mdr_ecouteur:1b}] 1
//! mdrvoix phrase <pseudo>                        à l'ouverture : l'Écouteur redevient muet
//! data remove storage mine_de_rien:dictee phrase
//! ```
//!
//! **L'Écouteur.** Un objet à clic droit, donné avec la clé. Deux choses répondent au clic,
//! sans attendre le tour de 3 secondes de l'agent : le plugin coupe ce qui joue et rejoue la
//! phrase désignée ; le datapack compte les clics (objectif `mdr_ecoute`) et réaffiche la
//! phrase à trou écrite dans le stockage (`function/dictee/redire.mcfunction`). Sans coffre de
//! dictée actif, rien n'est désigné, le stockage est vide et l'Écouteur ne fait rien.
//!
//! **Ce que l'enfant ne voit jamais** : le mot, ni une graphie fausse, même la sienne. La
//! phrase à trou porte toujours cinq tirets. Les sons ne portent que des numéros.
//!
//! **La clé.** Sa description disait « en minuscules » : faux pour « Dans » ou « Dubaï ».
//! Devant un coffre de dictée, une clé à l'ancienne description est remplacée, et c'est le
//! chat qui dit comment écrire le mot. Ni la casse ni les espaces ne comptent : quand l'agent
//! lit sur la clé un nom qui vaut la réponse, le verrou prend ce nom-là (`orchestrateur.rs`).
//!
//! **Le chat vocal d'abord.** Un coffre de dictée n'est posé que si l'enfant est là et son
//! jeu relié au chat vocal (`mdrvoix etat`). Sinon le coffre attend, les coffres de calcul
//! passent devant, l'enfant lit qu'il faut demander à un parent, et les parents sont prévenus
//! une fois par jour (`orchestrateur.rs`). Le lien se fait quelques secondes après la
//! connexion : l'agent attend ce délai avant de conclure.
//!
//! **Contournements**, en plus de ceux du coffre du soir.
//! - *Lire le mot sur la feuille de la maîtresse* : c'est de la préparation de dictée ; le
//!   parent garde la feuille s'il le souhaite.
//! - *Lire le dossier des sons* sur le disque du serveur : des fichiers numérotés, du son
//!   brut, aucun texte.
//! - *Couper le son* (volume « Mine de rien » à zéro dans le chat vocal) : le coffre devient
//!   muet, pas plus facile.
//! - *Faire jouer une autre phrase* : la commande du plugin est refusée à tout joueur.
//! - *Demander le mot à Pixel* : devant un coffre de dictée, Pixel n'interroge aucun LLM. Ses
//!   indices sont fixes (lecture lente, nombre de lettres, première lettre) et il ne bavarde
//!   pas : un modèle pourrait épeler le mot, ou pire, mal l'écrire.
//! - *Essayer au hasard* : chaque renommage coûte toujours un niveau.

use serde_json::{Value, json};

use crate::exercises::base::Exercice;
use crate::exercises::francais::dictee::COMPETENCE;
use crate::minecraft::texte::{self, LONGUEUR_MAX, PseudoInvalide};
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
/// Le jeu de l'enfant n'est pas relié au chat vocal : la dictée ne peut pas être lue.
pub const SANS_CHAT_VOCAL: &str = "Une dictée t'attend, mais ton jeu ne peut pas te la lire. \
    Demande à un parent d'installer le chat vocal.";

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

/// À la pose : la phrase à trou que l'Écouteur réaffichera. Vide si l'exercice n'en a pas.
pub fn commandes_stockage(ex: &Exercice) -> Vec<String> {
    phrase_a_trou(ex)
        .map(|phrase| {
            format!(
                "data modify storage {STOCKAGE} phrase set value {}",
                snbt(phrase)
            )
        })
        .into_iter()
        .collect()
}

/// À l'ouverture ou au retrait : l'Écouteur n'affiche plus rien. `son` date d'avant le chat
/// vocal : il est retiré d'un monde qui le porterait encore.
pub fn commandes_effacement() -> Vec<String> {
    vec![
        format!("data remove storage {STOCKAGE} son"),
        format!("data remove storage {STOCKAGE} phrase"),
    ]
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
