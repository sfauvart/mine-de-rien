//! La dictée depuis Telegram : préparation par la voix, confirmation, enregistrement, pack,
//! séance. Avec une fausse voix, un faux LLM et une vraie base jetable.

mod commun;

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeDelta, Utc};
use image::{ImageFormat, Rgb, RgbImage};
use mine_de_rien::adapters::channel::base::{Canal, MessageParent};
use mine_de_rien::adapters::llm::base::{ErreurLlm, Llm};
use mine_de_rien::adapters::voix::base::{ErreurVoix, Voix};
use mine_de_rien::alea::Alea;
use mine_de_rien::config::Coffre as ConfigCoffre;
use mine_de_rien::dictees::{
    PHRASES_MAX, ecrire_jour, lire_jour, numero_son, preparer, reconstruire_pack,
};
use mine_de_rien::orchestrateur::{
    AIDE, AIDE_DICTEE, CommandesParents, DICTEES_ABSENTES, LECTURE_EN_COURS, PREPARATION_EN_COURS,
    RAPPEL_DICTEE, RAPPEL_PHOTO_DICTEE, legende_de_dictee, servir,
};
use mine_de_rien::saisie::Source;
use mine_de_rien::son::lire_wav;
use mine_de_rien::storage::sqlite::{Depot, Statut};
use serde_json::{Value, json};

const PARENT: i64 = 111;
const DICTEE_6: &str = "Dans la ville de Dubaï, aux Émirats arabes unis, s’élève la plus haute \
    tour jamais construite. Burj Khalifa culmine à huit-cent-vingt-huit mètres de hauteur, \
    c’est presque trois tours Eiffel. Deux-millions de personnes la visitent chaque année.";

fn jour(a: i32, m: u32, j: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(a, m, j).unwrap()
}

/// Un WAV PCM 16 bits mono à 22 050 Hz : un dixième de seconde par mot du texte.
fn wav_pour(texte: &str, lent: bool) -> Vec<u8> {
    let mots = texte.split_whitespace().count().max(1);
    let trames = mots * 2205 * if lent { 2 } else { 1 };
    let donnees: Vec<u8> = (0..trames)
        .flat_map(|i| (((i % 100) as i16 - 50) * 200).to_le_bytes())
        .collect();
    let mut w = Vec::new();
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36 + donnees.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&22_050u32.to_le_bytes());
    w.extend_from_slice(&44_100u32.to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(donnees.len() as u32).to_le_bytes());
    w.extend_from_slice(&donnees);
    w
}

#[derive(Default)]
struct FausseVoix {
    en_panne: Cell<bool>,
    lus: RefCell<Vec<(String, bool)>>,
}

#[async_trait(?Send)]
impl Voix for FausseVoix {
    async fn dire(&self, texte: &str, lent: bool) -> Result<Vec<u8>, ErreurVoix> {
        if self.en_panne.get() {
            return Err(ErreurVoix(
                "La voix locale est injoignable (essai).".to_owned(),
            ));
        }
        self.lus.borrow_mut().push((texte.to_owned(), lent));
        Ok(wav_pour(texte, lent))
    }
}

struct FauxLlm {
    texte: String,
    distant: bool,
    appels: Cell<usize>,
}

#[async_trait(?Send)]
impl Llm for FauxLlm {
    fn distant(&self) -> bool {
        self.distant
    }

    async fn completer(
        &self,
        _: &str,
        consigne: &str,
        image: Option<&[u8]>,
        _: bool,
    ) -> Result<String, ErreurLlm> {
        self.appels.set(self.appels.get() + 1);
        assert!(image.is_some());
        // La consigne de dictée demande un texte, celle des fiches des lignes.
        Ok(if consigne.contains("\"lignes\"") {
            json!({"lignes": [{"genre": "calcul", "texte": "Double de 4"}]}).to_string()
        } else {
            json!({"texte": self.texte}).to_string()
        })
    }
}

type Temps = Rc<Cell<DateTime<Utc>>>;

