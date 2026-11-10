//! Indices par paliers : jamais la réponse, sur des centaines d'exercices de chaque famille.

mod commun;

use mine_de_rien::alea::Alea;
use mine_de_rien::exercises::anglais::vocabulaire::{self, Sens};
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::exercises::expression::calculer;
use mine_de_rien::exercises::indices::{
    LONGUEUR_MAX, SECOURS, contient_la_reponse, en_lettres, indices, reperes,
};
use mine_de_rien::exercises::registre;
use serde_json::{Map, json};

const GRAINES: std::ops::Range<u64> = 0..500;

fn verifier(ex: &Exercice) -> Vec<String> {
    let paliers = indices(ex);
    assert!((1..=4).contains(&paliers.len()), "{ex:?}");
    for palier in &paliers {
        assert!(palier.chars().count() <= LONGUEUR_MAX, "{palier}");
        assert!(!contient_la_reponse(palier, ex), "{palier} / {ex:?}");
    }
    assert_eq!(indices(ex), paliers); // déterministe
    paliers
}

fn exercice(competence: &str, enonce: &str, reponse: &str) -> Exercice {
    Exercice {
        competence: competence.to_owned(),
        enonce: enonce.to_owned(),
        reponse: reponse.to_owned(),
        donnees: Map::new(),
    }
}

#[test]
fn generateurs_du_registre() {
    for entree in registre::ENTREES {
        for graine in GRAINES {
            verifier(&(entree.generer)(&mut Alea::graine(graine), 1));
        }
    }
}

#[test]
fn tout_le_lexique_dans_les_deux_sens() {
    for p in vocabulaire::lexique() {
        for sens in &p.sens {
            let paliers = verifier(&vocabulaire::exercice(p, *sens).unwrap());
            assert!(paliers.len() >= 2, "{p:?}");
        }
    }
}

fn calcul_au_hasard(alea: &mut Alea) -> Option<Exercice> {
    let nombre = |alea: &mut Alea| {
        if alea.indice(5) == 0 {
            return format!("{},{}", alea.entier(0, 99), alea.entier(1, 99));
        }
        match alea.indice(3) {
            0 => alea.entier(1, 9),
            1 => alea.entier(10, 999),
            _ => alea.entier(1000, 99999),
        }
        .to_string()
    };
    let (a, b) = (nombre(alea), nombre(alea));
    let operation = ["+", "-", "×", "÷"][alea.indice(4)];
    calculer(&format!("{a} {operation} {b}")).ok()
}

#[test]
fn calculs_a_deux_nombres() {
    let mut vus = 0;
    for graine in 0..3000 {
        if let Some(ex) = calcul_au_hasard(&mut Alea::graine(graine)) {
            verifier(&ex);
            vus += 1;
        }
    }
    assert!(vus > 500, "{vus}");
}

/// L'exercice que la lecture de photo fabrique pour « Double de 17 ».
fn double_ou_moitie(double: bool, nombre: &str) -> Option<Exercice> {
    let calcul = if double {
        format!("{nombre} x 2")
    } else {
        format!("{nombre} ÷ 2")
    };
    let reponse = calculer(&calcul).ok()?.reponse;
    let (mot, famille) = if double {
        ("Double", "doubles")
    } else {
        ("Moitié", "moities")
    };
    let donnees = json!({"famille": famille, "nombre": nombre});
    Some(Exercice {
        competence: "maths/doubles-moities".to_owned(),
        enonce: format!("{mot} de {nombre} = ?"),
        reponse,
        donnees: donnees.as_object().cloned().unwrap(),
    })
}

#[test]
fn doubles_et_moities_de_1_a_2000_et_decimaux() {
    let mut cas: Vec<(bool, String)> = Vec::new();
    for double in [true, false] {
        cas.extend((1..=2000).map(|n| (double, n.to_string())));
    }
    for n in 0..30 {
        cas.extend((1..10).map(|d| (true, format!("{n},{d}"))));
        cas.extend([2, 4, 6, 8].map(|d| (false, format!("{n},{d}"))));
    }
    for (double, nombre) in cas {
        // Une moitié impaire ne tombe pas juste à deux décimales près : pas d'exercice.
        let Some(ex) = double_ou_moitie(double, &nombre) else {
            assert!(!double, "{nombre}");
            continue;
        };
        assert!(verifier(&ex).len() >= 2, "{ex:?}");
    }
}

#[test]
fn cas_ecrits_a_la_main() {
    assert_eq!(
        indices(&double_ou_moitie(true, "17").unwrap()),
        [
            "Le double de 17, c'est 17 ajouté à lui-même.",
            "Découpe : 17 = 10 + 7. Cherche le double de 10, puis le double de 7.",
            "Le double de 10 = 20 et le double de 7 = 14 : il reste à les ajouter.",
        ]
    );
    let division = indices(&calculer("348 ÷ 12").unwrap());
    assert_eq!(division[0], "Cherche combien de fois 12 entre dans 348.");
    assert!(
        division
            .last()
            .unwrap()
            .starts_with("Première étape : dans 34, 12 va 2 fois, il reste 10.")
    );
    assert_eq!(
        indices(&mot("august", Sens::VersFrancais)),
        [
            "C'est un mois de l'année.",
            "Le mot français commence par « a » et a 4 lettres.",
            "C'est le 8e mois de l'année.",
        ]
    );
}

