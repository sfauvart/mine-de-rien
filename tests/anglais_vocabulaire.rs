//! Générateur de vocabulaire anglais : propriétés sur des centaines de graines.

mod commun;

use std::collections::BTreeSet;

use mine_de_rien::alea::Alea;
use mine_de_rien::exercises::anglais::vocabulaire::{self as v, Paire, Sens};
use mine_de_rien::exercises::base::{Exercice, normaliser};
use mine_de_rien::minecraft::texte::lignes_panneau;
use mine_de_rien::saisie::{Source, cle_valide, interpreter};
use serde_json::json;

const GRAINES: std::ops::Range<u64> = 0..500;

fn par_anglais(mot: &str) -> &'static Paire {
    let mut trouvees = v::lexique().iter().filter(|p| p.anglais == mot);
    let p = trouvees.next().unwrap();
    assert!(trouvees.next().is_none());
    p
}

fn donnee<'a>(ex: &'a Exercice, cle: &str) -> &'a str {
    ex.donnees[cle].as_str().unwrap()
}

fn compter(famille: &str) -> usize {
    v::lexique().iter().filter(|p| p.famille == famille).count()
}

#[test]
fn lexique_charge_les_cinq_familles() {
    let familles: BTreeSet<&str> = v::lexique().iter().map(|p| p.famille.as_str()).collect();
    assert_eq!(
        familles,
        BTreeSet::from(["couleurs", "jours", "mois", "nombres", "salutations"])
    );
    assert_eq!(compter("jours"), 7);
    assert_eq!(compter("mois"), 12);
    assert_eq!(compter("nombres"), 21);
}

#[test]
fn proprietes() {
    for graine in GRAINES {
        let ex = v::generer(&mut Alea::graine(graine), 1);
        assert_eq!(ex.competence, v::COMPETENCE);
        assert!(lignes_panneau(&ex.enonce).is_some(), "{}", ex.enonce);
        assert!(cle_valide(&ex.reponse), "{}", ex.reponse);
        assert_eq!(ex.reponse, ex.reponse.to_lowercase());
        let p = par_anglais(donnee(&ex, "anglais"));
        let attendu = if donnee(&ex, "sens") == Sens::VersAnglais.nom() {
            &p.anglais
        } else {
            &p.francais
        };
        assert_eq!(&ex.reponse, attendu);
        assert!(ex.accepte(&ex.reponse));
        assert!(!ex.accepte(&format!("{}s", ex.reponse)));
        assert_eq!(v::generer(&mut Alea::graine(graine), 1), ex);
    }
}

#[test]
fn bonjour_jamais_demande_vers_l_anglais() {
    for mot in ["hello", "good morning"] {
        assert_eq!(par_anglais(mot).sens, [Sens::VersFrancais]);
    }
    for graine in GRAINES {
        assert!(
            !v::generer(&mut Alea::graine(graine), 1)
                .enonce
                .starts_with("bonjour")
        );
    }
}

#[test]
fn mots_identiques_jamais_tires() {
    assert!(par_anglais("orange").sens.is_empty());
    assert!(par_anglais("six").sens.is_empty());
    for graine in GRAINES {
        let ex = v::generer(&mut Alea::graine(graine), 1);
        assert!(!["orange", "six"].contains(&donnee(&ex, "anglais")));
    }
}

#[test]
fn goodbye_seulement_vers_le_francais() {
    assert_eq!(par_anglais("goodbye").sens, [Sens::VersFrancais]);
}

#[test]
fn deux_sens_et_cinq_familles_tires() {
    let exercices: Vec<Exercice> = GRAINES
        .map(|g| v::generer(&mut Alea::graine(g), 1))
        .collect();
    let sens: BTreeSet<&str> = exercices.iter().map(|ex| donnee(ex, "sens")).collect();
    assert_eq!(sens, BTreeSet::from(["vers-anglais", "vers-francais"]));
    let familles: BTreeSet<&str> = exercices.iter().map(|ex| donnee(ex, "famille")).collect();
    assert_eq!(familles.len(), 5);
}