struct Monde {
    dossier: tempfile::TempDir,
    depot: Rc<Depot>,
    voix: Rc<FausseVoix>,
    temps: Temps,
    commandes: CommandesParents,
}

impl Monde {
    fn pack(&self) -> PathBuf {
        self.dossier.path().join("mine-de-rien.zip")
    }

    async fn dit(&self, texte: &str) -> String {
        self.commandes
            .traiter(&MessageParent::texte(PARENT, texte))
            .await
            .unwrap()
            .unwrap()
    }

    async fn photo(&self, legende: &str) -> String {
        self.commandes
            .traiter(&MessageParent {
                expediteur: PARENT,
                texte: legende.to_owned(),
                image: Some(jpeg()),
            })
            .await
            .unwrap()
            .unwrap()
    }

    /// Les numéros des sons du pack écrit sur le disque.
    fn sons_du_pack(&self) -> BTreeSet<u32> {
        sons_de(&self.pack())
    }
}

fn sons_de(pack: &Path) -> BTreeSet<u32> {
    let mut zip = zip::ZipArchive::new(Cursor::new(std::fs::read(pack).unwrap())).unwrap();
    (0..zip.len())
        .filter_map(|i| {
            let nom = zip.by_index(i).unwrap().name().to_owned();
            nom.strip_prefix("assets/mine_de_rien/sounds/dictee/")?
                .strip_suffix(".ogg")?
                .parse()
                .ok()
        })
        .collect()
}

fn jpeg() -> Vec<u8> {
    let blanche = RgbImage::from_pixel(800, 300, Rgb([255, 255, 255]));
    let mut tampon = Cursor::new(Vec::new());
    blanche.write_to(&mut tampon, ImageFormat::Jpeg).unwrap();
    tampon.into_inner()
}

/// Dimanche 8 novembre 2026, 18 h UTC : « vendredi » est le 13.
fn monde_avec(llm: Option<Rc<FauxLlm>>, dictees: bool) -> Monde {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Rc::new(Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap());
    let temps: Temps = Rc::new(Cell::new(commun::instant("2026-11-08T18:00:00+00:00")));
    let lu = Rc::clone(&temps);
    let voix = Rc::new(FausseVoix::default());
    let mut commandes = CommandesParents::new(
        depot.clone(),
        ConfigCoffre::default(),
        None,
        &[PARENT, 222],
        Box::new(move || lu.get()),
        llm.map(|llm| llm as Rc<dyn Llm>),
        Alea::graine(0),
    );
    if dictees {
        commandes = commandes.avec_dictees(voix.clone(), &dossier.path().join("mine-de-rien.zip"));
    }
    Monde {
        dossier,
        depot,
        voix,
        temps,
        commandes,
    }
}

fn monde() -> Monde {
    monde_avec(None, true)
}

fn llm(texte: &str, distant: bool) -> Rc<FauxLlm> {
    Rc::new(FauxLlm {
        texte: texte.to_owned(),
        distant,
        appels: Cell::new(0),
    })
}

