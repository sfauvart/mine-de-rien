//! Le coffre du soir quand sa clé est un mot de dictée : ce qu'il dit, joue et range en plus.

mod commun;

use std::cell::RefCell;
use std::rc::Rc;

use async_trait::async_trait;
use commun::{FausseDiffusion, VoixDeTest};
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::dictees::{Lecteur, exercice_sonore, fichier_du_son, numero_son};
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::exercises::francais::dictee::{Position, candidats, decouper};
use mine_de_rien::exercises::indices::{contient_la_reponse, indices};
use mine_de_rien::games::coffre_du_soir::{Coffre, CoffreDuSoir, predicat_cle_a_jour};
use mine_de_rien::games::dictee::{
    self, commande_ecouteur, commandes_effacement, commandes_stockage, est_dictee,
    predicat_ecouteur,
};
use mine_de_rien::minecraft::texte::Style;
use mine_de_rien::saisie::Source;
use mine_de_rien::storage::sqlite::{Depot, ExerciceStocke, Statut};

const TEXTE: &str = "Dans la ville de Dubaï, aux Émirats arabes unis, s’élève la plus haute \
    tour jamais construite. Il dit : \"Viens\" \\ ici.";
const PSEUDO: &str = "Steve";

/// Un faux serveur : il note tout, et répond « Found » aux inventaires qu'on lui indique.
#[derive(Default)]
struct FauxJeu {
    commandes: RefCell<Vec<String>>,
    dits: RefCell<Vec<(String, String)>>,
    /// Morceaux de commande `clear … 0` pour lesquels l'objet est dans l'inventaire.
    possede: RefCell<Vec<String>>,
}

impl FauxJeu {
    fn commandes(&self) -> Vec<String> {
        self.commandes.borrow().clone()
    }

    fn textes(&self) -> Vec<String> {
        self.dits.borrow().iter().map(|(t, _)| t.clone()).collect()
    }

