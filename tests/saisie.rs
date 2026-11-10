mod commun;

use std::collections::BTreeSet;

use mine_de_rien::alea::Alea;
use mine_de_rien::saisie::{NOMBRE_MAX, Saisie, Source, cle_valide, interpreter};
use serde_json::json;

fn lire(texte: &str, graine: u64) -> Saisie {
    interpreter(texte, &mut Alea::graine(graine)).unwrap()
}

fn enonces(saisie: &Saisie) -> BTreeSet<&str> {
    saisie
        .exercices
        .iter()
        .map(|ex| ex.enonce.as_str())
        .collect()
}

#[test]
fn expression() {
    let s = lire("348 ÷ 12", 0);
    assert_eq!(s.source, Source::Expression);
    assert_eq!(s.exercices.len(), 1);
    assert_eq!(
        (
            s.exercices[0].enonce.as_str(),
            s.exercices[0].reponse.as_str()
        ),
        ("348 ÷ 12 = ?", "29")
    );
}

#[test]
fn competence_trois_exercices_differents_et_verifies() {
    let s = lire("division 3", 1);
    assert_eq!(s.source, Source::Competence);
    assert_eq!(s.exercices.len(), 3);
    assert_eq!(enonces(&s).len(), 3);
    for ex in &s.exercices {
        let d = |cle: &str| ex.donnees[cle].as_i64().unwrap();
        assert_eq!(d("dividende"), d("diviseur") * d("quotient"));
        assert_eq!(ex.reponse, d("quotient").to_string());
    }
}

#[test]
fn competence_sans_nombre_et_casse() {
    assert_eq!(lire("  Division ", 2).exercices.len(), 1);
}

#[test]
fn propriete_competence_toujours_utilisable() {
    for graine in 0..200 {
        let s = lire(&format!("division {NOMBRE_MAX}"), graine);
        assert_eq!(enonces(&s).len(), NOMBRE_MAX);
    }
}

#[test]
fn meme_graine_memes_exercices() {
    assert_eq!(
        lire("division 4", 7).exercices,
        lire("division 4", 7).exercices
    );
}

#[test]
fn nombre_borne() {
    for texte in ["division 0", "division 11", "division 500"] {
        let erreur = interpreter(texte, &mut Alea::graine(0)).unwrap_err();
        assert!(erreur.to_string().contains("Entre 1 et 10"), "{texte}");
    }
}

#[test]
fn competence_inconnue_liste_les_disponibles() {
    let erreur = interpreter("fractions 2", &mut Alea::graine(0)).unwrap_err();
    assert!(erreur.to_string().contains("division"));
}

#[test]
fn refus_avec_message() {
    for texte in ["10 ÷ 3", "5 - 9", "2 ** 8", "__import__", ""] {
        let erreur = interpreter(texte, &mut Alea::graine(0)).unwrap_err();
        assert!(!erreur.to_string().is_empty(), "{texte}");
    }
}

/// Clés limites, saisies de parent et refus : comme en Python.
#[test]
fn oracle_python() {
    for cas in commun::oracle("saisie") {
        let e = &cas["entree"];
        match e["f"].as_str().unwrap() {
            "cle_valide" => {
                let cle = e["cle"].as_str().unwrap();
                // Écart assumé : les chiffres d'autres écritures ne sont pas des chiffres.
                assert_eq!(json!(cle_valide(cle)), cas["sortie"], "{cle:?}");
            }
            "interpreter" => {
                let texte = e["texte"].as_str().unwrap();
                if texte.chars().any(|c| c.is_numeric() && !c.is_ascii_digit()) {
                    continue; // voir tests/expression.rs
                }
                let lue = interpreter(texte, &mut Alea::graine(0))
                    .map(|s| json!({"source": s.source.nom(), "exercices": s.exercices}));
                commun::comparer(&cas, lue);
            }
            "competence" => {
                let s = lire(e["texte"].as_str().unwrap(), 3);
                assert_eq!(
                    json!({"source": s.source.nom(), "nombre": s.exercices.len()}),
                    cas["sortie"],
                    "{e}"
                );
            }
            autre => panic!("fonction inconnue : {autre}"),
        }
    }
}