#[test]
fn le_jour_de_la_dictee() {
    let dimanche = jour(2026, 11, 8);
    assert_eq!(lire_jour("vendredi", dimanche).unwrap(), jour(2026, 11, 13));
    assert_eq!(
        lire_jour(" Vendredi ", dimanche).unwrap(),
        jour(2026, 11, 13)
    );
    assert_eq!(lire_jour("lundi", dimanche).unwrap(), jour(2026, 11, 9));
    // Un nom de jour désigne le prochain, jamais aujourd'hui.
    assert_eq!(lire_jour("dimanche", dimanche).unwrap(), jour(2026, 11, 15));
    assert_eq!(
        lire_jour("vendredi", jour(2026, 11, 10)).unwrap(),
        jour(2026, 11, 13)
    );
    assert_eq!(
        lire_jour("vendredi", jour(2026, 11, 13)).unwrap(),
        jour(2026, 11, 20)
    );
    assert_eq!(lire_jour("13/11", dimanche).unwrap(), jour(2026, 11, 13));
    assert_eq!(lire_jour("3/12", dimanche).unwrap(), jour(2026, 12, 3));
    // En décembre, janvier est celui de l'année suivante.
    assert_eq!(
        lire_jour("08/01", jour(2026, 12, 20)).unwrap(),
        jour(2027, 1, 8)
    );
    for passe in ["07/11", "8/11", "01/09"] {
        let erreur = lire_jour(passe, dimanche).unwrap_err();
        assert!(erreur.0.contains("déjà passé"), "{passe} : {erreur}");
    }
    for incompris in [
        "",
        "bientôt",
        "32/11",
        "13/13",
        "13-11",
        "13/11/2026",
        "a/b",
    ] {
        let erreur = lire_jour(incompris, dimanche).unwrap_err();
        assert!(
            erreur.0.starts_with("Je ne comprends pas le jour"),
            "{incompris}"
        );
    }
    assert_eq!(ecrire_jour(jour(2026, 11, 13)), "vendredi 13 novembre");
    assert_eq!(ecrire_jour(jour(2026, 8, 1)), "samedi 1er août");
    assert_eq!(ecrire_jour(jour(2027, 2, 28)), "dimanche 28 février");
}

#[test]
fn legende_d_une_photo_de_dictee() {
    assert_eq!(legende_de_dictee("dictée vendredi"), Some("vendredi"));
    assert_eq!(legende_de_dictee("  Dictee   13/11 "), Some("13/11"));
    assert_eq!(legende_de_dictee("DICTÉE"), Some(""));
    assert_eq!(legende_de_dictee("5 exercice 7"), None);
    assert_eq!(legende_de_dictee(""), None);
    assert_eq!(legende_de_dictee("dictées vendredi"), None);
}

#[tokio::test]
async fn la_voix_lit_chaque_phrase_aux_deux_vitesses() {
    let voix = FausseVoix::default();
    let p = preparer(&voix, DICTEE_6, false).await.unwrap();
    assert_eq!(p.phrases.len(), 3);
    assert_eq!(p.sonores.len(), 3);
    assert_eq!(p.mots_a_demander(), 34);
    let lus = voix.lus.borrow();
    assert_eq!(lus.len(), 6);
    for (i, phrase) in p.phrases.iter().enumerate() {
        assert_eq!(lus[2 * i], (phrase.texte.clone(), false));
        assert_eq!(lus[2 * i + 1], (phrase.texte.clone(), true));
        assert!(p.sonores[i].son.starts_with(b"OggS"));
        assert!(p.sonores[i].son_lent.len() > p.sonores[i].son.len());
        assert_eq!(p.sonores[i].texte, phrase.texte);
    }
    // Le texte gardé : une phrase par ligne, apostrophes du clavier.
    assert_eq!(p.texte.lines().count(), 3);
    assert!(p.texte.contains("s'élève") && !p.texte.contains('’'));
    // L'aperçu : les trois phrases à vitesse normale, séparées par deux silences.
    let attendu: f64 = p
        .phrases
        .iter()
        .map(|ph| {
            lire_wav(&wav_pour(&ph.texte, false))
                .unwrap()
                .duree_secondes()
        })
        .sum::<f64>()
        + 2.0 * 0.8;
    let mut decodeur = vorbis_rs::VorbisDecoder::new(Cursor::new(p.apercu.clone())).unwrap();
    let mut trames = 0usize;
    while let Some(bloc) = decodeur.decode_audio_block().unwrap() {
        trames += bloc.samples()[0].len();
    }
    assert!((trames as f64 / 22_050.0 - attendu).abs() < 0.01);
}

#[tokio::test]
async fn preparation_refusee() {
    let voix = FausseVoix::default();
    let sans_mot = preparer(&voix, "À 1 2 3 !", false).await.unwrap_err();
    assert_eq!(sans_mot.to_string(), "Aucun mot à demander dans ce texte.");
    let trop = "Il pleut. ".repeat(PHRASES_MAX + 1);
    let erreur = preparer(&voix, &trop, false).await.unwrap_err();
    assert!(erreur.to_string().starts_with("41 phrases : c'est trop"));
    assert!(
        voix.lus.borrow().is_empty(),
        "rien n'est lu avant les refus"
    );
    voix.en_panne.set(true);
    let panne = preparer(&voix, "Il pleut.", false).await.unwrap_err();
    assert_eq!(panne.to_string(), "La voix locale est injoignable (essai).");
}

