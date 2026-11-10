mod commun;

use chrono::{DateTime, TimeDelta, Utc};
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::games::coffre_du_soir::formater_duree;
use mine_de_rien::rapport::{a_retravailler, nombre_indices, rapport};
use mine_de_rien::saisie::Source;
use mine_de_rien::storage::sqlite::{Demande, Depot};
use serde_json::json;

fn maintenant() -> DateTime<Utc> {
    commun::instant("2026-11-14T18:00:00+00:00")
}

fn depot() -> (tempfile::TempDir, Depot) {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap();
    (dossier, depot)
}

fn ajouter(
    depot: &Depot,
    competence: &str,
    enonce: &str,
    pose_il_y_a: TimeDelta,
    ouvert_apres: Option<TimeDelta>,
    annule: bool,
) -> i64 {
    let exercice = Exercice {
        competence: competence.to_owned(),
        enonce: enonce.to_owned(),
        reponse: "1".to_owned(),
        donnees: Default::default(),
    };
    let demande = Demande {
        source: Source::Competence,
        demande_par: "111",
        recompense: "minecraft:diamond",
        recompense_nombre: 3,
    };
    let pose = maintenant() - pose_il_y_a;
    let ident = depot
        .ajouter(&exercice, demande, pose - TimeDelta::minutes(1))
        .unwrap();
    depot.marquer_pose(ident, pose).unwrap();
    if let Some(apres) = ouvert_apres {
        depot.marquer_ouvert(ident, pose + apres).unwrap();
    }
    if annule {
        depot.annuler(ident).unwrap();
    }
    ident
}

fn a_retravailler_dans(texte: &str) -> Vec<&str> {
    texte
        .split("À retravailler :\n")
        .nth(1)
        .unwrap()
        .split("\n\n")
        .next()
        .unwrap()
        .lines()
        .collect()
}

fn minutes(n: i64) -> Option<TimeDelta> {
    Some(TimeDelta::minutes(n))
}

#[test]
fn rapport_classe_les_exercices_a_retravailler() {
    let (_dossier, depot) = depot();
    let division = "maths/division-posee";
    // Un seul coffre posé à la fois : on les crée dans l'ordre de leur vie.
    ajouter(
        &depot,
        division,
        "ancien",
        TimeDelta::days(9),
        minutes(90),
        false,
    );
    ajouter(
        &depot,
        division,
        "72 ÷ 6 = ?",
        TimeDelta::days(4),
        minutes(10),
        false,
    );
    let lent = ajouter(
        &depot,
        division,
        "84 ÷ 7 = ?",
        TimeDelta::days(3),
        minutes(45),
        false,
    );
    ajouter(
        &depot,
        division,
        "96 ÷ 3 = ?",
        TimeDelta::days(2),
        minutes(4),
        false,
    );
    ajouter(
        &depot,
        "maths/addition-posee",
        "annulé",
        TimeDelta::days(2),
        None,
        true,
    );
    let ferme = ajouter(
        &depot,
        "maths/addition-posee",
        "3,5 + 1,25 = ?",
        TimeDelta::hours(50),
        None,
        false,
    );
    depot
        .noter(
            "coffre_disparu",
            Some(ferme),
            maintenant() - TimeDelta::days(1),
        )
        .unwrap();

    let texte = rapport(&depot, maintenant()).unwrap();
    assert!(texte.contains("maths/division-posee : 3 posé(s), 3 ouvert(s), durée médiane 10 min"));
    assert!(texte.contains("maths/addition-posee : 1 posé(s), 0 ouvert(s)"));
    assert_eq!(
        a_retravailler_dans(&texte),
        [
            format!("• n°{lent} 84 ÷ 7 = ? : ouvert en 45 min"),
            format!("• n°{ferme} 3,5 + 1,25 = ? : toujours fermé après 2 jours"),
        ]
    );
    assert!(!texte.contains("ancien") && !texte.contains("annulé"));
    assert!(texte.contains("Coffres disparus puis reposés : 1"));
}

