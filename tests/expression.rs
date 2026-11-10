mod commun;

use mine_de_rien::alea::Alea;
use mine_de_rien::exercises::expression::{calculer, ecrire_reponse};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::Signed;

fn fraction(numerateur: i64, denominateur: i64) -> BigRational {
    BigRational::new(BigInt::from(numerateur), BigInt::from(denominateur))
}

#[test]
fn calculs_acceptes() {
    for (saisie, enonce, reponse) in [
        ("348 ÷ 12", "348 ÷ 12 = ?", "29"),
        ("3,5 + 1,25", "3,5 + 1,25 = ?", "4,75"),
        ("(12 + 8) x 3", "(12 + 8) × 3 = ?", "60"),
        ("348:12", "348 ÷ 12 = ?", "29"),
        ("348 / 12 = ?", "348 ÷ 12 = ?", "29"),
        ("12*7 =", "12 × 7 = ?", "84"),
        ("3.5 + 1.5", "3,5 + 1,5 = ?", "5"),
        ("0,1 + 0,2", "0,1 + 0,2 = ?", "0,3"),
        ("7 - 7", "7 - 7 = ?", "0"),
        ("2 + 3 × 4", "2 + 3 × 4 = ?", "14"),
        ("(2 + 3) × 4", "(2 + 3) × 4 = ?", "20"),
        ("100 - 20 - 30", "100 - 20 - 30 = ?", "50"),
        ("100 ÷ 5 ÷ 2", "100 ÷ 5 ÷ 2 = ?", "10"),
        ("1 ÷ 4", "1 ÷ 4 = ?", "0,25"),
    ] {
        let ex = calculer(saisie).unwrap();
        assert_eq!(ex.enonce, enonce);
        assert_eq!(ex.reponse, reponse);
    }
}

#[test]
fn calculs_refuses() {
    for (saisie, motif) in [
        ("10 ÷ 3", "tombe pas juste"),
        ("1 ÷ 8", "tombe pas juste"),
        ("5 - 9", "négatif"),
        ("2 ** 8", "mal formée"),
        ("__import__('os')", "non reconnu"),
        ("2 ^ 8", "non reconnu"),
        ("3", "manque une opération"),
        ("(1 + 2", "non fermée"),
        ("1 + 2)", "mal formée"),
        ("1 ÷ 0", "zéro"),
        ("1 ÷ (2 - 2)", "zéro"),
        ("-5 + 9", "mal formée"),
        ("1 +", "incomplète"),
        ("", "vide"),
        ("1234567890 + 1", "trop grand"),
        ("1000+2000+3000+4000+5000+6000+7000+8000+9000", "panneau"),
    ] {
        let erreur = calculer(saisie).unwrap_err().to_string();
        assert!(erreur.contains(motif), "{saisie} : {erreur}");
    }
}

#[test]
fn competence_selon_les_operations() {
    assert_eq!(
        calculer("348 ÷ 12").unwrap().competence,
        "maths/division-posee"
    );
    assert_eq!(
        calculer("12 x 7").unwrap().competence,
        "maths/multiplication-posee"
    );
    assert_eq!(
        calculer("3,5 + 1,25").unwrap().competence,
        "maths/addition-posee"
    );
    assert_eq!(
        calculer("9 - 4").unwrap().competence,
        "maths/soustraction-posee"
    );
    assert_eq!(calculer("(12 + 8) x 3").unwrap().competence, "maths/calcul");
}

#[test]
fn ecriture_unique_de_la_reponse() {
    assert_eq!(ecrire_reponse(&fraction(7, 2)).unwrap(), "3,5");
    assert_eq!(ecrire_reponse(&fraction(350, 100)).unwrap(), "3,5");
    assert_eq!(ecrire_reponse(&fraction(301, 100)).unwrap(), "3,01");
    assert_eq!(ecrire_reponse(&fraction(30, 1)).unwrap(), "30");
}

/// Écrit `numerateur / denominateur` (dénominateur 1, 10 ou 100) en décimal à virgule.
fn ecrire(numerateur: i64, denominateur: i64) -> String {
    let (entier, reste) = (numerateur / denominateur, numerateur % denominateur);
    if reste == 0 {
        return entier.to_string();
    }
    let largeur = if denominateur == 10 { 1 } else { 2 };
    format!("{entier},{reste:0largeur$}")
        .trim_end_matches('0')
        .to_owned()
}

fn relire(reponse: &str) -> BigRational {
    let (entier, decimales) = reponse.split_once(',').unwrap_or((reponse, ""));
    let numerateur: BigInt = format!("{entier}{decimales}").parse().unwrap();
    BigRational::new(numerateur, BigInt::from(10).pow(decimales.len() as u32))
}

/// Sur des opérations tirées au hasard, la réponse relue vaut le calcul exact.
#[test]
fn propriete_le_calcul_est_exact() {
    for graine in 0..300 {
        let mut alea = Alea::graine(graine);
        let (na, da) = (alea.entier(0, 9999), *alea.choisir(&[1, 10, 100]).unwrap());
        let (nb, db) = (alea.entier(1, 999), *alea.choisir(&[1, 10, 100]).unwrap());
        let (a, b) = (fraction(na, da), fraction(nb, db));
        let (op, attendu) = [
            ("+", &a + &b),
            ("x", &a * &b),
            ("÷", &a / &b),
            ("-", &a - &b),
        ][alea.indice(4)]
        .clone();
        let texte = format!("{} {op} {}", ecrire(na, da), ecrire(nb, db));
        match calculer(&texte) {
            Err(_) => {
                let centiemes = &attendu * fraction(100, 1);
                assert!(attendu.is_negative() || !centiemes.is_integer(), "{texte}");
            }
            Ok(ex) => {
                assert_eq!(relire(&ex.reponse), attendu, "{texte}");
                assert!(!ex.reponse.contains('.'));
                assert!(!ex.reponse.contains(',') || !ex.reponse.ends_with('0'));
            }
        }
    }
}

/// Une centaine d'expressions : même exercice ou même refus qu'en Python.
#[test]
fn oracle_python() {
    for cas in commun::oracle("expression") {
        let saisie = cas["entree"].as_str().unwrap();
        // Écart assumé : seuls 0 à 9 sont des chiffres, là où Python lisait toute écriture.
        if saisie
            .chars()
            .any(|c| c.is_numeric() && !c.is_ascii_digit())
        {
            assert!(
                calculer(saisie)
                    .unwrap_err()
                    .to_string()
                    .contains("non reconnu")
            );
            continue;
        }
        commun::comparer(&cas, calculer(saisie));
    }
}