#[tokio::test]
async fn sans_voix_ni_pack_pas_de_dictee_et_l_aide_ne_change_pas() {
    let m = monde_avec(None, false);
    assert_eq!(m.dit("/dictee vendredi Il pleut.").await, DICTEES_ABSENTES);
    assert_eq!(m.dit("/dictee").await, DICTEES_ABSENTES);
    assert_eq!(m.photo("dictée vendredi").await, DICTEES_ABSENTES);
    assert_eq!(m.dit("bonjour").await, AIDE);
    let message = MessageParent::texte(PARENT, "/dictee vendredi Il pleut.");
    assert_eq!(m.commandes.accuse_reception(&message), None);

    let avec = monde();
    assert_eq!(avec.dit("bonjour").await, format!("{AIDE}{AIDE_DICTEE}"));
}

#[tokio::test]
async fn dictee_tapee_puis_oui() {
    let m = monde();
    let message = MessageParent::texte(PARENT, &format!("/dictee vendredi {DICTEE_6}"));
    assert_eq!(
        m.commandes.accuse_reception(&message),
        Some(PREPARATION_EN_COURS)
    );
    let recap = m.dit(&format!("/dictee vendredi {DICTEE_6}")).await;
    let lignes: Vec<&str> = recap.lines().collect();
    assert_eq!(
        lignes[0],
        "Dictée du vendredi 13 novembre : 3 phrase(s), 34 mot(s) à demander."
    );
    assert!(lignes[1].starts_with("1. Dans la ville de Dubaï"));
    assert!(lignes[2].starts_with("2. Burj Khalifa"));
    assert_eq!(
        lignes[3],
        "3. Deux-millions de personnes la visitent chaque année."
    );
    assert_eq!(lignes[4], RAPPEL_DICTEE);
    // Rien n'est enregistré avant le « /oui » ; l'aperçu attend d'être envoyé.
    assert!(m.depot.dictees_depuis(jour(2026, 1, 1)).unwrap().is_empty());
    assert!(!m.pack().exists());
    let (nom, apercu) = m.commandes.prendre_audio(PARENT).unwrap();
    assert_eq!(nom, "dictee-apercu.ogg");
    assert!(apercu.starts_with(b"OggS"));
    assert!(m.commandes.prendre_audio(PARENT).is_none());

    let reponse = m.dit("/oui").await;
    assert!(
        reponse.starts_with("Dictée n°1 enregistrée pour le vendredi 13 novembre.\n"),
        "{reponse}"
    );
    assert!(reponse.contains("/dictee seance 1"));
    let dictee = m.depot.lire_dictee(1).unwrap().unwrap();
    assert_eq!(dictee.jour, jour(2026, 11, 13));
    assert_eq!(dictee.demande_par, "111");
    assert!(!dictee.annulee && dictee.imposes.is_empty() && dictee.derniere_seance.is_none());
    let phrases = m.depot.phrases_de(1).unwrap();
    assert_eq!(
        phrases.iter().map(|p| p.rang).collect::<Vec<_>>(),
        [0, 1, 2]
    );
    // Deux sons par phrase dans le pack, numérotés d'après le numéro de la phrase.
    let attendus: BTreeSet<u32> = phrases
        .iter()
        .flat_map(|p| {
            [
                numero_son(p.id as u32, false),
                numero_son(p.id as u32, true),
            ]
        })
        .collect();
    assert_eq!(m.sons_du_pack(), attendus);
    assert_eq!(attendus.len(), 6);
    // Un second « /oui » n'a plus rien à confirmer.
    assert!(m.dit("/oui").await.starts_with("Rien à confirmer"));
}

