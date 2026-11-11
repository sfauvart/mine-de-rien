//! Du texte de la dictée aux trous : propriétés sur 500 graines et sur les dictées 5 et 6.

use std::collections::{HashMap, HashSet};

use mine_de_rien::alea::Alea;
use mine_de_rien::exercises::francais::dictee::{
    COMPETENCE, Phrase, Position, TROU, candidats, decouper, decouper_liste, est_candidat,
    exercice, imposes_absents, phrase_a_trou, tirer,
};
use mine_de_rien::minecraft::texte::lignes_panneau;
use mine_de_rien::saisie::{cle_dictee_valide, cle_valide};

const DICTEE_5: &str = "Deux filles et leur père emménagent dans une vieille maison de \
    campagne, au Japon. Mei et Satsuki découvrent que des noiraudes habitent les coins sombres. \
    Elles rient aux éclats pour les faire fuir.";
const DICTEE_6: &str = "Dans la ville de Dubaï, aux Émirats arabes unis, s’élève la plus haute \
    tour jamais construite. Burj Khalifa culmine à huit-cent-vingt-huit mètres de hauteur, \
    c’est presque trois tours Eiffel. Deux-millions de personnes la visitent chaque année.";

const GRAINES: u64 = 500;

fn mot(phrases: &[Phrase], p: Position) -> &str {
    &phrases[p.phrase].mots[p.mot].texte
}

fn mots_candidats(phrases: &[Phrase]) -> Vec<&str> {
    candidats(phrases)
        .into_iter()
        .map(|p| mot(phrases, p))
        .collect()
}

fn sans_poids() -> HashMap<Position, f64> {
    HashMap::new()
}

#[test]
fn decoupage_en_phrases() {
    let phrases = decouper(DICTEE_6);
    assert_eq!(phrases.len(), 3);
    assert_eq!(
        phrases[0].texte,
        "Dans la ville de Dubaï, aux Émirats arabes unis, s'élève la plus haute tour jamais \
         construite."
    );
    assert!(phrases[1].texte.starts_with("Burj Khalifa culmine"));
    assert_eq!(
        phrases[2].texte,
        "Deux-millions de personnes la visitent chaque année."
    );
    assert_eq!(decouper(DICTEE_5).len(), 3);
}

#[test]
fn ponctuation_et_espaces_du_decoupage() {
    let textes: Vec<String> =
        decouper("  Viens !  Où vas-tu ?\nJe ne sais pas… « Reste ici. » Il coûte 3.50 euros. Fin")
            .into_iter()
            .map(|p| p.texte)
            .collect();
    assert_eq!(
        textes,
        [
            "Viens !",
            "Où vas-tu ?",
            "Je ne sais pas…",
            "« Reste ici. »",
            "Il coûte 3.50 euros.",
            "Fin",
        ]
    );
    assert!(decouper("").is_empty());
    assert!(decouper(" ... !? ").is_empty());
}

#[test]
fn apostrophe_typographique_devient_celle_du_clavier() {
    let phrases = decouper("L’école s’élève. C`est l‘heure.");
    assert_eq!(phrases[0].texte, "L'école s'élève.");
    assert_eq!(phrases[1].texte, "C'est l'heure.");
    let mots: Vec<&str> = phrases[0].mots.iter().map(|m| m.texte.as_str()).collect();
    assert_eq!(mots, ["L'école", "s'élève"]);
}

#[test]
fn mots_et_positions_en_caracteres() {
    let phrases = decouper("Été à Dubaï : huit-cent-vingt-huit mètres, c'est haut - très.");
    let phrase = &phrases[0];
    let mots: Vec<&str> = phrase.mots.iter().map(|m| m.texte.as_str()).collect();
    assert_eq!(
        mots,
        [
            "Été",
            "à",
            "Dubaï",
            "huit-cent-vingt-huit",
            "mètres",
            "c'est",
            "haut",
            "très"
        ]
    );
    for m in &phrase.mots {
        let extrait: String = phrase
            .texte
            .chars()
            .skip(m.debut)
            .take(m.fin - m.debut)
            .collect();
        assert_eq!(extrait, m.texte);
    }
}

