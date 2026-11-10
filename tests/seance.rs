//! La séance de dictée de chaque jour, avec une horloge simulée et une vraie base jetable.

mod commun;

use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use chrono_tz::Europe::Paris;
use mine_de_rien::alea::Alea;
use mine_de_rien::config::lire_config;
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::exercises::francais::dictee::{Position, decouper};
use mine_de_rien::saisie::Source;
use mine_de_rien::seance::{
    Apparition, Etat, bilan, creer_seance_du_jour, etat, historique, jour_local, poids, poids_de,
};
use mine_de_rien::storage::sqlite::{Demande, Depot, PhraseSonore, Statut};

const DICTEE_6: &str = "Dans la ville de Dubaï, aux Émirats arabes unis, s'élève la plus haute \
    tour jamais construite. Burj Khalifa culmine à huit-cent-vingt-huit mètres de hauteur, \
    c'est presque trois tours Eiffel. Deux-millions de personnes la visitent chaque année.";

const DEMANDE: Demande<'static> = Demande {
    source: Source::Dictee,
    demande_par: "seance",
    recompense: "minecraft:diamond",
    recompense_nombre: 3,
};

fn jour(m: u32, j: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, m, j).unwrap()
}

/// 18 h à Paris en novembre (17 h UTC), le jour donné.
fn soir(j: u32) -> DateTime<Utc> {
    commun::instant(&format!("2026-11-{j:02}T17:00:00+00:00"))
}

fn depot() -> (tempfile::TempDir, Depot) {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap();
    (dossier, depot)
}

/// Enregistre `texte` comme dictée du jour `pour`, confirmée à `confirmee`.
fn dictee(depot: &Depot, texte: &str, pour: NaiveDate, confirmee: DateTime<Utc>) -> i64 {
    let phrases: Vec<PhraseSonore> = decouper(texte)
        .into_iter()
        .map(|p| PhraseSonore {
            texte: p.texte,
            son: b"OggS normal".to_vec(),
            son_lent: b"OggS lent".to_vec(),
        })
        .collect();
    depot
        .ajouter_dictee(pour, texte, &phrases, "111", confirmee)
        .unwrap()
}

/// La dictée 6, confirmée le dimanche 8 novembre pour le vendredi 13.
fn dictee_de_vendredi(depot: &Depot) -> i64 {
    dictee(depot, DICTEE_6, jour(11, 13), soir(8))
}

fn seance(depot: &Depot, quand: DateTime<Utc>, graine: u64) -> Option<Vec<(i64, Exercice)>> {
    creer_seance_du_jour(depot, quand, Paris, 4, &mut Alea::graine(graine), DEMANDE)
        .unwrap()
        .map(|s| s.coffres)
}

/// L'enfant ouvre le coffre `ident` à `quand`, après avoir écrit `rates` puis le bon mot.
fn ouvrir(depot: &Depot, ident: i64, quand: DateTime<Utc>, rates: &[&str]) {
    let reponse = depot.lire(ident).unwrap().unwrap().exercice.reponse;
    depot.marquer_pose(ident, quand).unwrap();
    for rate in rates {
        depot.noter_tentative(ident, rate, false, quand).unwrap();
    }
    depot.noter_tentative(ident, &reponse, true, quand).unwrap();
    depot
        .marquer_ouvert(ident, quand + TimeDelta::minutes(2))
        .unwrap();
}

fn position(ex: &Exercice) -> Position {
    Position {
        phrase: ex.donnees["phrase"].as_u64().unwrap() as usize,
        mot: ex.donnees["mot"].as_u64().unwrap() as usize,
    }
}

fn apparition(j: u32, ouvert: bool, rates: &[&str]) -> Apparition {
    Apparition {
        jour: jour(11, j),
        ouvert,
        rates: rates.iter().map(|r| (*r).to_owned()).collect(),
    }
}

#[test]
fn le_jour_se_compte_a_la_maison() {
    // 23 h 30 à Paris un lundi d'hiver : encore lundi, alors qu'il est 22 h 30 UTC.
    assert_eq!(
        jour_local(commun::instant("2026-11-09T22:30:00+00:00"), Paris),
        jour(11, 9)
    );
    // 0 h 30 à Paris : déjà mardi, encore lundi en UTC.
    let nuit = commun::instant("2026-11-09T23:30:00+00:00");
    assert_eq!(jour_local(nuit, Paris), jour(11, 10));
    assert_eq!(nuit.date_naive(), jour(11, 9));
    // En été, Paris a deux heures d'avance : 22 h 30 UTC, c'est déjà le lendemain.
    assert_eq!(
        jour_local(commun::instant("2026-07-01T22:30:00+00:00"), Paris),
        jour(7, 2)
    );
    assert_eq!(
        jour_local(commun::instant("2026-07-01T21:30:00+00:00"), Paris),
        jour(7, 1)
    );
}

