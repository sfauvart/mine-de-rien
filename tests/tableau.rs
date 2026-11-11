//! Le tableau de la dictée : le texte entier, ses trous, et les commandes qui l'affichent.

mod commun;

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use mine_de_rien::adapters::game::rcon::TAILLE_MAX_COMMANDE;
use mine_de_rien::alea::Alea;
use mine_de_rien::dictees::{seance, tableau};
use mine_de_rien::exercises::francais::dictee::{Position, decouper};
use mine_de_rien::games::atelier::Atelier;
use mine_de_rien::games::tableau::{
    Genre, Morceau, Trou, commande_effacement, commandes, morceaux,
};
use mine_de_rien::saisie::Source;
use mine_de_rien::storage::sqlite::{Demande, Depot, PhraseSonore};

const TEXTE: &str = "La plus haute tour s'élève. Elle est plus haute que trois tours Eiffel.";

const DEMANDE: Demande<'static> = Demande {
    source: Source::Dictee,
    demande_par: "seance",
    recompense: "minecraft:diamond",
    recompense_nombre: 3,
};

fn p(phrase: usize, mot: usize) -> Position {
    Position { phrase, mot }
}

/// Le tableau tel qu'on le lirait : `[?]` le trou jaune, `[…]` un trou gris, `[mot]` en vert.
fn lu(morceaux: &[Morceau]) -> String {
    morceaux
        .iter()
        .map(|m| match m.genre {
            Genre::Texte => m.texte.clone(),
            Genre::TrouActif => "[?]".to_owned(),
            Genre::TrouAttente => "[…]".to_owned(),
            Genre::Trouve => format!("[{}]", m.texte),
        })
        .collect()
}

#[test]
fn sans_trou_le_texte_est_entier() {
    let phrases = decouper(TEXTE);
    let tout = morceaux(&phrases, &HashMap::new());
    assert_eq!(lu(&tout), TEXTE);
    assert_eq!(tout.len(), 1, "un seul morceau de texte ordinaire");
}

#[test]
fn le_trou_actif_les_trous_en_attente_et_les_mots_trouves() {
    let phrases = decouper(TEXTE);
    // « tour » à trouver, « Eiffel » en attente, « s'élève » trouvé.
    let trous = HashMap::from([
        (p(0, 3), Trou::Actif),
        (p(1, 7), Trou::Attente),
        (p(0, 4), Trou::Trouve),
    ]);
    assert_eq!(
        lu(&morceaux(&phrases, &trous)),
        "La plus haute [?] [s'élève]. Elle est plus haute que trois tours […]."
    );
}

#[test]
fn un_mot_a_trouver_est_masque_partout_ou_il_apparait() {
    let phrases = decouper(TEXTE);
    // « plus » de la première phrase : celui de la seconde le donnerait, majuscule ou non.
    let trous = HashMap::from([(p(0, 1), Trou::Actif)]);
    assert_eq!(
        lu(&morceaux(&phrases, &trous)),
        "La [?] haute tour s'élève. Elle est […] haute que trois tours Eiffel."
    );
    // Trouvé à un endroit, encore à trouver à un autre : il reste caché aux deux.
    let trous = HashMap::from([(p(0, 1), Trou::Trouve), (p(1, 2), Trou::Attente)]);
    assert!(!lu(&morceaux(&phrases, &trous)).contains("plus"));
}

#[test]
fn les_commandes_assemblent_le_texte_puis_refont_l_entite() {
    let phrases = decouper(TEXTE);
    let trous = HashMap::from([(p(0, 3), Trou::Actif)]);
    let liste = commandes(&Atelier::new(-3, 64, 8), &morceaux(&phrases, &trous));
    assert_eq!(
        liste,
        [
            r#"data modify storage mine_de_rien:dictee tableau set value [{text:""}]"#,
            r#"data modify storage mine_de_rien:dictee tableau append value {"text": "La plus haute ", "color": "white"}"#,
            r#"data modify storage mine_de_rien:dictee tableau append value {"text": "_____", "color": "yellow", "bold": true}"#,
            r#"data modify storage mine_de_rien:dictee tableau append value {"text": " s'élève. Elle est plus haute que trois tours Eiffel.", "color": "white"}"#,
            "kill @e[type=minecraft:text_display,tag=mdr_tableau]",
            r#"summon minecraft:text_display -2.5 65.2 6.5 {Tags:["mdr_tableau"],line_width:220,alignment:"left",brightness:{sky:15,block:15},text:{text:""}}"#,
            "data modify entity @e[type=minecraft:text_display,tag=mdr_tableau,limit=1] text set from storage mine_de_rien:dictee tableau",
        ]
    );
    assert_eq!(liste[4], commande_effacement());
}

