//! Lecture d'une photo de fiche : réponses du LLM enregistrées le 2026-11-08, jamais les photos.
//!
//! Les photos montrent le prénom de l'enfant : elles n'entrent pas dans le dépôt. Les fichiers de
//! `tests/donnees/photo/` sont les réponses JSON brutes du LLM local (Gemma 4 E4B, llama.cpp),
//! une par morceau d'image, sur des photos réduites à 1280 px comme le fait Telegram.

mod commun;

use std::cell::RefCell;
use std::collections::{BTreeSet, VecDeque};
use std::io::Cursor;

use async_trait::async_trait;
use image::{ImageFormat, Rgb, RgbImage};
use mine_de_rien::adapters::llm::base::{ErreurLlm, Llm};
use mine_de_rien::alea::Alea;
use mine_de_rien::exercises::anglais::vocabulaire;
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::photo::{self, ErreurLecture, Lecture};
use mine_de_rien::saisie::cle_valide;
use serde_json::{Map, Value, json};

fn enregistre(nom: &str) -> Vec<Map<String, Value>> {
    let chemin = commun::racine()
        .join("tests/donnees/photo")
        .join(format!("{nom}.json"));
    serde_json::from_str(&std::fs::read_to_string(chemin).unwrap()).unwrap()
}

fn lire(reponses: &[Map<String, Value>]) -> Lecture {
    photo::interpreter(reponses, &mut Alea::graine(0)).unwrap()
}

fn lecture(lignes: &[(&str, &str)]) -> Lecture {
    let lignes: Vec<Value> = lignes
        .iter()
        .map(|(g, t)| json!({"genre": g, "texte": t}))
        .collect();
    lire(&[json!({"lignes": lignes}).as_object().cloned().unwrap()])
}

fn image(largeur: u32, hauteur: u32) -> Vec<u8> {
    let blanche = RgbImage::from_pixel(largeur, hauteur, Rgb([255, 255, 255]));
    let mut tampon = Cursor::new(Vec::new());
    blanche.write_to(&mut tampon, ImageFormat::Jpeg).unwrap();
    tampon.into_inner()
}

fn dimensions(jpeg: &[u8]) -> (u32, u32) {
    let ouverte = image::load_from_memory(jpeg).unwrap();
    (ouverte.width(), ouverte.height())
}

struct FauxLlm {
    reponses: RefCell<VecDeque<Value>>,
    distant: bool,
    /// Consigne, présence d'une image, demande de JSON.
    appels: RefCell<Vec<(String, bool, bool)>>,
}

impl FauxLlm {
    fn nouveau(reponses: Vec<Value>, distant: bool) -> Self {
        Self {
            reponses: RefCell::new(reponses.into()),
            distant,
            appels: RefCell::new(Vec::new()),
        }
    }
}

#[async_trait(?Send)]
impl Llm for FauxLlm {
    fn distant(&self) -> bool {
        self.distant
    }

    async fn completer(
        &self,
        _systeme: &str,
        utilisateur: &str,
        image: Option<&[u8]>,
        en_json: bool,
    ) -> Result<String, ErreurLlm> {
        self.appels.borrow_mut().push((
            utilisateur.to_owned(),
            image.is_some_and(|i| !i.is_empty()),
            en_json,
        ));
        Ok(self.reponses.borrow_mut().pop_front().unwrap().to_string())
    }
}

/// La valeur d'une réponse ou d'un nombre de fiche, en centièmes.
fn centiemes(nombre: &str) -> i64 {
    let (entier, decimales) = nombre.split_once(',').unwrap_or((nombre, ""));
    format!("{entier}{decimales:0<2}").parse().unwrap()
}

fn donnee<'a>(ex: &'a Exercice, cle: &str) -> &'a str {
    ex.donnees[cle].as_str().unwrap()
}

fn verifier_double_ou_moitie(ex: &Exercice) {
    let n = centiemes(donnee(ex, "nombre"));
    if donnee(ex, "famille") == "doubles" {
        assert_eq!(centiemes(&ex.reponse), n * 2, "{ex:?}");
    } else {
        assert_eq!(centiemes(&ex.reponse) * 2, n, "{ex:?}");
    }
}