    fn vider(&self) {
        self.commandes.borrow_mut().clear();
        self.dits.borrow_mut().clear();
    }
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
        if commande.ends_with(" 0") && commande.starts_with("clear ") {
            let trouve = self
                .possede
                .borrow()
                .iter()
                .any(|p| commande.contains(p.as_str()));
            return Ok(if trouve {
                "Found 1 matching item(s) on player Steve".to_owned()
            } else {
                "No items were found on player Steve".to_owned()
            });
        }
        Ok(String::new())
    }

    async fn dire(
        &self,
        joueur: &str,
        texte: &str,
        style: &[(&str, Style<'_>)],
    ) -> Result<(), ErreurRcon> {
        assert_eq!(joueur, PSEUDO);
        self.dits
            .borrow_mut()
            .push((texte.to_owned(), commun::style_json(style).to_string()));
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat ici".to_owned()))
    }
}

fn stocke(exercice: Exercice, ident: i64) -> ExerciceStocke {
    ExerciceStocke {
        id: ident,
        exercice,
        jeu: "coffre-du-soir".to_owned(),
        source: Source::Dictee,
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

/// Le trou du mot `mot` dans `TEXTE`, avec les sons de la phrase n°`phrase` de la base.
fn trou(mot: &str, phrase: u32) -> ExerciceStocke {
    let phrases = decouper(TEXTE);
    let position = candidats(&phrases)
        .into_iter()
        .find(|p| phrases[p.phrase].mots[p.mot].texte == mot)
        .unwrap_or_else(|| panic!("{mot} absent"));
    stocke(exercice_sonore(&phrases, position, phrase).unwrap(), 12)
}

fn maths() -> ExerciceStocke {
    let exercice = Exercice {
        competence: "maths/division-posee".to_owned(),
        enonce: "348 ÷ 12 = ?".to_owned(),
        reponse: "29".to_owned(),
        donnees: Default::default(),
    };
    ExerciceStocke {
        source: Source::Expression,
        ..stocke(exercice, 13)
    }
}

async fn coffre(jeu: &Rc<FauxJeu>) -> CoffreDuSoir {
    let coffre = CoffreDuSoir::new(jeu.clone(), PSEUDO, 3, Some("Pixel")).unwrap();
    coffre.actualiser_atelier().await.unwrap();
    jeu.vider();
    coffre
}

#[test]
fn numeros_des_sons() {
    assert_eq!((numero_son(1, false), numero_son(1, true)), (2, 3));
    assert_eq!((numero_son(7, false), numero_son(7, true)), (14, 15));
    let s = trou("plus", 7);
    assert!(est_dictee(&s.exercice));
    assert_eq!(
        (dictee::son(&s.exercice), dictee::son_lent(&s.exercice)),
        (Some(14), Some(15))
    );
    assert!(!est_dictee(&maths().exercice));
}

#[test]
fn stockage_de_l_ecouteur_echappe() {
    let s = trou("Viens", 9);
    assert_eq!(
        commandes_stockage(&s.exercice),
        [
            "data modify storage mine_de_rien:dictee phrase set value \"Il dit : \\\"_____\\\" \\\\ ici.\"",
        ]
    );
    assert_eq!(
        commandes_effacement(),
        [
            "data remove storage mine_de_rien:dictee son",
            "data remove storage mine_de_rien:dictee phrase",
        ]
    );
    assert!(commandes_stockage(&maths().exercice).is_empty());
}

#[test]
fn lecture_et_ecouteur() {
    let give = commande_ecouteur(PSEUDO).unwrap();
    assert!(give.starts_with(
        "give Steve minecraft:warped_fungus_on_a_stick[minecraft:item_name=\"Écouteur\","
    ));
    assert!(give.contains("minecraft:item_model=\"minecraft:note_block\""));
    assert!(give.ends_with("minecraft:custom_data={mdr_ecouteur:1b}] 1"));
    assert_eq!(
        predicat_ecouteur(),
        "minecraft:warped_fungus_on_a_stick[minecraft:custom_data={mdr_ecouteur:1b}]"
    );
    assert!(commande_ecouteur("@a").is_err());
    // La clé à jour se reconnaît à sa description, qui ne dit plus « en minuscules ».
    assert!(predicat_cle_a_jour().contains("\"2. Écris la réponse\""));
    assert!(!predicat_cle_a_jour().contains("minuscules"));
}

#[tokio::test]
async fn pose_verrou_a_majuscule_et_apostrophe_puis_stockage() {
    let jeu = Rc::new(FauxJeu::default());
    let coffre = coffre(&jeu).await;
    for (mot, cle) in [
        ("s'élève", "\"s'élève\""),
        ("Dubaï", "\"Dubaï\""),
        ("Dans", "\"Dans\""),
    ] {
        jeu.vider();
        coffre.poser(&trou(mot, 7)).await.unwrap();
        let commandes = jeu.commandes();
        assert_eq!(
            commandes[1],
            format!(
                "data modify block 0 -60 8 lock set value {{components:{{\"minecraft:custom_name\":{cle}}}}}"
            )
        );
        // Le panneau ne porte jamais le mot.
        let panneau = commandes.iter().find(|c| c.contains("oak_sign")).unwrap();
        assert!(
            panneau.contains("Dictée :") && !panneau.contains(mot),
            "{panneau}"
        );
        // Après la pose vérifiée, ce que l'Écouteur réaffichera.
        let fin = &commandes[commandes.len() - 1];
        assert!(fin.starts_with("data modify storage mine_de_rien:dictee phrase set value \""));
        assert!(fin.contains("_____"));
        // Le son ne passe plus par une commande du jeu.
        assert!(!commandes.iter().any(|c| c.contains("dictee son ")));
    }
}

#[tokio::test]
async fn un_coffre_de_maths_n_ecrit_rien_pour_l_ecouteur() {
    let jeu = Rc::new(FauxJeu::default());
    let coffre = coffre(&jeu).await;
    let s = maths();
    coffre.poser(&s).await.unwrap();
    coffre.annoncer(&s).await.unwrap();
    coffre.rappeler(&s).await.unwrap();
    coffre.ranger(&s).await.unwrap();
    coffre.retirer(None).await.unwrap();
    let commandes = jeu.commandes();
    assert!(
        !commandes.iter().any(|c| c.contains("mine_de_rien:dictee")),
        "{commandes:?}"
    );
    assert!(
        !commandes.iter().any(|c| c.contains("warped_fungus")),
        "{commandes:?}"
    );
    // Son mode d'emploi dans le chat dit toujours « en minuscules », et la cloche sonne.
    assert!(
        jeu.textes()
            .iter()
            .any(|t| t.contains("écrite en minuscules"))
    );
    assert!(
        commandes
            .iter()
            .any(|c| c.contains("minecraft:block.bell.use"))
    );
}

#[tokio::test]
async fn annonce_d_une_dictee() {
    let jeu = Rc::new(FauxJeu::default());
    let coffre = coffre(&jeu).await;
    let s = trou("plus", 7);
    coffre.annoncer(&s).await.unwrap();
    assert_eq!(
        jeu.textes(),
        [
            "Un nouveau coffre de dictée est apparu à l'atelier !",
            "Dictée : Dans la ville de Dubaï, aux Émirats arabes unis, s'élève la _____ haute tour jamais construite.",
            dictee::MODE_EMPLOI,
            dictee::REECOUTER,
            "Bloqué ou loin ? Écris « Pixel, aide-moi » ou « Pixel, ramène-moi au coffre » dans le chat.",
        ]
    );
    // Ni le mot, ni « en minuscules », ni la cloche qui couvrirait la voix.
    for texte in jeu.textes() {
        assert!(!contient_la_reponse(&texte, &s.exercice), "{texte}");
        assert!(!texte.contains("minuscules"), "{texte}");
    }
    let commandes = jeu.commandes();
    assert!(!commandes.iter().any(|c| c.contains("bell")));
    // Sans clé à jour ni Écouteur : l'ancienne clé est retirée, les deux objets sont donnés.
    // Sans lecteur, rien n'est joué : aucune commande de son.
    assert_eq!(commandes.len(), 5, "{commandes:?}");
    assert_eq!(
        commandes[0],
        format!("clear Steve {} 0", predicat_cle_a_jour())
    );
    assert_eq!(
        commandes[1],
        "clear Steve minecraft:trial_key[minecraft:custom_data={mdr_cle:1b}]"
    );
    assert!(commandes[2].starts_with("give Steve minecraft:trial_key["));
    assert_eq!(
        commandes[3],
        format!("clear Steve {} 0", predicat_ecouteur())
    );
    assert_eq!(commandes[4], commande_ecouteur(PSEUDO).unwrap());
}

#[tokio::test]
async fn rappel_d_une_dictee_sans_redonner_ce_qu_il_a() {
    let jeu = Rc::new(FauxJeu::default());
    let coffre = coffre(&jeu).await;
    jeu.possede
        .borrow_mut()
        .extend(["minecraft:lore=".to_owned(), "mdr_ecouteur".to_owned()]);
    let s = trou("Dubaï", 7);
    coffre.rappeler(&s).await.unwrap();
    assert_eq!(jeu.textes()[0], "Un coffre de dictée t'attend à l'atelier.");
    assert!(jeu.textes()[1].contains("la ville de _____,"));
    let commandes = jeu.commandes();
    assert_eq!(commandes.len(), 2, "{commandes:?}");
    assert!(!commandes.iter().any(|c| c.starts_with("give ")));
}

#[tokio::test]
async fn rangement_et_retrait_rendent_l_ecouteur_muet() {
    let jeu = Rc::new(FauxJeu::default());
    let coffre = coffre(&jeu).await;
    coffre.ranger(&trou("plus", 7)).await.unwrap();
    let commandes = jeu.commandes();
    assert_eq!(commandes[commandes.len() - 2..], commandes_effacement());
    jeu.vider();
    coffre
        .retirer(Some(&trou("plus", 7).exercice))
        .await
        .unwrap();
    let commandes = jeu.commandes();
    // L'Écouteur redevient muet, puis le tableau de la dictée disparaît.
    assert_eq!(
        commandes[commandes.len() - 3..commandes.len() - 1],
        commandes_effacement()
    );
    assert_eq!(
        commandes[commandes.len() - 1],
        mine_de_rien::games::tableau::commande_effacement()
    );
}

#[tokio::test]
async fn une_cle_juste_a_la_casse_pres_devient_le_verrou() {
    let jeu = Rc::new(FauxJeu::default());
    let coffre = coffre(&jeu).await;
    coffre.accepter_la_cle(" L'Homme").await.unwrap();
    let commandes = jeu.commandes();
    assert!(
        commandes
            .iter()
            .any(|c| c
                .ends_with(r#"lock set value {components:{"minecraft:custom_name":" L'Homme"}}"#)),
        "{commandes:?}"
    );
    assert_eq!(
        jeu.textes(),
        [mine_de_rien::games::coffre_du_soir::CLE_JUSTE]
    );
}

#[test]
fn ni_la_casse_ni_les_espaces_ne_comptent_mais_les_accents_si() {
    use mine_de_rien::exercises::base::meme_cle;
    for ecrit in ["Dubaï", "dubaï", " DUBAÏ ", "du baï"] {
        assert!(meme_cle(ecrit, "Dubaï"), "{ecrit}");
    }
    assert!(meme_cle("Élève", "élève"));
    for ecrit in ["dubai", "duba", "", "dubaïs"] {
        assert!(!meme_cle(ecrit, "Dubaï"), "{ecrit}");
    }
    assert!(!meme_cle("a", "à"));
    assert!(meme_cle("29 ", "29"));
}

#[tokio::test]
async fn message_sans_chat_vocal() {
    let jeu = Rc::new(FauxJeu::default());
    let coffre = coffre(&jeu).await;
    coffre.demander_le_chat_vocal().await.unwrap();
    assert_eq!(jeu.textes(), [dictee::SANS_CHAT_VOCAL]);
    assert!(dictee::SANS_CHAT_VOCAL.contains("Demande à un parent d'installer le chat vocal"));
    // Sans lecteur, le jeu de l'enfant n'est jamais tenu pour relié, et rien ne se prépare.
    assert!(!coffre.chat_vocal_relie().await.unwrap());
    assert!(coffre.preparer_la_dictee(&trou("plus", 7)).await.is_err());
}

/// Un coffre du soir qui sait lire les dictées, sur une base où `TEXTE` est une dictée
/// confirmée : la fausse diffusion note ce qui est gardé, désigné et joué.
struct AvecVoix {
    _dossier: tempfile::TempDir,
    jeu: Rc<FauxJeu>,
    coffre: CoffreDuSoir,
    diffusion: Rc<FausseDiffusion>,
    voix: Rc<VoixDeTest>,
    /// Le trou « plus » : sa phrase, et les noms de ses deux sons.
    trou: ExerciceStocke,
    phrase: String,
    normal: String,
    lent: String,
}

async fn avec_voix() -> AvecVoix {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Rc::new(Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap());
    let trou = stocke(commun::trou_en_base(&depot, TEXTE, "plus"), 12);
    let (diffusion, voix) = (
        Rc::new(FausseDiffusion::new()),
        Rc::new(VoixDeTest::default()),
    );
    let lecteur = Rc::new(Lecteur::new(diffusion.clone(), voix.clone(), depot));
    let jeu = Rc::new(FauxJeu::default());
    let coffre = CoffreDuSoir::new(jeu.clone(), PSEUDO, 3, Some("Pixel"))
        .unwrap()
        .avec_lecteur(lecteur);
    coffre.actualiser_atelier().await.unwrap();
    jeu.vider();
    AvecVoix {
        _dossier: dossier,
        jeu,
        coffre,
        diffusion,
        voix,
        phrase: decouper(TEXTE)[0].texte.clone(),
        normal: fichier_du_son(dictee::son(&trou.exercice).unwrap()),
        lent: fichier_du_son(dictee::son_lent(&trou.exercice).unwrap()),
        trou,
    }
}

#[tokio::test]
async fn la_phrase_est_dite_a_la_pose_et_au_rappel_par_le_chat_vocal() {
    let m = avec_voix().await;
    assert!(m.coffre.chat_vocal_relie().await.unwrap());

    // Avant la pose : la voix lit la phrase aux deux vitesses, les sons sont gardés.
    m.coffre.preparer_la_dictee(&m.trou).await.unwrap();
    assert_eq!(
        *m.voix.lus.borrow(),
        [(m.phrase.clone(), false), (m.phrase.clone(), true)]
    );
    let gardes = [format!("garder {}", m.normal), format!("garder {}", m.lent)];
    assert_eq!(m.diffusion.actions(), gardes);
    // Les noms sont des numéros : aucun mot de la phrase.
    assert!(m.normal.starts_with("phrase-") && m.lent.starts_with("phrase-"));
    // Une seconde préparation ne redemande rien à la voix.
    m.coffre.preparer_la_dictee(&m.trou).await.unwrap();
    assert_eq!(m.voix.lus.borrow().len(), 2);

    m.coffre.poser(&m.trou).await.unwrap();
    m.diffusion.actions.borrow_mut().clear();

    // À l'annonce : la phrase est désignée à l'Écouteur, puis dite.
    m.coffre.annoncer(&m.trou).await.unwrap();
    let presentee = [
        format!("désigner {}", m.normal),
        format!("redire {}", m.normal),
    ];
    assert_eq!(m.diffusion.actions(), presentee);
    // Au rappel de connexion : redésignée (le serveur a pu redémarrer) et redite.
    m.diffusion.actions.borrow_mut().clear();
    m.coffre.rappeler(&m.trou).await.unwrap();
    assert_eq!(m.diffusion.actions(), presentee);

    // Plus aucun son de dictée ne passe par une commande du jeu.
    let commandes = m.jeu.commandes();
    assert!(
        !commandes
            .iter()
            .any(|c| c.contains("playsound mine_de_rien:") || c.contains("dictee/jouer")),
        "{commandes:?}"
    );
    // Et rien de ce qui est écrit ne donne le mot.
    for texte in m.jeu.textes() {
        assert!(!contient_la_reponse(&texte, &m.trou.exercice), "{texte}");
    }
}

#[tokio::test]
async fn rien_n_est_dit_pour_un_coffre_de_calcul() {
    let m = avec_voix().await;
    let s = maths();
    m.coffre.poser(&s).await.unwrap();
    m.coffre.annoncer(&s).await.unwrap();
    m.coffre.rappeler(&s).await.unwrap();
    m.coffre.ranger(&s).await.unwrap();
    m.coffre.retirer(None).await.unwrap();
    assert!(
        m.diffusion.actions().is_empty(),
        "{:?}",
        m.diffusion.actions()
    );
    assert!(m.voix.lus.borrow().is_empty());
}

#[tokio::test]
async fn a_l_ouverture_et_au_retrait_les_sons_sont_oublies() {
    for retrait in [false, true] {
        let m = avec_voix().await;
        m.coffre.preparer_la_dictee(&m.trou).await.unwrap();
        m.coffre.annoncer(&m.trou).await.unwrap();
        m.diffusion.actions.borrow_mut().clear();
        if retrait {
            m.coffre.retirer(Some(&m.trou.exercice)).await.unwrap();
        } else {
            m.coffre.ranger(&m.trou).await.unwrap();
        }
        assert_eq!(
            m.diffusion.actions(),
            [
                "désigner rien".to_owned(),
                format!("oublier {}", m.normal),
                format!("oublier {}", m.lent)
            ]
        );
        assert!(m.diffusion.gardes.borrow().is_empty());
        assert_eq!(*m.diffusion.designe.borrow(), None);
    }
}

#[tokio::test]
async fn une_voix_en_panne_ne_casse_ni_l_annonce_ni_le_rangement() {
    // Le serveur refuse de jouer (enfant sans chat vocal) : l'annonce reste écrite.
    let m = avec_voix().await;
    m.coffre.preparer_la_dictee(&m.trou).await.unwrap();
    m.diffusion.en_panne.set(true);
    m.coffre.annoncer(&m.trou).await.unwrap();
    assert_eq!(m.jeu.textes()[0], dictee::ANNONCE);
    m.coffre.ranger(&m.trou).await.unwrap();

    // Piper ne répond pas : la préparation le dit, l'annonce passe quand même.
    let m = avec_voix().await;
    m.voix.en_panne.set(true);
    let erreur = m.coffre.preparer_la_dictee(&m.trou).await.unwrap_err();
    assert!(erreur.to_string().contains("voix locale"), "{erreur}");
    m.coffre.annoncer(&m.trou).await.unwrap();
    assert_eq!(m.jeu.textes()[0], dictee::ANNONCE);
    assert!(m.diffusion.actions().is_empty());
}

#[test]
fn les_trois_paliers_ne_donnent_jamais_le_mot() {
    let textes = [
        TEXTE,
        "Le mot a trois lettres et il commence par la première. Écoute la phrase, je relis bien \
         lentement. Réécoute plus doucement cette fois. Il y en a deux à écrire. Nombre : huit.",
        "Deux-millions de personnes la visitent chaque année, c'est aujourd'hui.",
    ];
    for texte in textes {
        let phrases = decouper(texte);
        for position in candidats(&phrases) {
            let ex = exercice_sonore(&phrases, position, 1).unwrap();
            let paliers = indices(&ex);
            assert_eq!(paliers.len(), 3, "{ex:?} : {paliers:?}");
            for palier in &paliers {
                assert!(
                    !contient_la_reponse(palier, &ex),
                    "{} : {palier}",
                    ex.reponse
                );
                assert!(palier.chars().count() <= 150, "{palier}");
            }
        }
    }
    let palier = |mot: &str| {
        let phrases = decouper(&format!("Voici {mot} ici."));
        indices(&exercice_sonore(&phrases, Position { phrase: 0, mot: 1 }, 1).unwrap())
    };
    assert_eq!(
        palier("plus"),
        [
            "Écoute bien, je relis la phrase lentement.",
            "Le mot a 4 lettres.",
            "Il commence par « p » et a 4 lettres.",
        ]
    );
    assert_eq!(
        palier("s'élève")[1],
        "Le mot a 6 lettres, avec une apostrophe."
    );
    assert_eq!(
        palier("huit-cent-vingt-huit")[2],
        "Il commence par « h » et a 17 lettres, avec 3 traits d'union."
    );
    assert_eq!(palier("Dubaï")[2], "Il commence par « D » et a 5 lettres.");
    // Le mot est dans la formulation habituelle : l'autre la remplace.
    assert_eq!(palier("mot")[1], "Il y en a 3 à écrire.");
    assert_eq!(palier("lettres")[2], "Première : « l ». Nombre : 7.");
    assert_eq!(palier("phrase")[0], "Réécoute, plus doucement cette fois.");
}

#[test]
fn l_adresse_du_jeu_du_pack_se_lit_encore() {
    use mine_de_rien::config::Pack;
    let pack = |adresse: &str| Pack {
        adresse_du_jeu: adresse.to_owned(),
        ..Pack::default()
    };
    assert_eq!(pack("").transfert(), Ok(None));
    assert_eq!(
        pack("192.168.1.20").transfert(),
        Ok(Some(("192.168.1.20".to_owned(), 25565)))
    );
    assert_eq!(
        pack(" localhost:25567 ").transfert(),
        Ok(Some(("localhost".to_owned(), 25567)))
    );
    // Rien qui puisse devenir une autre commande, ni un port impossible.
    for faux in [
        "maison 25565 @a",
        "hote:port",
        "hote:99999",
        ":25565",
        "a;b",
    ] {
        assert!(pack(faux).transfert().is_err(), "{faux}");
    }
}