#[test]
fn un_long_texte_tient_dans_les_commandes_de_rcon() {
    // Quarante phrases pleines d'accents et de guillemets : rien ne dépasse la limite.
    let texte = "L'élève « très appliqué » a écrit çà et là, où l'été fut brûlant. ".repeat(40);
    let phrases = decouper(&texte);
    let liste = commandes(
        &Atelier::new(0, 64, 0),
        &morceaux(&phrases, &HashMap::new()),
    );
    assert!(liste.len() > 10);
    for commande in &liste {
        assert!(commande.len() <= TAILLE_MAX_COMMANDE, "{}", commande.len());
    }
    // Aucun caractère perdu au découpage.
    let recolle: String = liste
        .iter()
        .filter_map(|c| c.split_once(r#"append value {"text": ""#))
        .filter_map(|(_, suite)| suite.split_once(r#"", "color""#))
        .map(|(texte, _)| texte.replace("\\\"", "\""))
        .collect();
    assert_eq!(recolle, texte.trim());
}

fn instant(heure: u32) -> DateTime<Utc> {
    commun::instant(&format!("2026-11-10T{heure:02}:00:00+00:00"))
}

#[test]
fn le_tableau_suit_la_seance_dans_la_base() {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap();
    let phrases: Vec<PhraseSonore> = decouper(TEXTE)
        .into_iter()
        .map(|p| PhraseSonore {
            texte: p.texte,
            son: b"OggS".to_vec(),
            son_lent: b"OggS".to_vec(),
        })
        .collect();
    let jour = NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
    let ident = depot
        .ajouter_dictee(jour, TEXTE, &phrases, "111", instant(8))
        .unwrap();
    let dictee = depot.lire_dictee(ident).unwrap().unwrap();
    let coffres = seance(
        &depot,
        &dictee,
        3,
        &mut Alea::graine(7),
        &HashMap::new(),
        DEMANDE,
        instant(17),
    )
    .unwrap();
    let numeros: Vec<i64> = coffres.iter().map(|(numero, _)| *numero).collect();
    let mots: Vec<&str> = coffres.iter().map(|(_, ex)| ex.reponse.as_str()).collect();
    let lire = |numero: i64| {
        let reference = depot.lire(numero).unwrap().unwrap();
        tableau(&depot, &reference).unwrap().unwrap()
    };
    let compte =
        |morceaux: &[Morceau], genre: Genre| morceaux.iter().filter(|m| m.genre == genre).count();

    // Premier coffre posé : un trou jaune, les deux autres mots sont cachés eux aussi.
    depot.marquer_pose(numeros[0], instant(17)).unwrap();
    let t = lire(numeros[0]);
    assert_eq!(compte(&t, Genre::TrouActif), 1);
    assert!(compte(&t, Genre::TrouAttente) >= 2);
    assert_eq!(compte(&t, Genre::Trouve), 0);
    for mot in &mots {
        assert!(
            !t.iter()
                .any(|m| m.texte.to_lowercase() == mot.to_lowercase()),
            "{mot} est lisible : {}",
            lu(&t)
        );
    }

    // Ouvert : son mot s'écrit en vert, plus aucun trou jaune avant la pose suivante.
    depot
        .marquer_ouvert(numeros[0], instant(17) + TimeDelta::minutes(2))
        .unwrap();
    let t = lire(numeros[0]);
    assert_eq!(compte(&t, Genre::TrouActif), 0);
    // Sauf si le même mot est encore à trouver ailleurs, il est écrit.
    let encore_cache = mots[1..]
        .iter()
        .any(|m| m.to_lowercase() == mots[0].to_lowercase());
    assert_eq!(compte(&t, Genre::Trouve), usize::from(!encore_cache));

    // Toute la séance ouverte : le texte entier, sans trou.
    for numero in &numeros[1..] {
        depot.marquer_pose(*numero, instant(17)).unwrap();
        depot
            .marquer_ouvert(*numero, instant(17) + TimeDelta::minutes(5))
            .unwrap();
    }
    let t = lire(numeros[2]);
    assert_eq!(
        t.iter().map(|m| m.texte.as_str()).collect::<String>(),
        TEXTE
    );
    assert_eq!(compte(&t, Genre::Trouve), 3);

    // Une séance du lendemain ne garde pas en vert les mots de la veille.
    let demain = seance(
        &depot,
        &dictee,
        1,
        &mut Alea::graine(8),
        &HashMap::new(),
        DEMANDE,
        instant(17) + TimeDelta::days(1),
    )
    .unwrap();
    depot
        .marquer_pose(demain[0].0, instant(17) + TimeDelta::days(1))
        .unwrap();
    let t = lire(demain[0].0);
    assert_eq!(compte(&t, Genre::TrouActif), 1);
    assert_eq!(compte(&t, Genre::Trouve), 0);
}

#[test]
fn un_coffre_qui_n_est_pas_d_une_dictee_enregistree_n_a_pas_de_tableau() {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap();
    let phrases = decouper(TEXTE);
    let ex = mine_de_rien::dictees::exercice_sonore(&phrases, p(0, 1), 1).unwrap();
    let numero = depot.ajouter(&ex, DEMANDE, instant(17)).unwrap();
    let reference = depot.lire(numero).unwrap().unwrap();
    assert!(tableau(&depot, &reference).unwrap().is_none());
}