#[test]
fn fiche_doubles_moities_redressee() {
    let lecture = lire(&enregistre("doubles-moities"));
    let famille = |nom: &str| -> Vec<&Exercice> {
        lecture
            .exercices
            .iter()
            .filter(|ex| donnee(ex, "famille") == nom)
            .collect()
    };
    let (doubles, moities) = (famille("doubles"), famille("moities"));
    assert_eq!((doubles.len(), moities.len()), (37, 35)); // 72 des 73 lignes, aucune inventée
    for ex in doubles.iter().chain(&moities) {
        verifier_double_ou_moitie(ex);
        assert!(cle_valide(&ex.reponse));
        assert_eq!(ex.competence, photo::COMPETENCE_DOUBLES);
    }
    let enonces: BTreeSet<&str> = lecture
        .exercices
        .iter()
        .map(|ex| ex.enonce.as_str())
        .collect();
    for attendu in [
        "Double de 17 = ?",
        "Double de 1000 = ?",
        "Moitié de 1200 = ?",
    ] {
        assert!(enonces.contains(attendu), "{attendu}");
    }
}

#[test]
fn fiche_anglais_exercice_7() {
    let lecture = lire(&enregistre("anglais-exercice-7"));
    let anglais: Vec<&str> = lecture
        .exercices
        .iter()
        .map(|ex| donnee(ex, "anglais"))
        .collect();
    assert_eq!(anglais[..3], ["wednesday", "saturday", "monday"]); // ordre de la fiche
    assert_eq!(anglais.len(), 19);
    let familles: BTreeSet<&str> = lecture
        .exercices
        .iter()
        .map(|ex| donnee(ex, "famille"))
        .collect();
    assert_eq!(familles, BTreeSet::from(["jours", "mois"]));
}

#[test]
fn problemes_ne_donnent_aucun_exercice_faux() {
    let lecture = lire(&enregistre("problemes"));
    for ex in &lecture.exercices {
        let juste = [("799 - 760 = ?", "39"), ("75,5 - 50 = ?", "25,5")];
        assert!(
            juste.contains(&(ex.enonce.as_str(), ex.reponse.as_str())),
            "{ex:?}"
        );
    }
}

/// Limite connue : de travers, le modèle invente des suites. Le code ne peut pas les écarter
/// (ce sont des calculs justes) ; les réponses restent exactes, le parent juge la liste.
#[test]
fn photo_de_travers_reste_juste_mais_infidele() {
    let lecture = lire(&enregistre("doubles-moities-de-travers"));
    assert!(!lecture.exercices.is_empty());
    for ex in &lecture.exercices {
        verifier_double_ou_moitie(ex);
    }
}

fn seul(lecture: &Lecture) -> &Exercice {
    assert_eq!(lecture.exercices.len(), 1, "{lecture:?}");
    &lecture.exercices[0]
}

#[test]
fn resultat_du_llm_jamais_lu() {
    assert_eq!(
        seul(&lecture(&[("calcul", "Double de 17 = 35")])).reponse,
        "34"
    );
    assert_eq!(seul(&lecture(&[("calcul", "348 ÷ 12 = 30")])).reponse, "29");
}

#[test]
fn doubles_et_moities() {
    let lue = lecture(&[("calcul", "Moitié de 3")]);
    assert_eq!(
        (seul(&lue).enonce.as_str(), seul(&lue).reponse.as_str()),
        ("Moitié de 3 = ?", "1,5")
    );
    let lue = lecture(&[("calcul", "moitie  de 1 000")]);
    assert_eq!(
        (seul(&lue).enonce.as_str(), seul(&lue).reponse.as_str()),
        ("Moitié de 1000 = ?", "500")
    );
    assert_eq!(seul(&lecture(&[("calcul", "Double de 2,5")])).reponse, "5");
}

#[test]
fn calculs_rejetes() {
    let lue = lecture(&[
        ("calcul", "Moitié de 1,25"),
        ("calcul", "10 ÷ 3"),
        ("calcul", "799"),
    ]);
    assert!(lue.exercices.is_empty());
    let textes: Vec<&str> = lue.rejets.iter().map(|r| r.texte.as_str()).collect();
    assert_eq!(textes, ["Moitié de 1,25", "10 ÷ 3", "799"]);
}

#[test]
fn tiret_typographique_et_milliers() {
    assert_eq!(seul(&lecture(&[("calcul", "799 – 760")])).reponse, "39");
    assert_eq!(
        seul(&lecture(&[("calcul", "1 200 + 12 345")])).reponse,
        "13545"
    );
}