#[test]
fn candidats_des_dictees_reelles() {
    let phrases = decouper(DICTEE_6);
    let mots = mots_candidats(&phrases);
    for attendu in [
        "Dans",
        "la",
        "ville",
        "de",
        "Dubaï",
        "s'élève",
        "plus",
        "haute",
        "tour",
        "jamais",
        "huit-cent-vingt-huit",
        "mètres",
        "hauteur",
        "c'est",
        "presque",
        "trois",
        "tours",
        "Deux-millions",
        "personnes",
        "visitent",
        "chaque",
        "année",
    ] {
        assert!(
            mots.contains(&attendu),
            "{attendu} devrait être un trou possible"
        );
    }
    // Majuscule accentuée, et mot d'une lettre.
    assert!(!mots.contains(&"Émirats"));
    assert!(!mots.contains(&"à"));

    let cinq = decouper(DICTEE_5);
    let mots = mots_candidats(&cinq);
    for attendu in [
        "filles",
        "et",
        "emménagent",
        "campagne",
        "Japon",
        "Mei",
        "rient",
        "pour",
    ] {
        assert!(mots.contains(&attendu), "{attendu}");
    }
}

#[test]
fn regles_des_candidats() {
    for bon in [
        "la",
        "Dans",
        "Dubaï",
        "s'élève",
        "aujourd'hui",
        "œuf",
        "où",
        "Eiffel",
    ] {
        assert!(est_candidat(bon), "{bon}");
    }
    let trop_long = "a".repeat(51);
    for mauvais in [
        "à",
        "y",
        "Émirats",
        "École",
        "CM2",
        "828",
        "3e",
        "l'",
        trop_long.as_str(),
    ] {
        assert!(!est_candidat(mauvais), "{mauvais}");
    }
    assert!(est_candidat(&"a".repeat(50)));
    // Un mot avec chiffre n'est pas un trou, mais il reste dans la phrase.
    let phrases = decouper("Il a 10 ans en CM2.");
    assert_eq!(mots_candidats(&phrases), ["Il", "ans", "en"]);
    assert_eq!(phrases[0].mots.len(), 6);
}

#[test]
fn cle_de_dictee() {
    for bonne in [
        "Dans",
        "Dubaï",
        "s'élève",
        "huit-cent-vingt-huit",
        "année",
        "Émirats",
        "œil",
    ] {
        assert!(cle_dictee_valide(bonne), "{bonne}");
    }
    let trop_longue = "é".repeat(51);
    for mauvaise in [
        "",
        " plus",
        "plus ",
        "chaque année",
        "s’élève",
        "-tour",
        "tour-",
        "l''eau",
        "a--b",
        "tour2",
        "tour.",
        trop_longue.as_str(),
    ] {
        assert!(!cle_dictee_valide(mauvaise), "{mauvaise:?}");
    }
    // 50 caractères accentués : la limite se compte en caractères, pas en octets.
    assert!(cle_dictee_valide(&"é".repeat(50)));
    // Les clés des autres coffres ne changent pas : jamais de majuscule ni d'apostrophe.
    assert!(!cle_valide("Dans"));
    assert!(!cle_valide("s'élève"));
    assert!(cle_valide("mercredi"));
}