#[test]
fn etat_et_poids_d_un_mot() {
    let mercredi = jour(11, 11);
    let cas: [(&[Apparition], Etat, f64); 9] = [
        (&[], Etat::Nouveau, 1.0),
        (&[apparition(11, false, &[])], Etat::EnCours, 0.0),
        (
            &[apparition(9, true, &["plu"])],
            Etat::Fragile(vec!["plu".to_owned()]),
            10.0,
        ),
        (
            &[apparition(10, true, &["plu"])],
            Etat::Fragile(vec!["plu".to_owned()]),
            0.0,
        ),
        (
            &[apparition(11, true, &["plu"])],
            Etat::Fragile(vec!["plu".to_owned()]),
            0.0,
        ),
        (&[apparition(9, true, &[])], Etat::AConfirmer, 1.0),
        (
            &[apparition(8, true, &[]), apparition(9, true, &[])],
            Etat::Acquis,
            0.0,
        ),
        // Raté puis réussi une fois : pas encore acquis.
        (
            &[apparition(8, true, &["plu"]), apparition(10, true, &[])],
            Etat::AConfirmer,
            1.0,
        ),
        // Deux réussites, puis une erreur : fragile de nouveau.
        (
            &[
                apparition(5, true, &[]),
                apparition(6, true, &[]),
                apparition(9, true, &["pluss"]),
            ],
            Etat::Fragile(vec!["pluss".to_owned()]),
            10.0,
        ),
    ];
    for (apparitions, attendu, poids) in cas {
        assert_eq!(etat(apparitions), attendu, "{apparitions:?}");
        assert_eq!(poids_de(apparitions, mercredi), poids, "{apparitions:?}");
    }
}

#[test]
fn une_seance_par_jour_du_lendemain_a_la_veille() {
    let (_dossier, depot) = depot();
    let ident = dictee_de_vendredi(&depot);
    // Dimanche, jour de la confirmation : pas encore.
    assert_eq!(seance(&depot, soir(8) + TimeDelta::hours(2), 1), None);
    // Lundi : quatre coffres. Une seconde connexion lundi n'en crée pas.
    let lundi = seance(&depot, soir(9), 1).unwrap();
    assert_eq!(lundi.len(), 4);
    assert_eq!(seance(&depot, soir(9) + TimeDelta::hours(3), 2), None);
    for (_, ex) in &lundi {
        assert_eq!(ex.donnees["dictee"], ident);
    }
    let coffres = depot.en_file().unwrap();
    assert!(
        coffres
            .iter()
            .all(|c| c.source == Source::Dictee && c.demande_par == "seance")
    );
    // Mardi : quatre autres, même si ceux de lundi attendent encore.
    let mardi = seance(&depot, soir(10), 3).unwrap();
    assert_eq!(mardi.len(), 4);
    let deja: BTreeSet<Position> = lundi.iter().map(|(_, ex)| position(ex)).collect();
    assert!(mardi.iter().all(|(_, ex)| !deja.contains(&position(ex))));
    assert_eq!(depot.en_file().unwrap().len(), 8);
    // Mercredi et jeudi (la veille) : oui. Vendredi, jour de la dictée, et après : non.
    assert!(seance(&depot, soir(11), 4).is_some());
    assert!(seance(&depot, soir(12), 5).is_some());
    assert_eq!(seance(&depot, soir(13), 6), None);
    assert_eq!(seance(&depot, soir(14), 7), None);
    assert_eq!(
        depot.lire_dictee(ident).unwrap().unwrap().derniere_seance,
        Some(jour(11, 12))
    );
}

#[test]
fn un_jour_sans_connexion_n_est_pas_rattrape() {
    let (_dossier, depot) = depot();
    dictee_de_vendredi(&depot);
    // Ni lundi ni mardi : mercredi, quatre coffres, pas douze.
    assert_eq!(seance(&depot, soir(11), 1).unwrap().len(), 4);
    assert_eq!(depot.en_file().unwrap().len(), 4);
}

