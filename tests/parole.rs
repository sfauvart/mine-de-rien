//! Les messages du chat dits à voix haute : ce qui est réécrit, ce qui est dit, ce qui ne
//! l'est jamais.

mod commun;

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use commun::{FausseDiffusion, VoixDeTest};
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::alea::Alea;
use mine_de_rien::dictees::{Lecteur, fichier_du_son};
use mine_de_rien::diffusion::{Parole, prononcer};
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::exercises::indices::{contient_la_reponse, indices};
use mine_de_rien::exercises::registre::ENTREES;
use mine_de_rien::games::coffre_du_soir::{CLE_JUSTE, Coffre, CoffreDuSoir, MODE_EMPLOI};
use mine_de_rien::games::dictee;
use mine_de_rien::minecraft::texte::Style;
use mine_de_rien::saisie::Source;
use mine_de_rien::storage::sqlite::{Depot, ExerciceStocke, Statut};

const PSEUDO: &str = "Steve";

#[test]
fn les_symboles_d_un_enonce_se_disent() {
    let dit = |ecrit: &str| prononcer(ecrit).unwrap();
    assert_eq!(dit("348 ÷ 12 = ?"), "348 divisé par 12 égale combien ?");
    assert_eq!(dit("7 × 8 = ?"), "7 fois 8 égale combien ?");
    assert_eq!(dit("12 + 5 − 3"), "12 plus 5 moins 3");
    // Un tiret entre deux nombres est un « moins » ; ailleurs, un trait d'union.
    assert_eq!(dit("12 - 5 = ?"), "12 moins 5 égale combien ?");
    assert_eq!(
        dit("Écris « Pixel, aide-moi »"),
        "Écris « Pixel, aide-moi »"
    );
    assert_eq!(
        dit("Un coffre t'attend à l'atelier : 348 ÷ 12 = ?"),
        "Un coffre t'attend à l'atelier : 348 divisé par 12 égale combien ?"
    );
    assert_eq!(dit("20 % de 50"), "20 pour cent de 50");
}

#[test]
fn le_trou_et_les_codes_de_couleur_ne_se_lisent_pas_tels_quels() {
    assert_eq!(
        prononcer("Dans la _____ de Dubaï.").unwrap(),
        "Dans la le mot manquant de Dubaï."
    );
    assert_eq!(prononcer("§6Bravo §r!").unwrap(), "Bravo !");
    assert!(!prononcer("§aUn nouveau coffre").unwrap().contains('§'));
    // Un seul tiret bas n'est pas un trou.
    assert_eq!(prononcer("mdr_cle").unwrap(), "mdr cle");
    // Rien à dire : pas de son du tout.
    for vide in ["", "   ", "…", "?", "_____", "§a", "= ?"] {
        let dit = prononcer(vide);
        assert!(
            dit.as_deref()
                .is_none_or(|d| d.chars().any(char::is_alphanumeric)),
            "{vide:?} → {dit:?}"
        );
    }
    assert_eq!(prononcer("…"), None);
    assert_eq!(prononcer(""), None);
}

fn parole() -> (Parole, Rc<FausseDiffusion>, Rc<VoixDeTest>) {
    let (diffusion, voix) = (
        Rc::new(FausseDiffusion::new()),
        Rc::new(VoixDeTest::default()),
    );
    (
        Parole::new(diffusion.clone(), voix.clone()),
        diffusion,
        voix,
    )
}

fn lus(voix: &VoixDeTest) -> Vec<String> {
    voix.lus.borrow().iter().map(|(t, _)| t.clone()).collect()
}

fn calcul(enonce: &str, reponse: &str) -> Exercice {
    Exercice {
        competence: "maths/division-posee".to_owned(),
        enonce: enonce.to_owned(),
        reponse: reponse.to_owned(),
        donnees: Default::default(),
    }
}