#[test]
fn enonces() {
    let mercredi = par_anglais("wednesday");
    assert_eq!(
        v::exercice(mercredi, Sens::VersAnglais).unwrap().enonce,
        "mercredi en anglais ?"
    );
    assert_eq!(
        v::exercice(mercredi, Sens::VersFrancais).unwrap().enonce,
        "Wednesday en français ?"
    );
    let dix_sept = v::exercice(par_anglais("seventeen"), Sens::VersFrancais).unwrap();
    assert_eq!(dix_sept.enonce, "seventeen en français, en lettres ?");
    assert_eq!(dix_sept.reponse, "dix-sept");
    assert_eq!(
        v::exercice(par_anglais("good night"), Sens::VersAnglais)
            .unwrap()
            .reponse,
        "good night"
    );
}

fn entree(famille: &str, anglais: &str, francais: &str) -> v::Entree {
    (
        famille.to_owned(),
        anglais.to_owned(),
        francais.to_owned(),
        true,
    )
}

#[test]
fn repli_sans_accents() {
    let entrees = [
        entree("mois", "august", "août"),
        entree("mois", "may", "mai"),
    ];
    let paires = v::construire(&entrees, false).unwrap();
    assert_eq!(paires[0].sens, [Sens::VersAnglais]);
    assert_eq!(paires[1].sens, [Sens::VersAnglais, Sens::VersFrancais]);
    let paires = v::construire(&entrees, true).unwrap();
    assert_eq!(paires[0].sens, [Sens::VersAnglais, Sens::VersFrancais]);
}

#[test]
fn majuscule_refusee_dans_le_lexique() {
    assert!(v::construire(&[entree("jours", "Monday", "lundi")], true).is_err());
}

#[test]
fn sens_interdit_refuse() {
    assert!(v::exercice(par_anglais("hello"), Sens::VersAnglais).is_err());
}

#[test]
fn exercice_pour_un_mot_lu() {
    let mut alea = Alea::graine(1);
    let ex = v::exercice_pour("Wednesday", &mut alea).unwrap();
    assert_eq!(normaliser(donnee(&ex, "francais")), "mercredi");
    assert!(v::exercice_pour("GOOD NIGHT", &mut alea).is_some());
    assert_eq!(
        donnee(&v::exercice_pour("février", &mut alea).unwrap(), "anglais"),
        "february"
    );
    assert!(v::exercice_pour("bonjour", &mut alea).is_none()); // ambigu
    assert!(v::exercice_pour("orange", &mut alea).is_none()); // aucun sens autorisé
    assert!(v::exercice_pour("book", &mut alea).is_none()); // hors lexique
}

#[test]
fn cle_valide_accepte_et_refuse() {
    for bonne in [
        "29",
        "4,75",
        "mercredi",
        "août",
        "dix-sept",
        "good night",
        "bon après-midi",
    ] {
        assert!(cle_valide(bonne), "{bonne}");
    }
    let trop_longue = "a".repeat(51);
    for mauvaise in [
        "Mercredi",
        " lundi",
        "lundi ",
        "good  night",
        "3.5",
        "3,50",
        &trop_longue,
        "",
    ] {
        assert!(!cle_valide(mauvaise), "{mauvaise}");
    }
}

#[test]
fn coffre_anglais_3() {
    let saisie = interpreter("anglais 3", &mut Alea::graine(4)).unwrap();
    assert_eq!(saisie.source, Source::Competence);
    let enonces: BTreeSet<&str> = saisie
        .exercices
        .iter()
        .map(|ex| ex.enonce.as_str())
        .collect();
    assert_eq!(enonces.len(), 3);
}

/// Tout le lexique : mêmes sens autorisés et mêmes exercices qu'en Python.
#[test]
fn oracle_python() {
    let cas = commun::oracle("vocabulaire");
    assert_eq!(cas.len(), v::lexique().len());
    for (cas, p) in cas.iter().zip(v::lexique()) {
        assert_eq!(cas["entree"]["anglais"], p.anglais.as_str());
        let exercices: serde_json::Map<String, serde_json::Value> = p
            .sens
            .iter()
            .map(|s| (s.nom().to_owned(), json!(v::exercice(p, *s).unwrap())))
            .collect();
        let sortie = json!({
            "francais": p.francais,
            "famille": p.famille,
            "sens": p.sens.iter().map(|s| s.nom()).collect::<Vec<_>>(),
            "accent": p.accent(),
            "exercices": exercices,
        });
        assert_eq!(sortie, cas["sortie"], "{}", p.anglais);
    }
}