#[test]
fn minuit_a_paris_change_de_jour() {
    let (_dossier, depot) = depot();
    dictee_de_vendredi(&depot);
    // Lundi 23 h 30 à Paris : la séance de lundi.
    assert!(seance(&depot, commun::instant("2026-11-09T22:30:00+00:00"), 1).is_some());
    // Mardi 0 h 30 à Paris, encore lundi en UTC : c'est déjà la séance de mardi.
    assert!(seance(&depot, commun::instant("2026-11-09T23:30:00+00:00"), 2).is_some());
    assert_eq!(
        seance(&depot, commun::instant("2026-11-10T08:00:00+00:00"), 3),
        None
    );
}

#[test]
fn un_mot_rate_repose_puis_revient_un_mot_reussi_deux_fois_sort() {
    let (_dossier, depot) = depot();
    let ident = dictee_de_vendredi(&depot);
    let lundi = seance(&depot, soir(9), 1).unwrap();
    // Lundi : le premier mot est raté, les trois autres réussis du premier coup.
    ouvrir(&depot, lundi[0].0, soir(9), &["fote", "faute"]);
    for (coffre, _) in &lundi[1..] {
        ouvrir(&depot, *coffre, soir(9), &[]);
    }
    let rate = position(&lundi[0].1);
    let reussi = position(&lundi[1].1);

    // Mardi : le mot raté lundi repose ; mercredi il pèse dix fois plus.
    assert_eq!(
        poids(&depot, ident, jour(11, 10), Paris).unwrap()[&rate],
        0.0
    );
    assert_eq!(
        poids(&depot, ident, jour(11, 11), Paris).unwrap()[&rate],
        10.0
    );
    assert_eq!(
        poids(&depot, ident, jour(11, 10), Paris).unwrap()[&reussi],
        1.0
    );
    for graine in 0..200 {
        let (_d, autre) = self::depot();
        let ident = dictee_de_vendredi(&autre);
        let lundi = seance(&autre, soir(9), graine).unwrap();
        ouvrir(&autre, lundi[0].0, soir(9), &["fote"]);
        let rate = position(&lundi[0].1);
        let mardi = seance(&autre, soir(10), graine).unwrap();
        assert!(
            mardi.iter().all(|(_, ex)| position(ex) != rate),
            "graine {graine}"
        );
        assert_eq!(
            poids(&autre, ident, jour(11, 11), Paris).unwrap()[&rate],
            10.0
        );
    }

    // Le mot réussi lundi, réussi encore mercredi : acquis, il ne sort plus.
    let histoire = historique(&depot, ident, Paris).unwrap();
    assert_eq!(histoire[&rate][0].rates, ["fote", "faute"]);
    assert_eq!(histoire[&rate][0].jour, jour(11, 9));
    let (phrases, numeros) = mine_de_rien::dictees::phrases_enregistrees(&depot, ident).unwrap();
    let mut encore =
        mine_de_rien::dictees::exercice_sonore(&phrases, reussi, numeros[reussi.phrase]).unwrap();
    encore.donnees.insert("dictee".to_owned(), ident.into());
    let coffre = depot.ajouter(&encore, DEMANDE, soir(11)).unwrap();
    assert_eq!(
        poids(&depot, ident, jour(11, 11), Paris).unwrap()[&reussi],
        0.0,
        "en file"
    );
    ouvrir(&depot, coffre, soir(11), &[]);
    for j in 11..=20 {
        assert_eq!(
            poids(&depot, ident, jour(11, j), Paris).unwrap()[&reussi],
            0.0
        );
    }

    let b = bilan(&depot, ident, Paris).unwrap();
    assert_eq!((b.mots, b.acquis), (34, 1));
    let mot_rate = lundi[0].1.reponse.clone();
    assert_eq!(
        b.fragiles,
        [(
            mot_rate.clone(),
            vec!["fote".to_owned(), "faute".to_owned()]
        )]
    );
    assert_eq!(
        b.fragiles_en_clair().unwrap(),
        format!("{mot_rate} (fote, faute)")
    );
}

#[test]
fn un_coffre_annule_ne_compte_pas_dans_l_histoire() {
    let (_dossier, depot) = depot();
    let ident = dictee_de_vendredi(&depot);
    let lundi = seance(&depot, soir(9), 1).unwrap();
    depot.annuler(lundi[0].0).unwrap();
    let histoire = historique(&depot, ident, Paris).unwrap();
    assert!(!histoire.contains_key(&position(&lundi[0].1)));
    assert_eq!(histoire.len(), 3);
}