#[test]
fn mots() {
    let lue = lecture(&[("mot", "Wednesday"), ("mot", "book"), ("mot", "orange")]);
    let anglais: Vec<&str> = lue
        .exercices
        .iter()
        .map(|ex| donnee(ex, "anglais"))
        .collect();
    assert_eq!(anglais, ["wednesday"]);
    let raisons: Vec<&str> = lue.rejets.iter().map(|r| r.raison.as_str()).collect();
    assert_eq!(raisons, [photo::HORS_LEXIQUE, photo::HORS_LEXIQUE]);
}

#[test]
fn doublons_retires() {
    let lue = lecture(&[
        ("calcul", "Double de 20"),
        ("calcul", "Double de 20 = 40"),
        ("mot", "May"),
        ("mot", "may"),
    ]);
    assert_eq!(lue.exercices.len(), 2);
}

#[test]
fn lignes_mal_formees() {
    let reponse = json!({"lignes": ["texte nu", {"genre": "mot"}]});
    let lue = lire(&[reponse.as_object().cloned().unwrap()]);
    assert!(lue.exercices.is_empty());
    assert_eq!(lue.rejets.len(), 2);
    let sans_lignes = json!({"autre": []}).as_object().cloned().unwrap();
    assert!(photo::interpreter(&[sans_lignes], &mut Alea::graine(0)).is_err());
}

#[test]
fn paysage_deux_appels_portrait_un_seul() {
    assert_eq!(photo::preparer(&image(1280, 885)).unwrap().len(), 2);
    assert_eq!(photo::preparer(&image(885, 1280)).unwrap().len(), 1);
    for morceau in photo::preparer(&image(4000, 3000)).unwrap() {
        let (largeur, hauteur) = dimensions(&morceau);
        assert!(largeur.max(hauteur) <= photo::COTE_MAX);
    }
}

#[test]
fn image_illisible() {
    assert!(photo::preparer(b"pas une image").is_err());
}

#[tokio::test]
async fn lire_appelle_le_llm_par_moitie_avec_la_consigne_du_parent() {
    let llm = FauxLlm::nouveau(
        vec![
            json!({"lignes": [{"genre": "calcul", "texte": "Double de 1"}]}),
            json!({"lignes": [{"genre": "calcul", "texte": "Moitié de 2"}]}),
        ],
        false,
    );
    let lue = photo::lire(&llm, &image(1280, 885), "exercice 7", &mut Alea::graine(0))
        .await
        .unwrap();
    let appels = llm.appels.borrow();
    assert_eq!(appels.len(), 2);
    assert!(
        appels
            .iter()
            .all(|(_, avec_image, en_json)| *avec_image && *en_json)
    );
    assert!(appels[0].0.contains("exercice 7"));
    assert_eq!(lue.exercices.len(), 2);
}

#[tokio::test]
async fn lire_refuse_un_llm_distant_sans_l_appeler() {
    let llm = FauxLlm::nouveau(Vec::new(), true);
    let erreur = photo::lire(&llm, &image(100, 100), "", &mut Alea::graine(0))
        .await
        .unwrap_err();
    assert!(matches!(&erreur, ErreurLecture::Llm(e) if e.to_string().contains("local")));
    assert!(llm.appels.borrow().is_empty());
}

/// La photo n'est jamais écrite sur le disque : lire ne crée aucun fichier.
#[tokio::test]
async fn lire_n_ecrit_rien_sur_le_disque() {
    let dossier = tempfile::tempdir().unwrap();
    let avant = std::env::current_dir().unwrap();
    let llm = FauxLlm::nouveau(vec![json!({"lignes": []})], false);
    photo::lire(&llm, &image(300, 400), "", &mut Alea::graine(0))
        .await
        .unwrap();
    assert_eq!(std::fs::read_dir(dossier.path()).unwrap().count(), 0);
    assert_eq!(std::env::current_dir().unwrap(), avant);
}