#[tokio::test]
async fn non_n_ecrit_rien_et_la_dictee_expire() {
    let m = monde();
    m.dit("/dictee vendredi Il pleut sur la ville.").await;
    assert_eq!(
        m.dit("/non").await,
        "Abandonné : la dictée n'a pas été enregistrée."
    );
    assert!(m.depot.dictees_depuis(jour(2026, 1, 1)).unwrap().is_empty());
    assert!(!m.pack().exists());
    assert_eq!(m.dit("/non").await, "Aucune lecture de photo en attente.");

    m.dit("/dictee vendredi Il pleut sur la ville.").await;
    m.temps.set(m.temps.get() + TimeDelta::minutes(31));
    assert!(m.dit("/oui").await.starts_with("Rien à confirmer"));
    assert!(m.depot.dictees_depuis(jour(2026, 1, 1)).unwrap().is_empty());
}

#[tokio::test]
async fn refus_d_une_nouvelle_dictee() {
    let m = monde();
    assert!(
        m.dit("/dictee 07/11 Il pleut.")
            .await
            .starts_with("Refusé : Le samedi 7 novembre")
    );
    assert!(
        m.dit("/dictee bientôt Il pleut.")
            .await
            .starts_with("Refusé : Je ne comprends pas")
    );
    assert_eq!(
        m.dit("/dictee vendredi À 1 2 !").await,
        "Dictée impossible : Aucun mot à demander dans ce texte."
    );
    assert!(
        m.dit("/dictee vendredi")
            .await
            .starts_with("Il manque le texte")
    );
    assert!(m.voix.lus.borrow().is_empty());
    m.voix.en_panne.set(true);
    assert_eq!(
        m.dit("/dictee vendredi Il pleut.").await,
        "Dictée impossible : La voix locale est injoignable (essai)."
    );
    assert!(m.commandes.dictee_en_attente(PARENT).is_none());
    assert!(m.commandes.prendre_audio(PARENT).is_none());
}

#[tokio::test]
async fn liste_de_mots_sans_texte() {
    let m = monde();
    let recap = m
        .dit("/dictee 13/11 mots : plus, trois, chaque année")
        .await;
    assert!(
        recap.starts_with("Dictée du vendredi 13 novembre : 3 phrase(s), 4 mot(s) à demander.")
    );
    assert!(recap.contains("\n1. plus\n2. trois\n3. chaque année\n"));
    assert_eq!(
        *m.voix.lus.borrow().first().unwrap(),
        ("plus".to_owned(), false)
    );
    m.dit("/oui").await;
    assert_eq!(m.depot.phrases_de(1).unwrap().len(), 3);
}

#[tokio::test]
async fn photo_de_dictee() {
    // LLM distant : refus sans appel, comme pour toute photo.
    let distant = llm(DICTEE_6, true);
    let m = monde_avec(Some(distant.clone()), true);
    assert!(
        m.photo("dictée vendredi")
            .await
            .contains("ne part jamais vers un LLM distant")
    );
    assert_eq!(distant.appels.get(), 0);

    // Sans LLM : le texte tapé reste possible.
    let m = monde();
    assert!(
        m.photo("dictée vendredi")
            .await
            .contains("/dictee vendredi <texte>")
    );

    // LLM local : le texte recopié suit le même chemin que le texte tapé.
    let local = llm(DICTEE_6, false);
    let m = monde_avec(Some(local.clone()), true);
    let message = MessageParent {
        expediteur: PARENT,
        texte: "Dictée vendredi".to_owned(),
        image: Some(jpeg()),
    };
    assert_eq!(
        m.commandes.accuse_reception(&message),
        Some(LECTURE_EN_COURS)
    );
    assert_eq!(
        m.photo("dictée").await,
        "Il manque le jour dans la légende : « dictée vendredi »."
    );
    assert!(m.photo("dictée 07/11").await.starts_with("Refusé"));
    assert_eq!(
        local.appels.get(),
        0,
        "le jour est vérifié avant la lecture"
    );
    let recap = m.photo("Dictée vendredi").await;
    assert_eq!(local.appels.get(), 1);
    assert!(recap.starts_with("Dictée du vendredi 13 novembre : 3 phrase(s)"));
    // Une lecture de photo le dit : le modèle peut se tromper d'une lettre.
    let lignes: Vec<&str> = recap.lines().collect();
    assert_eq!(lignes[lignes.len() - 2], RAPPEL_PHOTO_DICTEE);
    assert_eq!(lignes[lignes.len() - 1], RAPPEL_DICTEE);
    assert!(m.commandes.prendre_audio(PARENT).is_some());
    assert!(m.dit("/oui").await.starts_with("Dictée n°1 enregistrée"));

    // Une photo sans le mot « dictée » reste une fiche de calculs.
    let fiche = m.photo("1").await;
    assert!(fiche.contains("Tirés au hasard (1)"), "{fiche}");
    assert!(m.commandes.prendre_audio(PARENT).is_none());
}

