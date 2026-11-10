use mine_de_rien::alea::Alea;
use mine_de_rien::exercises::base::normaliser;
use mine_de_rien::exercises::maths::division;

fn entier(ex: &mine_de_rien::exercises::base::Exercice, cle: &str) -> i64 {
    ex.donnees[cle].as_i64().unwrap()
}

#[test]
fn niveau_1_tombe_juste() {
    for graine in 0..200 {
        let ex = division::generer(&mut Alea::graine(graine), 1);
        assert_eq!(
            entier(&ex, "dividende"),
            entier(&ex, "diviseur") * entier(&ex, "quotient")
        );
        assert!(ex.accepte(&entier(&ex, "quotient").to_string()));
    }
}

#[test]
fn niveau_2_reste_coherent() {
    for graine in 0..200 {
        let ex = division::generer(&mut Alea::graine(graine), 2);
        let (diviseur, reste) = (entier(&ex, "diviseur"), entier(&ex, "reste"));
        assert!(0 <= reste && reste < diviseur);
        assert_eq!(
            entier(&ex, "dividende"),
            diviseur * entier(&ex, "quotient") + reste
        );
    }
}

#[test]
fn reponse_tolerante_aux_espaces_et_a_la_casse() {
    assert_eq!(normaliser(" 12 R 3 "), normaliser("12r3"));
}

/// Les deux niveaux couvrent leurs bornes : le tirage n'est pas figé sur une valeur.
#[test]
fn les_tirages_varient_et_restent_dans_les_bornes() {
    let mut diviseurs = std::collections::BTreeSet::new();
    for graine in 0..200 {
        let ex = division::generer(&mut Alea::graine(graine), 1);
        let (diviseur, quotient) = (entier(&ex, "diviseur"), entier(&ex, "quotient"));
        assert!((2..=9).contains(&diviseur) && (12..=99).contains(&quotient));
        diviseurs.insert(diviseur);
    }
    assert_eq!(diviseurs.len(), 8);
}