#[test]
fn proprietes_des_exercices_sur_500_graines() {
    for texte in [DICTEE_5, DICTEE_6] {
        let phrases = decouper(texte);
        let mut vus = HashSet::new();
        for graine in 0..GRAINES {
            let tires = tirer(&phrases, 4, &mut Alea::graine(graine), &sans_poids(), &[]).unwrap();
            assert_eq!(tires.len(), 4);
            for position in tires {
                vus.insert(position);
                let ex = exercice(&phrases, position).unwrap();
                assert_eq!(ex.competence, COMPETENCE);
                assert_eq!(ex.reponse, mot(&phrases, position));
                assert!(cle_dictee_valide(&ex.reponse), "{ex:?}");
                assert!(lignes_panneau(&ex.enonce).is_some(), "{ex:?}");
                assert!(ex.enonce.starts_with("Dictée : "), "{ex:?}");
                assert!(ex.enonce.contains(TROU));
                // La clé n'est pas un mot de l'énoncé.
                assert!(
                    !ex.enonce
                        .split(' ')
                        .any(|m| m.to_lowercase() == ex.reponse.to_lowercase()),
                    "{ex:?}"
                );
                // La phrase à trou redonne la phrase d'origine, et le trou a une longueur fixe.
                let a_trou = ex.donnees["a_trou"].as_str().unwrap();
                assert_eq!(a_trou.matches(TROU).count(), 1, "{a_trou}");
                assert_eq!(
                    a_trou.replacen(TROU, &ex.reponse, 1),
                    phrases[position.phrase].texte
                );
                assert_eq!(ex.donnees["phrase"], position.phrase);
                assert_eq!(ex.donnees["mot"], position.mot);
                assert_eq!(ex.donnees["lettres"], ex.reponse.chars().count());
                assert!(ex.accepte(&ex.reponse));
            }
        }
        // Tout le texte sert : chaque candidat finit par sortir.
        assert_eq!(vus.len(), candidats(&phrases).len());
    }
}

#[test]
fn meme_graine_meme_tirage() {
    let phrases = decouper(DICTEE_6);
    for graine in 0..50 {
        let a = tirer(&phrases, 5, &mut Alea::graine(graine), &sans_poids(), &[]).unwrap();
        let b = tirer(&phrases, 5, &mut Alea::graine(graine), &sans_poids(), &[]).unwrap();
        assert_eq!(a, b);
    }
}

#[test]
fn sans_remise_et_une_phrase_a_la_fois() {
    let phrases = decouper(DICTEE_6);
    let total = candidats(&phrases).len();
    for graine in 0..GRAINES {
        let mut alea = Alea::graine(graine);
        let trois = tirer(&phrases, 3, &mut alea, &sans_poids(), &[]).unwrap();
        let phrases_touchees: HashSet<usize> = trois.iter().map(|p| p.phrase).collect();
        assert_eq!(phrases_touchees.len(), 3, "trois trous, trois phrases");
        let tout = tirer(&phrases, 100, &mut alea, &sans_poids(), &[]).unwrap();
        assert_eq!(tout.len(), total);
        assert_eq!(tout.iter().collect::<HashSet<_>>().len(), total);
    }
}

#[test]
fn enonce_sans_voisin_trop_long_ni_identique() {
    let phrases = decouper("Il mesure huit-cent-vingt-huit mètres. Nous nous aimons. Oui.");
    let enonce = |p, m| {
        exercice(&phrases, Position { phrase: p, mot: m })
            .unwrap()
            .enonce
    };
    assert_eq!(enonce(0, 1), "Dictée : Il _____");
    assert_eq!(enonce(0, 2), "Dictée : mesure _____ mètres");
    assert_eq!(enonce(0, 3), "Dictée : _____");
    // « nous » ne s'affiche pas à côté du trou « Nous ».
    assert_eq!(enonce(1, 0), "Dictée : _____");
    assert_eq!(enonce(1, 1), "Dictée : _____ aimons");
    assert_eq!(enonce(2, 0), "Dictée : _____");
}

#[test]
fn mot_repete_le_trou_est_a_sa_position() {
    let phrases = decouper("La tour, la plus haute tour.");
    let phrase = &phrases[0];
    assert_eq!(
        phrase_a_trou(phrase, &phrase.mots[1]),
        "La _____, la plus haute tour."
    );
    assert_eq!(
        phrase_a_trou(phrase, &phrase.mots[5]),
        "La tour, la plus haute _____."
    );
    assert_eq!(
        phrase_a_trou(phrase, &phrase.mots[2]),
        "La tour, _____ plus haute tour."
    );
}