#[tokio::test]
async fn chaque_message_retenu_est_dit_une_fois_dans_l_ordre() {
    let (parole, diffusion, voix) = parole();
    parole.retenir("Premier message.", None);
    parole.retenir("…", None);
    parole.retenir("Deuxième : 6 × 7 = ?", None);
    // Rien ne part avant « dire » : le texte n'attend pas la voix.
    assert!(voix.lus.borrow().is_empty());
    parole.dire().await;
    assert_eq!(
        lus(&voix),
        ["Premier message.", "Deuxième : 6 fois 7 égale combien ?"]
    );
    // À vitesse normale, et chaque son part dans la file de l'enfant.
    assert!(voix.lus.borrow().iter().all(|(_, lent)| !lent));
    assert_eq!(diffusion.actions(), ["dire", "dire"]);
    // Un second « dire » n'a plus rien à dire.
    parole.dire().await;
    assert_eq!(diffusion.dits.borrow().len(), 2);
}

#[tokio::test]
async fn un_texte_qui_donne_la_reponse_n_est_pas_dit() {
    let (parole, _, voix) = parole();
    let ex = calcul("348 ÷ 12 = ?", "29");
    parole.retenir("Le résultat est 29.", Some(&ex));
    parole.retenir("C'est vingt-neuf !", Some(&ex));
    parole.retenir("Un coffre t'attend à l'atelier : 348 ÷ 12 = ?", Some(&ex));
    // La réécriture ne doit pas fabriquer la réponse : ici le mot « plus » est la clé.
    let mot = Exercice {
        competence: mine_de_rien::exercises::francais::dictee::COMPETENCE.to_owned(),
        enonce: "Dictée".to_owned(),
        reponse: "plus".to_owned(),
        donnees: Default::default(),
    };
    parole.retenir("3 + 4", Some(&mot));
    parole.dire().await;
    assert_eq!(
        lus(&voix),
        ["Un coffre t'attend à l'atelier : 348 divisé par 12 égale combien ?"]
    );
}

#[tokio::test]
async fn sans_chat_vocal_ou_sans_voix_rien_n_est_dit_et_rien_n_echoue() {
    // L'enfant n'est pas relié : la voix n'est même pas sollicitée.
    let (parole, diffusion, voix) = parole();
    diffusion.reliee.set(false);
    parole.retenir("Bonjour.", None);
    parole.dire().await;
    assert!(voix.lus.borrow().is_empty());
    assert!(diffusion.actions().is_empty());
    // Le message n'est pas gardé pour plus tard : il serait dit hors de propos.
    diffusion.reliee.set(true);
    parole.dire().await;
    assert!(voix.lus.borrow().is_empty());

    // Piper ne répond pas.
    let (parole, diffusion, voix) = crate::parole();
    voix.en_panne.set(true);
    parole.retenir("Un.", None);
    parole.retenir("Deux.", None);
    parole.dire().await;
    assert!(diffusion.actions().is_empty());

    // Le serveur refuse de jouer : on s'arrête au premier, sans insister.
    let (parole, diffusion, voix) = crate::parole();
    diffusion.en_panne.set(true);
    parole.retenir("Un.", None);
    parole.retenir("Deux.", None);
    parole.dire().await;
    assert_eq!(lus(&voix), ["Un."]);
    assert!(diffusion.dits.borrow().is_empty());
}

/// Un faux serveur : il note les messages écrits à l'enfant et les commandes.
#[derive(Default)]
struct FauxJeu {
    commandes: RefCell<Vec<String>>,
    ecrits: RefCell<Vec<String>>,
}

#[async_trait(?Send)]
impl AdaptateurJeu for FauxJeu {
    async fn commande(&self, commande: &str) -> Result<String, ErreurRcon> {
        self.commandes.borrow_mut().push(commande.to_owned());
        if commande == "data get storage mine_de_rien:atelier pos" {
            return Ok(
                "Storage mine_de_rien:atelier has the following contents: [0, -60, 8]".into(),
            );
        }
        if commande.starts_with("execute if ") {
            return Ok("Test passed".to_owned());
        }
        if commande.starts_with("clear ") && commande.ends_with(" 0") {
            return Ok("Found 1 matching item(s) on player Steve".to_owned());
        }
        Ok(String::new())
    }