#[test]
fn priorites_pour_un_calcul_a_plusieurs_operations() {
    let paliers = indices(&calculer("(2 + 3) × 4").unwrap());
    assert_eq!(paliers.len(), 1);
    assert!(paliers[0].contains("parenthèses"));
}

#[test]
fn exercice_inconnu_donne_le_secours() {
    assert_eq!(
        indices(&exercice("francais/conjugaison", "Il (manger) ?", "mange")),
        [SECOURS]
    );
}

#[test]
fn un_palier_qui_donnerait_la_reponse_est_retire() {
    // 36 ÷ 6 = 6 : la liste des multiples de 6 commence par 6, elle est retirée.
    let paliers = indices(&calculer("36 ÷ 6").unwrap());
    assert!(!paliers.iter().any(|p| p.contains("multiples")));
}

#[test]
fn reponse_numerique_reconnue() {
    let ex = exercice("maths/x", "17 x 2 = ?", "34");
    for texte in [
        "c'est 34",
        "34,0 !",
        "34.00",
        "trente-quatre",
        "Trente quatre",
        "la réponse: 34.",
    ] {
        assert!(contient_la_reponse(texte, &ex), "{texte}");
    }
}

#[test]
fn reponse_numerique_non_reconnue() {
    let ex = exercice("maths/x", "17 x 2 = ?", "34");
    for texte in ["341", "134", "3,4", "17 + 17", "trente", "quatre"] {
        assert!(!contient_la_reponse(texte, &ex), "{texte}");
    }
}

#[test]
fn milliers_et_decimaux() {
    assert!(contient_la_reponse(
        "ça fait 13 545",
        &exercice("maths/x", "?", "13545")
    ));
    assert!(contient_la_reponse(
        "4.75",
        &exercice("maths/x", "?", "4,75")
    ));
    assert!(!contient_la_reponse(
        "4,7 et 5",
        &exercice("maths/x", "?", "4,75")
    ));
    assert!(contient_la_reponse(
        "vingt et un",
        &exercice("maths/x", "?", "21")
    ));
}

fn mot(anglais: &str, sens: Sens) -> Exercice {
    let p = vocabulaire::lexique()
        .iter()
        .find(|p| p.anglais == anglais)
        .unwrap();
    vocabulaire::exercice(p, sens).unwrap()
}

#[test]
fn reponse_en_mot() {
    let aout = mot("august", Sens::VersFrancais);
    for texte in ["AOÛT", "c'est aout", "Août."] {
        assert!(contient_la_reponse(texte, &aout), "{texte}");
    }
    assert!(!contient_la_reponse("le mois d'aoûtat", &aout));
    let nuit = mot("good night", Sens::VersFrancais);
    assert!(contient_la_reponse("on dit Bonne Nuit", &nuit));
    assert!(!contient_la_reponse("une bonne idée, la nuit", &nuit));
}

#[test]
fn nombre_anglais_en_chiffres_donne_la_reponse() {
    let sept = mot("seven", Sens::VersFrancais);
    assert!(contient_la_reponse("c'est 7", &sept));
    assert!(contient_la_reponse("sept", &sept));
    assert!(!contient_la_reponse("17", &sept));
}

#[test]
fn en_lettres_ecrit_les_entiers() {
    for (n, attendu) in [
        (0, "zero"),
        (17, "dix-sept"),
        (21, "vingt-et-un"),
        (71, "soixante-et-onze"),
        (80, "quatre-vingts"),
        (99, "quatre-vingt-dix-neuf"),
        (200, "deux-cents"),
        (201, "deux-cent-un"),
        (1000, "mille"),
        (2026, "deux-mille-vingt-six"),
    ] {
        assert_eq!(en_lettres(n).unwrap(), attendu);
    }
    assert!(en_lettres(1_000_000).is_none());
}

#[test]
fn reperes_relus_couvrent_des_mots_du_lexique() {
    for (famille, table) in reperes() {
        for mot in table.keys() {
            assert!(
                vocabulaire::lexique()
                    .iter()
                    .any(|p| p.famille == *famille && p.anglais == *mot),
                "{mot}"
            );
        }
    }
}

/// Les paliers, le filtre et l'écriture en lettres : mêmes sorties que Python, sur plus de
/// deux mille exercices tirés par lui.
#[test]
fn oracle_python() {
    for cas in commun::oracle("indices") {
        let e = &cas["entree"];
        // Écart assumé : les chiffres d'autres écritures ne sont pas des chiffres.
        if e.to_string()
            .chars()
            .any(|c| c.is_numeric() && !c.is_ascii_digit())
        {
            continue;
        }
        let exercice = || serde_json::from_value::<Exercice>(e["exercice"].clone()).unwrap();
        let sortie = match e["f"].as_str().unwrap() {
            "indices" => json!(indices(&exercice())),
            "en_lettres" => json!(en_lettres(e["n"].as_u64().unwrap() as u32)),
            "contient" => json!(contient_la_reponse(
                e["texte"].as_str().unwrap(),
                &exercice()
            )),
            autre => panic!("fonction inconnue : {autre}"),
        };
        assert_eq!(sortie, cas["sortie"], "{e}");
    }
}