#[test]
fn mots_imposes() {
    let phrases = decouper(DICTEE_6);
    let imposes: Vec<String> = ["plus", "trois", "Personnes"].map(String::from).to_vec();
    assert!(imposes_absents(&phrases, &imposes).is_empty());
    for graine in 0..GRAINES {
        let tires = tirer(
            &phrases,
            4,
            &mut Alea::graine(graine),
            &sans_poids(),
            &imposes,
        )
        .unwrap();
        let mots: Vec<&str> = tires.iter().map(|&p| mot(&phrases, p)).collect();
        // Les trois imposés sortent tous, le quatrième est tiré au hasard ; le tout dans
        // l'ordre du texte.
        let tous: HashSet<&str> = mots.iter().copied().collect();
        assert_eq!(tous.len(), 4, "{mots:?}");
        assert!(
            tous.is_superset(&HashSet::from(["plus", "trois", "personnes"])),
            "{mots:?}"
        );
        assert!(tires.is_sorted(), "{tires:?}");
    }
    // Moins de coffres que de mots imposés : seulement des imposés.
    let deux = tirer(&phrases, 2, &mut Alea::graine(1), &sans_poids(), &imposes).unwrap();
    assert!(
        deux.iter()
            .all(|&p| ["plus", "trois", "personnes"].contains(&mot(&phrases, p)))
    );
}

#[test]
fn mot_impose_absent_ou_impossible_est_signale() {
    let phrases = decouper(DICTEE_6);
    let imposes: Vec<String> = ["plus", "girafe", "Émirats", "à"]
        .map(String::from)
        .to_vec();
    assert_eq!(
        imposes_absents(&phrases, &imposes),
        ["girafe", "Émirats", "à"]
    );
    let erreur = tirer(&phrases, 4, &mut Alea::graine(1), &sans_poids(), &imposes).unwrap_err();
    assert_eq!(
        erreur.0,
        "Pas dans le texte, ou impossible à demander : girafe, Émirats, à."
    );
}

#[test]
fn les_poids_orientent_le_tirage() {
    let phrases = decouper(DICTEE_6);
    let tous = candidats(&phrases);
    let position = |texte: &str| *tous.iter().find(|&&p| mot(&phrases, p) == texte).unwrap();
    let lourd = position("personnes");
    let temoin = position("visitent");
    let ecarte = position("chaque");
    let poids = HashMap::from([(lourd, 10.0), (ecarte, 0.0)]);
    let (mut n_lourd, mut n_temoin) = (0, 0);
    for graine in 0..GRAINES {
        let tires = tirer(&phrases, 1, &mut Alea::graine(graine), &poids, &[]).unwrap();
        assert_ne!(tires[0], ecarte, "un poids nul ne sort jamais");
        n_lourd += usize::from(tires[0] == lourd);
        n_temoin += usize::from(tires[0] == temoin);
    }
    assert!(n_lourd > 4 * n_temoin.max(1), "{n_lourd} contre {n_temoin}");

    // Même imposé, un mot de poids nul ne sort pas.
    let impose = vec!["chaque".to_owned()];
    for graine in 0..50 {
        let tires = tirer(&phrases, 100, &mut Alea::graine(graine), &poids, &impose).unwrap();
        assert!(!tires.contains(&ecarte));
        assert_eq!(tires.len(), tous.len() - 1);
    }
}