    async fn dire(&self, _: &str, texte: &str, _: &[(&str, Style<'_>)]) -> Result<(), ErreurRcon> {
        self.ecrits.borrow_mut().push(texte.to_owned());
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat ici".to_owned()))
    }
}

fn stocke(exercice: Exercice, source: Source) -> ExerciceStocke {
    ExerciceStocke {
        id: 12,
        exercice,
        jeu: "coffre-du-soir".to_owned(),
        source,
        statut: Statut::EnFile,
        demande_par: "cli".to_owned(),
        recompense: "minecraft:diamond".to_owned(),
        recompense_nombre: 3,
        cree_le: commun::instant("2026-11-10T18:00:00+00:00"),
        pose_le: None,
        ouvert_le: None,
        niveaux_donnes: false,
    }
}

struct Monde {
    _dossier: tempfile::TempDir,
    depot: Rc<Depot>,
    jeu: Rc<FauxJeu>,
    coffre: CoffreDuSoir,
    diffusion: Rc<FausseDiffusion>,
    voix: Rc<VoixDeTest>,
}

/// Un coffre du soir qui parle : la même fausse diffusion reçoit les messages et les phrases.
async fn monde(avec_parole: bool) -> Monde {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Rc::new(Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap());
    let (diffusion, voix) = (
        Rc::new(FausseDiffusion::new()),
        Rc::new(VoixDeTest::default()),
    );
    let jeu = Rc::new(FauxJeu::default());
    let mut coffre = CoffreDuSoir::new(jeu.clone(), PSEUDO, 3, Some("Pixel"))
        .unwrap()
        .avec_lecteur(Rc::new(Lecteur::new(
            diffusion.clone(),
            voix.clone(),
            depot.clone(),
        )));
    if avec_parole {
        coffre = coffre.avec_parole(Rc::new(Parole::new(diffusion.clone(), voix.clone())));
    }
    coffre.actualiser_atelier().await.unwrap();
    jeu.commandes.borrow_mut().clear();
    Monde {
        _dossier: dossier,
        depot,
        jeu,
        coffre,
        diffusion,
        voix,
    }
}

#[tokio::test]
async fn annonce_rappel_et_bonne_reponse_d_un_calcul_sont_dits_pas_le_mode_d_emploi() {
    let m = monde(true).await;
    let s = stocke(calcul("348 ÷ 12 = ?", "29"), Source::Expression);
    m.coffre.annoncer(&s).await.unwrap();
    m.coffre.rappeler(&s).await.unwrap();
    m.coffre.accepter_la_cle("29").await.unwrap();
    assert_eq!(
        lus(&m.voix),
        [
            "Un nouveau coffre est apparu à l'atelier !",
            "Un coffre t'attend à l'atelier : 348 divisé par 12 égale combien ?",
            CLE_JUSTE,
        ]
    );
    assert_eq!(m.diffusion.actions(), ["dire", "dire", "dire"]);
    // Le mode d'emploi est écrit à chaque fois, jamais dit.
    let ecrits = m.jeu.ecrits.borrow();
    assert_eq!(ecrits.iter().filter(|e| *e == MODE_EMPLOI).count(), 2);
    assert!(!lus(&m.voix).iter().any(|dit| dit.contains("enclume")));
    assert!(!lus(&m.voix).iter().any(|dit| dit.contains("aide-moi")));
}

#[tokio::test]
async fn l_annonce_d_une_dictee_est_dite_avant_la_phrase_jamais_la_phrase_a_trou() {
    let m = monde(true).await;
    let texte = "Dans la ville de Dubaï s'élève la plus haute tour jamais construite.";
    let s = stocke(
        commun::trou_en_base(&m.depot, texte, "plus"),
        Source::Dictee,
    );
    let normal = fichier_du_son(dictee::son(&s.exercice).unwrap());
    m.coffre.preparer_la_dictee(&s).await.unwrap();
    m.voix.lus.borrow_mut().clear();
    m.diffusion.actions.borrow_mut().clear();

    m.coffre.annoncer(&s).await.unwrap();
    // L'annonce, dite par la voix à la volée, puis la phrase gardée.
    assert_eq!(lus(&m.voix), [dictee::ANNONCE]);
    assert_eq!(
        m.diffusion.actions(),
        [
            "dire".to_owned(),
            format!("désigner {normal}"),
            format!("redire {normal}")
        ]
    );
    // La phrase à trou est écrite, pas dite ; le mot n'est ni écrit ni dit.
    assert!(m.jeu.ecrits.borrow().iter().any(|e| e.contains("_____")));
    for dit in lus(&m.voix) {
        assert!(!dit.contains("mot manquant"), "{dit}");
        assert!(!contient_la_reponse(&dit, &s.exercice), "{dit}");
    }

    m.voix.lus.borrow_mut().clear();
    m.coffre.rappeler(&s).await.unwrap();
    assert_eq!(lus(&m.voix), [dictee::RAPPEL]);
}

#[tokio::test]
async fn une_voix_en_panne_ne_change_ni_le_chat_ni_les_commandes() {
    let s = stocke(calcul("348 ÷ 12 = ?", "29"), Source::Expression);
    let mut vus = Vec::new();
    for (avec_parole, voix_en_panne, serveur_en_panne, relie) in [
        (false, false, false, true),
        (true, false, false, true),
        (true, true, false, true),
        (true, false, true, true),
        (true, false, false, false),
    ] {
        let m = monde(avec_parole).await;
        m.voix.en_panne.set(voix_en_panne);
        m.diffusion.en_panne.set(serveur_en_panne);
        m.diffusion.reliee.set(relie);
        m.coffre.poser(&s).await.unwrap();
        m.coffre.annoncer(&s).await.unwrap();
        m.coffre.rappeler(&s).await.unwrap();
        m.coffre.accepter_la_cle("29").await.unwrap();
        m.coffre.ranger(&s).await.unwrap();
        vus.push((
            m.jeu.ecrits.borrow().clone(),
            m.jeu.commandes.borrow().clone(),
        ));
    }
    // Avec ou sans voix, en panne ou non : les mêmes messages, les mêmes commandes.
    for vu in &vus[1..] {
        assert_eq!(vu, &vus[0]);
    }
    assert!(!vus[0].0.is_empty() && !vus[0].1.is_empty());
}

#[tokio::test]
async fn rien_de_ce_qui_est_dit_ne_donne_la_reponse_sur_des_centaines_de_graines() {
    let m = monde(true).await;
    let (parole, _, voix_des_indices) = parole();
    let mut dits = 0;
    for entree in &ENTREES {
        for graine in 0..300 {
            let mut alea = Alea::graine(graine);
            let ex = (entree.generer)(&mut alea, 1 + (graine % 3) as u32);
            let s = stocke(ex.clone(), Source::Expression);
            m.voix.lus.borrow_mut().clear();
            m.coffre.annoncer(&s).await.unwrap();
            m.coffre.rappeler(&s).await.unwrap();
            m.coffre.accepter_la_cle(&ex.reponse).await.unwrap();
            // Les indices de Pixel passent par la même parole.
            voix_des_indices.lus.borrow_mut().clear();
            for palier in indices(&ex) {
                parole.retenir(&palier, Some(&ex));
            }
            parole.dire().await;
            for dit in lus(&m.voix).into_iter().chain(lus(&voix_des_indices)) {
                assert!(
                    !contient_la_reponse(&dit, &ex),
                    "{} graine {graine} : « {dit} » donne {}",
                    entree.nom,
                    ex.reponse
                );
                assert!(!dit.contains(['÷', '×', '=', '_', '§']), "{dit}");
                dits += 1;
            }
        }
    }
    // L'annonce et la bonne réponse au moins, pour chaque coffre.
    assert!(dits >= 2 * 300 * ENTREES.len(), "{dits}");
}