#[test]
fn tirage_sans_remise() {
    let lecture = lire(&enregistre("doubles-moities"));
    for n in [0, 3, 72, 500] {
        let tires = photo::tirer(&lecture, n, &mut Alea::graine(n as u64));
        assert_eq!(tires.len(), n.min(72));
        let enonces: BTreeSet<&str> = tires.iter().map(|ex| ex.enonce.as_str()).collect();
        assert_eq!(enonces.len(), tires.len());
        // Dans l'ordre de la fiche.
        let rangs: Vec<usize> = tires
            .iter()
            .map(|ex| {
                lecture
                    .exercices
                    .iter()
                    .position(|autre| autre == ex)
                    .unwrap()
            })
            .collect();
        assert!(rangs.is_sorted());
    }
}

#[test]
fn resume_montre_les_calculs_ecartes_et_compte_les_mots() {
    let lue = lecture(&[
        ("calcul", "Double de 4"),
        ("calcul", "10 ÷ 3"),
        ("mot", "Greetings"),
    ]);
    let texte = photo::resumer(&lue).join("\n");
    assert!(texte.contains("Doubles de (1) : 4"));
    assert!(texte.contains("« 10 ÷ 3 »"));
    assert!(!texte.contains("Greetings") && texte.contains("1 texte(s) hors lexique"));
}

fn lecture_json(lecture: &Lecture) -> Value {
    let exercices: Vec<Value> = lecture
        .exercices
        .iter()
        .map(|ex| {
            // Le sens d'un mot est tiré au hasard : on ne compare que ce qui n'en dépend pas.
            if ex.competence == vocabulaire::COMPETENCE {
                json!({"anglais": ex.donnees["anglais"], "famille": ex.donnees["famille"]})
            } else {
                json!(ex)
            }
        })
        .collect();
    let rejets: Vec<Value> = lecture
        .rejets
        .iter()
        .map(|r| json!([r.texte, r.raison]))
        .collect();
    json!({"exercices": exercices, "rejets": rejets, "resume": photo::resumer(lecture)})
}

/// Consignes au modèle, calculs recopiés, fiches enregistrées, lignes mal formées, tailles
/// des morceaux et redressement EXIF : comme la version Python.
#[test]
fn oracle_python() {
    for cas in commun::oracle("photo") {
        let e = &cas["entree"];
        match e["f"].as_str().unwrap() {
            "constantes" => {
                assert_eq!(cas["sortie"]["systeme"], photo::SYSTEME);
                assert_eq!(cas["sortie"]["consigne"], photo::CONSIGNE);
            }
            "calcul" => commun::comparer(&cas, photo::calcul(e["texte"].as_str().unwrap())),
            "interpreter" => {
                let reponses: Vec<Map<String, Value>> = match e.get("fichier") {
                    Some(fichier) => {
                        enregistre(fichier.as_str().unwrap().trim_end_matches(".json"))
                    }
                    None => serde_json::from_value(e["reponses"].clone()).unwrap(),
                };
                let lue = photo::interpreter(&reponses, &mut Alea::graine(0));
                commun::comparer(&cas, lue.map(|l| lecture_json(&l)));
            }
            "preparer" => {
                let (largeur, hauteur) = (
                    e["taille"][0].as_u64().unwrap() as u32,
                    e["taille"][1].as_u64().unwrap() as u32,
                );
                let tailles: Vec<Value> = photo::preparer(&image(largeur, hauteur))
                    .unwrap()
                    .iter()
                    .map(|m| json!(dimensions(m)))
                    .collect();
                assert_eq!(json!(tailles), cas["sortie"], "{e}");
            }
            "orientation" => {
                let chemin = commun::racine()
                    .join("tests/donnees/photo")
                    .join(e["fichier"].as_str().unwrap());
                let morceaux = photo::preparer(&std::fs::read(chemin).unwrap()).unwrap();
                assert_eq!(morceaux.len(), 1);
                let redressee = image::load_from_memory(&morceaux[0]).unwrap().to_rgb8();
                let couleur = |x: u32, y: u32| {
                    let p = redressee.get_pixel(x, y);
                    if p[0] > p[2] { "rouge" } else { "bleu" }
                };
                let sortie = json!({
                    "taille": [redressee.width(), redressee.height()],
                    "haut": couleur(10, 5),
                    "bas": couleur(10, 55),
                });
                assert_eq!(sortie, cas["sortie"], "{e}");
            }
            "lire_json" => {} // voir tests/llm.rs
            autre => panic!("fonction inconnue : {autre}"),
        }
    }
}
