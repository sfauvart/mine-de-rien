mod commun;

use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::saisie::Source;
use mine_de_rien::storage::sqlite::{
    Demande, Depot, ExerciceStocke, MIGRATIONS, SCHEMA, Statut, VERSION_SCHEMA,
};
use rusqlite::Connection;
use serde_json::{Value, json};

fn t0() -> DateTime<Utc> {
    commun::instant("2026-11-07T18:00:00+00:00")
}

fn ex(n: i64) -> Exercice {
    Exercice {
        competence: "maths/division-posee".to_owned(),
        enonce: format!("{} ÷ 12 = ?", n * 12),
        reponse: n.to_string(),
        donnees: json!({"n": n}).as_object().cloned().unwrap(),
    }
}

fn depot() -> (tempfile::TempDir, Depot) {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap();
    (dossier, depot)
}

const DEMANDE: Demande<'static> = Demande {
    source: Source::Expression,
    demande_par: "42",
    recompense: "diamond",
    recompense_nombre: 3,
};

fn ajouter(depot: &Depot, n: i64, quand: DateTime<Utc>) -> i64 {
    depot.ajouter(&ex(n), DEMANDE, quand).unwrap()
}

fn version(connexion: &Connection) -> u32 {
    connexion
        .query_row("PRAGMA user_version", [], |l| l.get(0))
        .unwrap()
}

fn ids(liste: &[ExerciceStocke]) -> Vec<i64> {
    liste.iter().map(|s| s.id).collect()
}

#[test]
fn schema_date() {
    let (_dossier, depot) = depot();
    assert_eq!(version(depot.connexion()), VERSION_SCHEMA);
}

#[test]
fn file_dans_l_ordre_et_sans_les_annules() {
    let (_dossier, depot) = depot();
    let numeros: Vec<i64> = (1..=4).map(|n| ajouter(&depot, n, t0())).collect();
    depot.annuler(numeros[1]).unwrap();
    assert_eq!(
        ids(&depot.en_file().unwrap()),
        [numeros[0], numeros[2], numeros[3]]
    );
    assert_eq!(depot.suivant().unwrap().unwrap().id, numeros[0]);
}

#[test]
fn cycle_de_vie() {
    let (_dossier, depot) = depot();
    let (a, b) = (ajouter(&depot, 1, t0()), ajouter(&depot, 2, t0()));
    depot.marquer_pose(a, t0() + TimeDelta::minutes(1)).unwrap();
    assert_eq!(depot.actif().unwrap().unwrap().id, a);
    assert_eq!(depot.suivant().unwrap().unwrap().id, b);
    let erreur = depot.marquer_pose(b, t0()).unwrap_err();
    assert!(erreur.to_string().contains("déjà posé"));
    depot
        .marquer_ouvert(a, t0() + TimeDelta::minutes(5))
        .unwrap();
    let lu = depot.lire(a).unwrap().unwrap();
    assert_eq!(lu.statut, Statut::Ouvert);
    assert_eq!(lu.duree(), Some(TimeDelta::minutes(4)));
    assert_eq!(lu.exercice, ex(1));
    assert!(depot.actif().unwrap().is_none());
}

#[test]
fn annuler() {
    let (_dossier, depot) = depot();
    let a = ajouter(&depot, 1, t0());
    depot.marquer_pose(a, t0()).unwrap();
    assert_eq!(depot.annuler(a).unwrap().unwrap().statut, Statut::Pose);
    assert!(depot.annuler(a).unwrap().is_none());
    assert!(depot.annuler(999).unwrap().is_none());
    assert!(depot.actif().unwrap().is_none());
}

#[test]
fn ouvrir_un_exercice_non_pose_echoue() {
    let (_dossier, depot) = depot();
    let a = ajouter(&depot, 1, t0());
    assert!(depot.marquer_ouvert(a, t0()).is_err());
}

#[test]
fn entre() {
    let (_dossier, depot) = depot();
    ajouter(&depot, 1, t0() - TimeDelta::days(8));
    let b = ajouter(&depot, 2, t0() - TimeDelta::days(1));
    assert_eq!(
        ids(&depot.entre(t0() - TimeDelta::days(7), t0()).unwrap()),
        [b]
    );
}