#[test]
fn les_coffres_deja_en_file_restent_devant() {
    let (_dossier, depot) = depot();
    dictee_de_vendredi(&depot);
    let maths = Exercice {
        competence: "maths/division-posee".to_owned(),
        enonce: "348 ÷ 12 = ?".to_owned(),
        reponse: "29".to_owned(),
        donnees: Default::default(),
    };
    let demande = Demande {
        source: Source::Expression,
        ..DEMANDE
    };
    let premier = depot.ajouter(&maths, demande, soir(9)).unwrap();
    seance(&depot, soir(9), 1).unwrap();
    let file = depot.en_file().unwrap();
    assert_eq!(file.len(), 5);
    assert_eq!(file[0].id, premier);
    assert_eq!(depot.suivant().unwrap().unwrap().exercice.reponse, "29");
}

#[test]
fn deux_dictees_la_plus_proche_d_abord_et_une_annulee_jamais() {
    let (_dossier, depot) = depot();
    let lointaine = dictee(&depot, "Il pleut sur la ville.", jour(11, 20), soir(7));
    let proche = dictee_de_vendredi(&depot);
    let lundi = creer_seance_du_jour(&depot, soir(9), Paris, 4, &mut Alea::graine(1), DEMANDE)
        .unwrap()
        .unwrap();
    assert_eq!(lundi.dictee.id, proche);
    // Une seule séance par jour, toutes dictées confondues.
    assert_eq!(seance(&depot, soir(9) + TimeDelta::hours(1), 2), None);
    // La proche annulée, c'est la lointaine que sert mardi.
    depot.annuler_dictee(proche).unwrap();
    let mardi = creer_seance_du_jour(&depot, soir(10), Paris, 4, &mut Alea::graine(1), DEMANDE)
        .unwrap()
        .unwrap();
    assert_eq!(mardi.dictee.id, lointaine);
    depot.annuler_dictee(lointaine).unwrap();
    assert_eq!(seance(&depot, soir(11), 3), None);
}

#[test]
fn seance_plus_courte_puis_plus_rien_quand_tout_est_acquis() {
    let (_dossier, depot) = depot();
    let ident = dictee(&depot, "Il pleut ici.", jour(11, 13), soir(8));
    // Trois mots seulement : la séance de lundi en a trois.
    let lundi = seance(&depot, soir(9), 1).unwrap();
    assert_eq!(lundi.len(), 3);
    for (coffre, _) in &lundi {
        ouvrir(&depot, *coffre, soir(9), &[]);
    }
    let mardi = seance(&depot, soir(10), 1).unwrap();
    assert_eq!(mardi.len(), 3);
    for (coffre, _) in &mardi {
        ouvrir(&depot, *coffre, soir(10), &[]);
    }
    // Tout est acquis : mercredi, une séance vide, que l'annonce explique aux parents.
    let mercredi = creer_seance_du_jour(&depot, soir(11), Paris, 4, &mut Alea::graine(1), DEMANDE)
        .unwrap()
        .unwrap();
    assert!(mercredi.coffres.is_empty());
    assert_eq!((mercredi.bilan.mots, mercredi.bilan.acquis), (3, 3));
    assert_eq!(
        mercredi.annonce(),
        format!(
            "Pas de séance aujourd'hui : tous les mots de la dictée n°{ident} (vendredi 13 \
             novembre) sont acquis, ou reposent après une erreur."
        )
    );
    // Et une seule annonce par jour.
    assert_eq!(seance(&depot, soir(11) + TimeDelta::hours(1), 1), None);
}

#[test]
fn annonce_aux_parents_et_veille_de_la_dictee() {
    let (_dossier, depot) = depot();
    let ident = dictee_de_vendredi(&depot);
    let lundi = creer_seance_du_jour(&depot, soir(9), Paris, 4, &mut Alea::graine(1), DEMANDE)
        .unwrap()
        .unwrap();
    assert!(!lundi.veille);
    assert_eq!(
        lundi.annonce(),
        format!(
            "Séance de dictée du jour : 4 coffre(s), dictée n°{ident} pour le vendredi 13 novembre."
        )
    );
    ouvrir(&depot, lundi.coffres[0].0, soir(9), &["persone"]);
    let jeudi = creer_seance_du_jour(&depot, soir(12), Paris, 4, &mut Alea::graine(2), DEMANDE)
        .unwrap()
        .unwrap();
    assert!(jeudi.veille);
    let mot = &lundi.coffres[0].1.reponse;
    assert_eq!(
        jeudi.annonce().lines().nth(1).unwrap(),
        format!("La dictée est demain. Mots encore fragiles : {mot} (persone).")
    );
}