#[tokio::test]
async fn photo_sans_texte_lu() {
    let m = monde_avec(Some(llm("  ", false)), true);
    assert_eq!(
        m.photo("dictée vendredi").await,
        "Lecture impossible : Je n'ai lu aucun texte imprimé. La photo est-elle à l'endroit ?"
    );
}

#[tokio::test]
async fn une_seule_chose_a_confirmer_a_la_fois() {
    let m = monde_avec(Some(llm(DICTEE_6, false)), true);
    m.dit("/dictee vendredi Il pleut sur la ville.").await;
    // Une fiche arrive ensuite : c'est elle que « /oui » confirme.
    m.photo("1").await;
    assert!(m.commandes.dictee_en_attente(PARENT).is_none());
    assert!(m.dit("/oui").await.starts_with("Coffre n°1 en file"));
    // Et l'inverse : la dictée remplace la fiche en attente.
    m.photo("1").await;
    m.dit("/dictee vendredi Il pleut sur la ville.").await;
    assert!(m.commandes.lecture_en_attente(PARENT).is_none());
    assert!(m.dit("/oui").await.starts_with("Dictée n°1 enregistrée"));
}

async fn dictee_enregistree(m: &Monde) {
    m.dit(&format!("/dictee vendredi {DICTEE_6}")).await;
    m.dit("/oui").await;
}

#[tokio::test]
async fn mots_imposes_et_liste_des_dictees() {
    let m = monde();
    assert_eq!(m.dit("/dictee").await, "Aucune dictée en cours.");
    dictee_enregistree(&m).await;
    assert_eq!(
        m.dit("/dictee mots 1 plus girafe Émirats").await,
        "Refusé : pas dans le texte, ou impossible à demander : girafe, Émirats."
    );
    assert!(m.depot.lire_dictee(1).unwrap().unwrap().imposes.is_empty());
    assert_eq!(
        m.dit("/dictee mots 1 plus, trois personnes").await,
        "Dictée n°1 : ces mots sortiront en premier : plus, trois, personnes."
    );
    assert_eq!(
        m.dit("/dictee").await,
        "Dictées en cours :\n• n°1, vendredi 13 novembre : 3 phrase(s), mots imposés : plus, trois, personnes"
    );
    assert_eq!(
        m.dit("/dictee mots 1").await,
        "Dictée n°1 : plus aucun mot imposé."
    );
    assert!(m.dit("/dictee mots 9 plus").await.contains("n'existe pas"));
    assert!(
        m.dit("/dictee mots plus")
            .await
            .starts_with("Préciser le numéro")
    );
}