#[test]
fn ancienne_base_sans_version_refusee() {
    let dossier = tempfile::tempdir().unwrap();
    let chemin = dossier.path().join("vieille.sqlite");
    let cx = Connection::open(&chemin).unwrap();
    cx.execute("CREATE TABLE exercices (id INTEGER PRIMARY KEY)", [])
        .unwrap();
    drop(cx);
    let erreur = Depot::ouvrir(&chemin).err().unwrap();
    assert!(erreur.to_string().contains("antérieure"));
}

#[test]
fn base_d_une_version_plus_recente_refusee() {
    let dossier = tempfile::tempdir().unwrap();
    let chemin = dossier.path().join("future.sqlite");
    let cx = Connection::open(&chemin).unwrap();
    cx.pragma_update(None, "user_version", VERSION_SCHEMA + 1)
        .unwrap();
    drop(cx);
    assert!(
        Depot::ouvrir(&chemin)
            .err()
            .unwrap()
            .to_string()
            .contains("plus récente")
    );
}

#[test]
fn reouverture() {
    let dossier = tempfile::tempdir().unwrap();
    let chemin = dossier.path().join("mdr.sqlite");
    let d = Depot::ouvrir(&chemin).unwrap();
    ajouter(&d, 1, t0());
    drop(d);
    assert_eq!(Depot::ouvrir(&chemin).unwrap().en_file().unwrap().len(), 1);
}

#[test]
fn migration_2_vers_3_garde_les_exercices_et_admet_la_photo() {
    let dossier = tempfile::tempdir().unwrap();
    let chemin = dossier.path().join("v2.sqlite");
    let cx = Connection::open(&chemin).unwrap();
    cx.execute_batch(SCHEMA).unwrap();
    cx.execute_batch(MIGRATIONS[0]).unwrap();
    cx.execute(
        "INSERT INTO exercices (competence, enonce, reponse, jeu, source, statut, demande_par,
             recompense, recompense_nombre, cree_le, niveaux_donnes)
         VALUES ('maths/calcul', '1 + 1 = ?', '2', 'coffre-du-soir', 'expression', 'ouvert',
                 'cli', 'minecraft:diamond', 3, '2026-11-01T18:00:00+00:00', 1)",
        [],
    )
    .unwrap();
    cx.execute(
        "INSERT INTO evenements (exercice_id, genre, quand) VALUES (1, 'x', '2026-11-01T18:00:00+00:00')",
        [],
    )
    .unwrap();
    cx.pragma_update(None, "user_version", 2).unwrap();
    drop(cx);

    let depot = Depot::ouvrir(&chemin).unwrap();
    assert_eq!(version(depot.connexion()), 3);
    let ancien = depot.lire(1).unwrap().unwrap();
    assert!(ancien.statut == Statut::Ouvert && ancien.niveaux_donnes);
    assert_eq!(ancien.exercice.reponse, "2");
    let orphelines: Vec<String> = depot
        .connexion()
        .prepare("PRAGMA foreign_key_check")
        .unwrap()
        .query_map([], |l| l.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert!(orphelines.is_empty());
    let cles: i64 = depot
        .connexion()
        .query_row("PRAGMA foreign_keys", [], |l| l.get(0))
        .unwrap();
    assert_eq!(cles, 1);
    let mai = Exercice {
        competence: "anglais/vocabulaire".to_owned(),
        enonce: "mai en anglais ?".to_owned(),
        reponse: "may".to_owned(),
        donnees: Default::default(),
    };
    let demande = Demande {
        source: Source::Photo,
        demande_par: "111",
        recompense: "minecraft:diamond",
        recompense_nombre: 3,
    };
    let numero = depot.ajouter(&mai, demande, t0()).unwrap();
    assert_eq!(depot.lire(numero).unwrap().unwrap().source, Source::Photo);
}

#[test]
fn les_dates_gardent_l_ecriture_de_python() {
    let (_dossier, depot) = depot();
    let a = ajouter(&depot, 1, t0() + TimeDelta::milliseconds(750));
    let ecrite: String = depot
        .connexion()
        .query_row("SELECT cree_le FROM exercices WHERE id = ?", [a], |l| {
            l.get(0)
        })
        .unwrap();
    assert_eq!(ecrite, "2026-11-07T18:00:00+00:00");
    let donnees: String = depot
        .connexion()
        .query_row("SELECT donnees FROM exercices WHERE id = ?", [a], |l| {
            l.get(0)
        })
        .unwrap();
    assert_eq!(donnees, "{\"n\": 1}");
}