#[test]
fn tout_ecarte_ou_rien_a_tirer() {
    let phrases = decouper(DICTEE_5);
    let aucun: HashMap<Position, f64> = candidats(&phrases).into_iter().map(|p| (p, 0.0)).collect();
    assert!(
        tirer(&phrases, 4, &mut Alea::graine(1), &aucun, &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        tirer(&[], 4, &mut Alea::graine(1), &sans_poids(), &[])
            .unwrap()
            .is_empty()
    );
    assert!(
        tirer(&phrases, 0, &mut Alea::graine(1), &sans_poids(), &[])
            .unwrap()
            .is_empty()
    );
    let bizarres = HashMap::from([
        (candidats(&phrases)[0], f64::NAN),
        (candidats(&phrases)[1], -3.0),
    ]);
    let tires = tirer(&phrases, 100, &mut Alea::graine(1), &bizarres, &[]).unwrap();
    assert_eq!(tires.len(), candidats(&phrases).len() - 2);
}

#[test]
fn liste_de_mots_sans_texte() {
    let phrases = decouper_liste("plus, trois ;personnes\nchaque année,, à , ");
    let textes: Vec<&str> = phrases.iter().map(|p| p.texte.as_str()).collect();
    assert_eq!(textes, ["plus", "trois", "personnes", "chaque année", "à"]);
    assert_eq!(
        mots_candidats(&phrases),
        ["plus", "trois", "personnes", "chaque", "année"]
    );
    let ex = exercice(&phrases, Position { phrase: 0, mot: 0 }).unwrap();
    assert_eq!(
        (ex.enonce.as_str(), ex.reponse.as_str()),
        ("Dictée : _____", "plus")
    );
    assert_eq!(ex.donnees["a_trou"], TROU);
}

#[test]
fn liste_sans_virgule_coupee_aux_espaces() {
    let textes = |liste: &str| -> Vec<String> {
        decouper_liste(liste).into_iter().map(|p| p.texte).collect()
    };
    assert_eq!(
        textes(" tour  seules personnes "),
        ["tour", "seules", "personnes"]
    );
    // Dès qu'un séparateur est là, un élément garde ses espaces.
    assert_eq!(textes("tout à coup, tour"), ["tout à coup", "tour"]);
    assert_eq!(textes("tout à coup,"), ["tout à coup"]);
}

#[test]
fn trous_tires_dans_l_ordre_du_texte() {
    let phrases = decouper("Le renard traverse la grande forêt. Il cherche une tanière profonde.");
    for graine in 0..200 {
        let tires = tirer(&phrases, 4, &mut Alea::graine(graine), &HashMap::new(), &[]).unwrap();
        assert_eq!(tires.len(), 4);
        assert!(tires.is_sorted(), "graine {graine} : {tires:?}");
    }
    // Un mot imposé sort toujours, mais à sa place dans le texte.
    let imposes = ["profonde".to_owned()];
    for graine in 0..200 {
        let tires = tirer(
            &phrases,
            2,
            &mut Alea::graine(graine),
            &HashMap::new(),
            &imposes,
        )
        .unwrap();
        assert!(tires.is_sorted(), "graine {graine} : {tires:?}");
        assert!(tires.contains(&Position { phrase: 1, mot: 4 }), "{tires:?}");
    }
}

#[test]
fn exercice_refuse_une_position_hors_du_texte_ou_non_candidate() {
    let phrases = decouper("Il va à Paris.");
    assert!(exercice(&phrases, Position { phrase: 3, mot: 0 }).is_err());
    assert!(exercice(&phrases, Position { phrase: 0, mot: 9 }).is_err());
    let erreur = exercice(&phrases, Position { phrase: 0, mot: 2 }).unwrap_err();
    assert_eq!(erreur.0, "« à » ne peut pas être demandé.");
}

#[test]
fn tirage_pondere_de_l_alea() {
    let mut alea = Alea::graine(7);
    assert_eq!(alea.pondere(&[]), None);
    assert_eq!(alea.pondere(&[0.0, -1.0, f64::NAN]), None);
    let mut compte = [0usize; 3];
    for _ in 0..3000 {
        compte[alea.pondere(&[1.0, 0.0, 3.0]).unwrap()] += 1;
    }
    assert_eq!(compte[1], 0);
    assert!(compte[2] > 2 * compte[0] && compte[0] > 500, "{compte:?}");
}