#[tokio::test]
async fn seance_a_la_demande() {
    let m = monde();
    dictee_enregistree(&m).await;
    m.dit("/dictee mots 1 plus trois personnes").await;
    let reponse = m.dit("/dictee seance 1").await;
    let lignes: Vec<&str> = reponse.lines().collect();
    assert_eq!(lignes.len(), 4);
    assert!(
        lignes.iter().all(|l| l.contains("en file : Dictée : ")),
        "{reponse}"
    );
    let coffres = m.depot.en_file().unwrap();
    assert_eq!(coffres.len(), 4);
    let phrases: HashMap<usize, i64> = m
        .depot
        .phrases_de(1)
        .unwrap()
        .into_iter()
        .map(|p| (p.rang, p.id))
        .collect();
    for coffre in &coffres {
        assert_eq!(coffre.source, Source::Dictee);
        assert_eq!(coffre.statut, Statut::EnFile);
        assert_eq!(coffre.demande_par, "111");
        let d = &coffre.exercice.donnees;
        assert_eq!(d["dictee"], 1);
        let phrase = phrases[&(d["phrase"].as_u64().unwrap() as usize)] as u32;
        assert_eq!(d["son"], numero_son(phrase, false));
        assert_eq!(d["son_lent"], numero_son(phrase, true));
        assert!(m.sons_du_pack().contains(&numero_son(phrase, false)));
    }
    // Les trois mots imposés d'abord.
    let premiers: BTreeSet<&str> = coffres[..3]
        .iter()
        .map(|c| c.exercice.reponse.as_str())
        .collect();
    assert_eq!(premiers, BTreeSet::from(["plus", "trois", "personnes"]));
    assert!(m.dit("/dictee seance 7").await.contains("n'existe pas"));
}

#[tokio::test]
async fn annuler_une_dictee_retire_ses_coffres_et_ses_sons() {
    let m = monde();
    dictee_enregistree(&m).await;
    m.dit("/dictee seance 1").await;
    m.dit("/coffre 348 ÷ 12").await;
    assert_eq!(m.sons_du_pack().len(), 6);
    assert_eq!(
        m.dit("/dictee annuler 1").await,
        "Dictée n°1 annulée, 4 coffre(s) retiré(s) de la file."
    );
    // Le coffre de maths reste, les sons partent.
    let restants = m.depot.en_file().unwrap();
    assert_eq!(restants.len(), 1);
    assert_eq!(restants[0].exercice.reponse, "29");
    assert!(m.sons_du_pack().is_empty());
    assert_eq!(m.dit("/dictee").await, "Aucune dictée en cours.");
    assert!(
        m.dit("/dictee annuler 1")
            .await
            .contains("n'existe pas, ou elle est annulée")
    );
    assert!(m.dit("/dictee seance 1").await.contains("annulée"));
}

#[tokio::test]
async fn le_pack_garde_une_dictee_trente_jours_apres_son_jour() {
    let m = monde();
    dictee_enregistree(&m).await;
    let pack = m.pack();
    assert_eq!(
        reconstruire_pack(&m.depot, &pack, jour(2026, 12, 13)).unwrap(),
        6
    );
    assert_eq!(
        reconstruire_pack(&m.depot, &pack, jour(2026, 12, 14)).unwrap(),
        0
    );
    assert!(sons_de(&pack).is_empty());
    // Elle n'est plus « en cours » dès le lendemain de son jour.
    assert_eq!(m.depot.dictees_depuis(jour(2026, 11, 13)).unwrap().len(), 1);
    assert!(
        m.depot
            .dictees_depuis(jour(2026, 11, 14))
            .unwrap()
            .is_empty()
    );
}

#[derive(Default)]
struct FauxCanal {
    messages: RefCell<VecDeque<MessageParent>>,
    envois: RefCell<Vec<String>>,
}

#[async_trait(?Send)]
impl Canal for FauxCanal {
    async fn envoyer(&self, texte: &str) {
        self.envois.borrow_mut().push(format!("à tous : {texte}"));
    }

    async fn repondre(&self, destinataire: i64, texte: &str) {
        let debut: String = texte.lines().next().unwrap_or_default().to_owned();
        self.envois
            .borrow_mut()
            .push(format!("{destinataire} : {debut}"));
    }

    async fn envoyer_audio(&self, destinataire: i64, nom: &str, ogg: &[u8]) {
        assert!(ogg.starts_with(b"OggS"));
        self.envois
            .borrow_mut()
            .push(format!("{destinataire} : audio {nom}"));
    }

    async fn recevoir(&self) -> Option<MessageParent> {
        self.messages.borrow_mut().pop_front()
    }
}