#[test]
fn une_valeur_inconnue_dans_la_base_est_une_erreur_pas_une_panique() {
    let (_dossier, depot) = depot();
    let a = ajouter(&depot, 1, t0());
    depot
        .connexion()
        .pragma_update(None, "ignore_check_constraints", "ON")
        .unwrap();
    depot
        .connexion()
        .execute("UPDATE exercices SET statut = 'perdu' WHERE id = ?", [a])
        .unwrap();
    assert!(
        depot
            .lire(a)
            .unwrap_err()
            .to_string()
            .contains("statut inconnu")
    );
}

fn iso(quand: Option<DateTime<Utc>>) -> Value {
    json!(quand.map(|q| q.to_rfc3339_opts(SecondsFormat::Secs, false)))
}

/// L'exercice stocké, écrit comme dans les fichiers de référence.
fn stocke(s: &ExerciceStocke) -> Value {
    json!({
        "id": s.id,
        "exercice": s.exercice,
        "jeu": s.jeu,
        "source": s.source.nom(),
        "statut": s.statut.nom(),
        "demande_par": s.demande_par,
        "recompense": s.recompense,
        "recompense_nombre": s.recompense_nombre,
        "cree_le": iso(Some(s.cree_le)),
        "pose_le": iso(s.pose_le),
        "ouvert_le": iso(s.ouvert_le),
        "niveaux_donnes": s.niveaux_donnes,
        "duree": s.duree().map(|d| d.num_seconds() as f64),
    })
}

fn tout_lire(depot: &Depot) -> Value {
    let (debut, fin) = (
        commun::instant("2000-01-01T00:00:00+00:00"),
        commun::instant("2100-01-01T00:00:00+00:00"),
    );
    let maintenant = commun::instant("2026-11-14T18:00:00+00:00");
    let tous = depot.entre(debut, fin).unwrap();
    let optionnel = |s: Option<ExerciceStocke>| s.as_ref().map(stocke);
    let evenements: Vec<Value> = depot
        .evenements_entre(debut, fin)
        .unwrap()
        .iter()
        .map(|e| json!({"exercice_id": e.exercice_id, "genre": e.genre, "quand": iso(Some(e.quand))}))
        .collect();
    let indices: serde_json::Map<String, Value> = tous
        .iter()
        .map(|s| {
            (
                s.id.to_string(),
                json!(depot.compter("indice", s.id).unwrap()),
            )
        })
        .collect();
    json!({
        "exercices": tous.iter().map(stocke).collect::<Vec<_>>(),
        "en_file": ids(&depot.en_file().unwrap()),
        "suivant": optionnel(depot.suivant().unwrap()),
        "actif": optionnel(depot.actif().unwrap()),
        "dernier_ouvert": optionnel(depot.dernier_ouvert().unwrap()),
        "poses": ids(&depot.poses_entre(maintenant - TimeDelta::days(7), maintenant).unwrap()),
        "evenements": evenements,
        "indices": indices,
    })
}

/// Les trois bases écrites par Python, une par version du schéma, se rouvrent en version 3
/// et rendent exactement ce que la version Python y lit.
#[test]
fn oracle_python_les_bases_de_chaque_version_se_rouvrent() {
    for cas in commun::oracle("stockage") {
        let nom = cas["entree"]["base"].as_str().unwrap();
        let (_dossier, copie) = commun::copie_base(nom);
        let depot = Depot::ouvrir(&copie).unwrap();
        assert_eq!(version(depot.connexion()), VERSION_SCHEMA, "{nom}");
        assert_eq!(tout_lire(&depot), cas["sortie"], "{nom}");
    }
}

#[test]
fn les_bases_v1_et_v2_migrees_donnent_les_lignes_de_v3() {
    let lire = |nom: &str| {
        let (_dossier, copie) = commun::copie_base(nom);
        let depot = Depot::ouvrir(&copie).unwrap();
        depot
            .entre(
                commun::instant("2000-01-01T00:00:00+00:00"),
                commun::instant("2100-01-01T00:00:00+00:00"),
            )
            .unwrap()
    };
    let (v1, v2, v3) = (lire("v1"), lire("v2"), lire("v3"));
    assert_eq!(v2[..], v3[..v2.len()]);
    // La version 1 n'avait pas la colonne des niveaux donnés.
    let sans_niveaux = |liste: &[ExerciceStocke]| -> Vec<ExerciceStocke> {
        liste
            .iter()
            .cloned()
            .map(|s| ExerciceStocke {
                niveaux_donnes: false,
                ..s
            })
            .collect()
    };
    assert_eq!(sans_niveaux(&v1), sans_niveaux(&v2));
}