#[test]
fn un_mot_rate_d_une_dictee_passee_revient_en_revision() {
    let (_dossier, depot) = depot();
    // Dictée du vendredi 6, travaillée la semaine d'avant : un mot y est resté raté.
    let passee = dictee(&depot, "Le chat dort sur le canapé.", jour(11, 6), soir(1));
    let ancienne = creer_seance_du_jour(&depot, soir(3), Paris, 4, &mut Alea::graine(1), DEMANDE)
        .unwrap()
        .unwrap();
    ouvrir(&depot, ancienne.coffres[0].0, soir(3), &["canapai"]);
    for (coffre, _) in &ancienne.coffres[1..] {
        ouvrir(&depot, *coffre, soir(3), &[]);
    }
    let courante = dictee_de_vendredi(&depot);
    for graine in 0..50 {
        let (_d, autre) = self::depot();
        let passee = dictee(&autre, "Le chat dort sur le canapé.", jour(11, 6), soir(1));
        let s = creer_seance_du_jour(&autre, soir(3), Paris, 4, &mut Alea::graine(1), DEMANDE)
            .unwrap()
            .unwrap();
        ouvrir(&autre, s.coffres[0].0, soir(3), &["x"]);
        let courante = dictee_de_vendredi(&autre);
        let lundi = seance(&autre, soir(9), graine).unwrap();
        let origines: Vec<i64> = lundi
            .iter()
            .map(|(_, ex)| ex.donnees["dictee"].as_i64().unwrap())
            .collect();
        // Trois coffres de la dictée en cours, et le dernier reprend le mot raté de l'ancienne.
        assert_eq!(
            origines,
            [courante, courante, courante, passee],
            "graine {graine}"
        );
        assert_eq!(lundi[3].1.reponse, s.coffres[0].1.reponse);
    }
    let lundi = seance(&depot, soir(9), 1).unwrap();
    assert_eq!(lundi[3].1.donnees["dictee"], passee);
    assert_eq!(lundi[3].1.reponse, ancienne.coffres[0].1.reponse);
    // Le même vieux mot ne revient pas mardi : son coffre attend déjà.
    let mardi = seance(&depot, soir(10), 2).unwrap();
    assert!(mardi.iter().all(|(_, ex)| ex.donnees["dictee"] == courante));
    // Sans dictée en cours, pas de révision : rien.
    let (_d, seule) = self::depot();
    dictee(&seule, "Le chat dort.", jour(11, 6), soir(1));
    assert_eq!(seance(&seule, soir(9), 1), None);
}

#[test]
fn configuration_des_seances() {
    const BASE: &str = "[enfant]\npseudo_minecraft = \"Steve\"\n";
    let lire = |section: &str| lire_config(&format!("{BASE}{section}"), &|_| None).map_err(|e| e.0);
    let defaut = lire("").unwrap();
    assert_eq!(defaut.dictee.coffres_par_seance, 4);
    assert_eq!(defaut.dictee.tz(), Paris);
    // Aux valeurs d'usage, la section ne s'écrit pas : même relecture qu'avant.
    assert!(
        serde_json::to_value(&defaut)
            .unwrap()
            .get("dictee")
            .is_none()
    );
    let reglee = lire("[dictee]\ncoffres_par_seance = 5\nfuseau = \"America/Montreal\"\n").unwrap();
    assert_eq!(reglee.dictee.coffres_par_seance, 5);
    assert_eq!(reglee.dictee.tz(), chrono_tz::America::Montreal);
    assert!(
        serde_json::to_value(&reglee)
            .unwrap()
            .get("dictee")
            .is_some()
    );
    for (section, message) in [
        (
            "[dictee]\ncoffres_par_seance = 2\n",
            "[dictee] coffres_par_seance doit être entre 3 et 5 : 2",
        ),
        (
            "[dictee]\ncoffres_par_seance = 6\n",
            "[dictee] coffres_par_seance doit être entre 3 et 5 : 6",
        ),
        (
            "[dictee]\nfuseau = \"Paris\"\n",
            "[dictee] fuseau inconnu : 'Paris' (exemple : \"Europe/Paris\")",
        ),
    ] {
        assert_eq!(lire(section).unwrap_err(), message);
    }
    let _ = HashMap::<u8, u8>::new();
    let _ = Statut::EnFile;
}