#[tokio::test]
async fn servir_accuse_reception_repond_puis_envoie_l_apercu() {
    let m = monde();
    let canal = FauxCanal::default();
    canal.messages.borrow_mut().extend([
        MessageParent::texte(PARENT, "/dictee vendredi Il pleut sur la ville."),
        MessageParent::texte(333, "/dictee vendredi Un inconnu écrit."),
        MessageParent::texte(PARENT, "/oui"),
    ]);
    servir(&canal, &m.commandes).await;
    assert_eq!(
        *canal.envois.borrow(),
        [
            format!("111 : {PREPARATION_EN_COURS}"),
            "111 : Dictée du vendredi 13 novembre : 1 phrase(s), 5 mot(s) à demander.".to_owned(),
            "111 : audio dictee-apercu.ogg".to_owned(),
            "111 : Dictée n°1 enregistrée pour le vendredi 13 novembre.".to_owned(),
        ]
    );
}

#[test]
fn rien_d_une_dictee_dans_le_json_d_un_exercice_ne_donne_le_mot_en_clair_au_jeu() {
    // Le jeu ne lit de `donnees` que le son et la phrase à trou : ni l'un ni l'autre ne
    // contient la réponse.
    let phrases = mine_de_rien::exercises::francais::dictee::decouper("La plus haute tour.");
    let position = mine_de_rien::exercises::francais::dictee::Position { phrase: 0, mot: 1 };
    let ex = mine_de_rien::dictees::exercice_sonore(&phrases, position, 4).unwrap();
    let Value::String(a_trou) = &ex.donnees["a_trou"] else {
        panic!("phrase à trou absente")
    };
    assert_eq!(a_trou, "La _____ haute tour.");
    assert_eq!(
        (ex.donnees["son"].clone(), ex.donnees["son_lent"].clone()),
        (json!(8), json!(9))
    );
}

#[tokio::test]
async fn le_rapport_fait_le_point_sur_les_dictees_en_cours() {
    let m = monde();
    // Sans dictée, le rapport est celui d'avant.
    assert!(!m.dit("/rapport").await.contains("Dictées en cours"));
    dictee_enregistree(&m).await;
    m.dit("/dictee mots 1 personnes").await;
    m.dit("/dictee seance 1").await;
    let coffres = m.depot.en_file().unwrap();
    let personnes = coffres
        .iter()
        .find(|c| c.exercice.reponse == "personnes")
        .unwrap();
    let quand = m.temps.get();
    m.depot.marquer_pose(personnes.id, quand).unwrap();
    m.depot
        .noter_tentative(personnes.id, "persone", false, quand)
        .unwrap();
    m.depot
        .noter_tentative(personnes.id, "personnes", true, quand)
        .unwrap();
    m.depot
        .marquer_ouvert(personnes.id, quand + TimeDelta::minutes(2))
        .unwrap();
    let rapport = m.dit("/rapport").await;
    let fin: Vec<&str> = rapport.lines().rev().take(2).collect();
    assert_eq!(
        fin,
        [
            "• n°1, vendredi 13 novembre : 0 mot(s) acquis sur 34 ; fragiles : personnes (persone)",
            "Dictées en cours :",
        ]
    );
    // Le mot raté aujourd'hui repose : une nouvelle séance ne le redemande pas.
    m.dit("/annuler tout").await;
    for _ in 0..5 {
        m.dit("/dictee seance 1").await;
    }
    let redemandes = m
        .depot
        .coffres_de_dictee(1)
        .unwrap()
        .iter()
        .filter(|c| c.exercice.reponse == "personnes")
        .count();
    assert_eq!(redemandes, 1);
}

#[tokio::test]
async fn la_confirmation_annonce_les_seances_quotidiennes() {
    let m = monde();
    m.dit("/dictee vendredi Il pleut sur la ville.").await;
    let reponse = m.dit("/oui").await;
    assert!(
        reponse.contains("4 coffres l'attendront à sa première connexion de chaque jour"),
        "{reponse}"
    );
}