#[test]
fn pas_encore_a_retravailler_avant_48_heures() {
    let (_dossier, depot) = depot();
    let ident = ajouter(
        &depot,
        "maths/addition-posee",
        "1 + 1 = ?",
        TimeDelta::hours(47),
        None,
        false,
    );
    assert!(a_retravailler(&depot.lire(ident).unwrap().unwrap(), maintenant(), 0).is_none());
    assert!(
        !rapport(&depot, maintenant())
            .unwrap()
            .contains("À retravailler")
    );
}

#[test]
fn rapport_vide() {
    let (_dossier, depot) = depot();
    assert!(
        rapport(&depot, maintenant())
            .unwrap()
            .contains("Aucun coffre posé.")
    );
}

fn indices(depot: &Depot, ident: i64, n: usize) {
    for _ in 0..n {
        depot
            .noter("indice", Some(ident), maintenant() - TimeDelta::days(1))
            .unwrap();
    }
}

#[test]
fn rapport_compte_les_indices_et_retravaille_a_trois() {
    let (_dossier, depot) = depot();
    let doubles = "maths/doubles-moities";
    let deux = ajouter(
        &depot,
        doubles,
        "Double de 17 = ?",
        TimeDelta::days(4),
        minutes(5),
        false,
    );
    indices(&depot, deux, 2);
    let trois = ajouter(
        &depot,
        doubles,
        "Moitié de 75 = ?",
        TimeDelta::days(3),
        minutes(5),
        false,
    );
    indices(&depot, trois, 3);
    let anglais = "anglais/vocabulaire";
    let lent = ajouter(
        &depot,
        anglais,
        "August en français ?",
        TimeDelta::days(2),
        minutes(40),
        false,
    );
    indices(&depot, lent, 4);
    let annule = ajouter(&depot, anglais, "annulé", TimeDelta::days(1), None, true);
    indices(&depot, annule, 5);

    let texte = rapport(&depot, maintenant()).unwrap();
    assert!(texte.contains(
        "maths/doubles-moities : 2 posé(s), 2 ouvert(s), durée médiane 5 min, 5 indices"
    ));
    assert!(
        texte.contains(
            "anglais/vocabulaire : 1 posé(s), 1 ouvert(s), durée médiane 40 min, 4 indices"
        )
    );
    assert_eq!(
        a_retravailler_dans(&texte),
        [
            format!("• n°{trois} Moitié de 75 = ? : ouvert avec 3 indices"),
            format!("• n°{lent} August en français ? : ouvert en 40 min avec 4 indices"),
        ]
    );
}

/// Sur les bases écrites par Python, à plusieurs dates : le même texte de rapport, les mêmes
/// raisons de retravailler, les mêmes durées.
#[test]
fn oracle_python() {
    for cas in commun::oracle("rapport") {
        let e = &cas["entree"];
        let ouvrir = || {
            let (dossier, copie) = commun::copie_base(e["base"].as_str().unwrap());
            (dossier, Depot::ouvrir(&copie).unwrap())
        };
        let quand = || commun::instant(e["quand"].as_str().unwrap());
        let sortie = match e["f"].as_str().unwrap() {
            "rapport" => json!(rapport(&ouvrir().1, quand()).unwrap()),
            "a_retravailler" => {
                let (_dossier, depot) = ouvrir();
                let s = depot.lire(e["id"].as_i64().unwrap()).unwrap().unwrap();
                json!(a_retravailler(
                    &s,
                    quand(),
                    e["indices"].as_u64().unwrap() as usize
                ))
            }
            "formater_duree" => {
                json!(formater_duree(
                    e["secondes"].as_i64().map(TimeDelta::seconds)
                ))
            }
            "nombre_indices" => json!(nombre_indices(e["n"].as_u64().unwrap() as usize)),
            autre => panic!("fonction inconnue : {autre}"),
        };
        assert_eq!(sortie, cas["sortie"], "{e}");
    }
}
