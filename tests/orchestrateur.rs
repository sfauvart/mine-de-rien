//! Commandes des parents : texte, photo d'une fiche (lecture, tirage, /oui, /non), /atelier.
//! Sans réseau ni LLM réel.

mod commun;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::io::Cursor;
use std::rc::Rc;

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use commun::terrain::{Terrain, Y};
use image::{ImageFormat, Rgb, RgbImage};
use mine_de_rien::adapters::channel::base::{Canal, MessageParent};
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::adapters::llm::base::{ErreurLlm, Llm};
use mine_de_rien::alea::Alea;
use mine_de_rien::config::Coffre as ConfigCoffre;
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::games::coffre_du_soir::{Coffre, CoffreDuSoir, Notificateur};
use mine_de_rien::games::emplacement::{AUCUN_TERRAIN, ENFANT_ABSENT};
use mine_de_rien::minecraft::texte::Style;
use mine_de_rien::orchestrateur::{
    AIDE, CommandesParents, LECTURE_EN_COURS, Orchestrateur, RAPPEL_FIDELITE, RECHERCHE_EN_COURS,
    servir,
};
use mine_de_rien::photo;
use mine_de_rien::saisie::Source;
use mine_de_rien::storage::sqlite::{Demande, Depot};
use serde_json::{Value, json};

const PARENTS: [i64; 2] = [111, 222];

fn doubles() -> Value {
    let lignes: Vec<Value> = (1..=20)
        .map(|n| json!({"genre": "calcul", "texte": format!("Double de {n}")}))
        .collect();
    json!({"lignes": lignes})
}

fn jpeg() -> Vec<u8> {
    let blanche = RgbImage::from_pixel(600, 800, Rgb([255, 255, 255]));
    let mut tampon = Cursor::new(Vec::new());
    blanche.write_to(&mut tampon, ImageFormat::Jpeg).unwrap();
    tampon.into_inner()
}

struct FauxLlm {
    reponse: Value,
    distant: bool,
    /// Répond du texte libre au lieu du JSON demandé.
    bavard: bool,
    consignes: RefCell<Vec<String>>,
}

impl FauxLlm {
    fn nouveau(reponse: Value) -> Rc<Self> {
        Rc::new(Self {
            reponse,
            distant: false,
            bavard: false,
            consignes: RefCell::new(Vec::new()),
        })
    }

    fn consignes(&self) -> Vec<String> {
        self.consignes.borrow().clone()
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
        _image: Option<&[u8]>,
        _en_json: bool,
    ) -> Result<String, ErreurLlm> {
        self.consignes.borrow_mut().push(utilisateur.to_owned());
        Ok(if self.bavard {
            "Voici ce que je vois".to_owned()
        } else {
            self.reponse.to_string()
        })
    }
}

type Temps = Rc<Cell<DateTime<Utc>>>;

fn horloge() -> Temps {
    Rc::new(Cell::new(commun::instant("2026-11-08T18:00:00+00:00")))
}

fn depot() -> (tempfile::TempDir, Rc<Depot>) {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Rc::new(Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap());
    (dossier, depot)
}

fn commandes_avec(
    depot: &Rc<Depot>,
    coffre: Option<Rc<CoffreDuSoir>>,
    llm: Option<Rc<FauxLlm>>,
    temps: &Temps,
) -> CommandesParents {
    let lu = Rc::clone(temps);
    CommandesParents::new(
        depot.clone(),
        ConfigCoffre::default(),
        coffre,
        &PARENTS,
        Box::new(move || lu.get()),
        llm.map(|llm| llm as Rc<dyn Llm>),
        Alea::graine(0),
    )
}

fn commandes(depot: &Rc<Depot>, llm: Option<Rc<FauxLlm>>) -> CommandesParents {
    commandes_avec(depot, None, llm, &horloge())
}

fn avec_image(expediteur: i64, legende: &str) -> MessageParent {
    MessageParent {
        expediteur,
        texte: legende.to_owned(),
        image: Some(jpeg()),
    }
}

async fn envoie_photo(c: &CommandesParents, expediteur: i64, legende: &str) -> Option<String> {
    c.traiter(&avec_image(expediteur, legende)).await.unwrap()
}

async fn photo_de(c: &CommandesParents, legende: &str) -> String {
    envoie_photo(c, 111, legende).await.unwrap()
}

async fn texte_de(c: &CommandesParents, expediteur: i64, texte: &str) -> Option<String> {
    c.traiter(&MessageParent::texte(expediteur, texte))
        .await
        .unwrap()
}

async fn texte(c: &CommandesParents, texte: &str) -> String {
    texte_de(c, 111, texte).await.unwrap()
}

#[tokio::test]
async fn photo_d_un_inconnu_ignoree() {
    let (_dossier, depot) = depot();
    let llm = FauxLlm::nouveau(doubles());
    let c = commandes(&depot, Some(llm.clone()));
    assert_eq!(envoie_photo(&c, 333, "").await, None);
    let inconnu = MessageParent {
        expediteur: 333,
        texte: String::new(),
        image: Some(vec![b'x']),
    };
    assert_eq!(c.accuse_reception(&inconnu), None);
    assert!(llm.consignes().is_empty());
}

#[tokio::test]
async fn llm_distant_refuse_sans_appel() {
    let (_dossier, depot) = depot();
    let llm = Rc::new(FauxLlm {
        reponse: doubles(),
        distant: true,
        bavard: false,
        consignes: RefCell::new(Vec::new()),
    });
    let c = commandes(&depot, Some(llm.clone()));
    assert_eq!(c.accuse_reception(&avec_image(111, "")), None);
    assert!(photo_de(&c, "").await.contains("LLM local"));
    assert!(llm.consignes().is_empty());
}

#[tokio::test]
async fn sans_llm_configure() {
    let (_dossier, depot) = depot();
    assert!(
        photo_de(&commandes(&depot, None), "")
            .await
            .contains("pas configurée")
    );
}

#[tokio::test]
async fn legende_nombre_et_consigne() {
    let (_dossier, depot) = depot();
    let llm = FauxLlm::nouveau(doubles());
    let c = commandes(&depot, Some(llm.clone()));
    let reponse = photo_de(&c, "5 exercice 7").await;
    assert!(llm.consignes()[0].contains("exercice 7"));
    assert!(reponse.contains("Doubles de (20)"));
    assert!(reponse.contains("Tirés au hasard (5)"));
    assert!(reponse.contains("Compare cette liste à la fiche"));
    assert_eq!(c.lecture_en_attente(111).unwrap().exercices.len(), 5);
    assert!(depot.en_file().unwrap().is_empty());
}

#[tokio::test]
async fn legende_par_defaut_et_bornes() {
    let (_dossier, depot) = depot();
    let c = commandes(&depot, Some(FauxLlm::nouveau(doubles())));
    assert!(photo_de(&c, "").await.contains("Tirés au hasard (3)"));
    assert!(photo_de(&c, "11").await.contains("Entre 1 et 10"));
    assert!(photo_de(&c, "0").await.contains("Entre 1 et 10"));
}

#[tokio::test]
async fn oui_met_en_file_avec_la_source_photo() {
    let (_dossier, depot) = depot();
    let c = commandes(&depot, Some(FauxLlm::nouveau(doubles())));
    photo_de(&c, "2").await;
    let reponse = texte(&c, "/oui").await;
    assert_eq!(reponse.matches("en file").count(), 2);
    let file = depot.en_file().unwrap();
    assert!(
        file.iter()
            .all(|s| s.source == Source::Photo && s.demande_par == "111")
    );
    assert_eq!(file.len(), 2);
    assert!(file[0].exercice.enonce.starts_with("Double de "));
    assert!(texte(&c, "/oui").await.contains("Rien à confirmer"));
}

#[tokio::test]
async fn non_abandonne() {
    let (_dossier, depot) = depot();
    let c = commandes(&depot, Some(FauxLlm::nouveau(doubles())));
    photo_de(&c, "").await;
    assert!(texte(&c, "/non").await.contains("Abandonné"));
    assert!(depot.en_file().unwrap().is_empty());
    assert!(texte(&c, "/non").await.contains("Aucune lecture"));
}

#[tokio::test]
async fn lecture_expiree_apres_30_minutes() {
    let (_dossier, depot) = depot();
    let temps = horloge();
    let c = commandes_avec(&depot, None, Some(FauxLlm::nouveau(doubles())), &temps);
    photo_de(&c, "").await;
    temps.set(temps.get() + TimeDelta::minutes(31));
    assert!(texte(&c, "/oui").await.contains("Rien à confirmer"));
    assert!(depot.en_file().unwrap().is_empty());
}

#[tokio::test]
async fn une_lecture_par_parent() {
    let (_dossier, depot) = depot();
    let c = commandes(&depot, Some(FauxLlm::nouveau(doubles())));
    envoie_photo(&c, 111, "1").await;
    envoie_photo(&c, 222, "2").await;
    assert!(
        !texte_de(&c, 222, "/oui")
            .await
            .unwrap()
            .contains("Rien à confirmer")
    );
    assert_eq!(depot.en_file().unwrap().len(), 2);
    assert_eq!(c.lecture_en_attente(111).unwrap().exercices.len(), 1);
}

#[tokio::test]
async fn rien_reconnu() {
    let (_dossier, depot) = depot();
    let reponse = json!({"lignes": [{"genre": "mot", "texte": "Greetings"}]});
    let c = commandes(&depot, Some(FauxLlm::nouveau(reponse)));
    assert_eq!(
        photo_de(&c, "").await,
        "Je n'ai rien reconnu. La photo est-elle à l'endroit ?"
    );
    assert!(c.lecture_en_attente(111).is_none());
}

#[tokio::test]
async fn reponse_illisible_du_llm() {
    let (_dossier, depot) = depot();
    let llm = Rc::new(FauxLlm {
        reponse: doubles(),
        distant: false,
        bavard: true,
        consignes: RefCell::new(Vec::new()),
    });
    assert!(
        photo_de(&commandes(&depot, Some(llm)), "")
            .await
            .starts_with("Lecture impossible")
    );
}

struct FauxCanal {
    messages: RefCell<VecDeque<MessageParent>>,
    reponses: RefCell<Vec<(i64, String)>>,
}

impl FauxCanal {
    fn nouveau(messages: Vec<MessageParent>) -> Self {
        Self {
            messages: RefCell::new(messages.into()),
            reponses: RefCell::new(Vec::new()),
        }
    }
}

#[async_trait(?Send)]
impl Canal for FauxCanal {
    async fn envoyer(&self, texte: &str) {
        self.reponses.borrow_mut().push((0, texte.to_owned()));
    }

    async fn repondre(&self, destinataire: i64, texte: &str) {
        self.reponses
            .borrow_mut()
            .push((destinataire, texte.to_owned()));
    }

    async fn recevoir(&self) -> Option<MessageParent> {
        self.messages.borrow_mut().pop_front()
    }
}

#[tokio::test]
async fn servir_previent_avant_de_lire() {
    let (_dossier, depot) = depot();
    let c = commandes(&depot, Some(FauxLlm::nouveau(doubles())));
    let canal = FauxCanal::nouveau(vec![avec_image(111, "2")]);
    servir(&canal, &c).await;
    let reponses = canal.reponses.borrow();
    assert_eq!(reponses[0], (111, LECTURE_EN_COURS.to_owned()));
    assert!(reponses[1].1.contains("Tirés au hasard (2)"));
}

#[tokio::test]
async fn servir_ne_repond_qu_aux_parents() {
    let (_dossier, depot) = depot();
    let c = commandes(&depot, None);
    let canal = FauxCanal::nouveau(vec![
        MessageParent::texte(333, "/coffre 1 + 1"),
        MessageParent::texte(111, "/coffre 1 + 1"),
    ]);
    servir(&canal, &c).await;
    assert_eq!(
        *canal.reponses.borrow(),
        [(111, "Coffre n°1 en file : 1 + 1 = ? (réponse 2)".to_owned())]
    );
}

struct Atelier {
    terrain: Rc<Terrain>,
    coffre: Rc<CoffreDuSoir>,
    commandes: CommandesParents,
}

fn atelier(depot: &Rc<Depot>, terrain: Terrain) -> Atelier {
    let terrain = Rc::new(terrain);
    let coffre = Rc::new(CoffreDuSoir::new(terrain.clone(), "Steve_42", 3, None).unwrap());
    let commandes = commandes_avec(depot, Some(coffre.clone()), None, &horloge());
    Atelier {
        terrain,
        coffre,
        commandes,
    }
}

#[tokio::test]
async fn atelier_construit_sans_le_pseudo_dans_la_reponse() {
    let (_dossier, depot) = depot();
    let a = atelier(&depot, Terrain::default());
    let reponse = texte(&a.commandes, "/atelier").await;
    assert_eq!(
        reponse,
        "Atelier construit en 92 64 192, à 11 blocs de l'enfant."
    );
    assert!(!reponse.contains("Steve"));
}

#[tokio::test]
async fn atelier_inconnu_ignore() {
    let (_dossier, depot) = depot();
    let a = atelier(&depot, Terrain::default());
    assert_eq!(texte_de(&a.commandes, 333, "/atelier").await, None);
    assert_eq!(
        a.commandes
            .accuse_reception(&MessageParent::texte(333, "/atelier")),
        None
    );
    assert!(a.terrain.recues().is_empty());
}

#[tokio::test]
async fn atelier_accuse_reception_pour_un_parent() {
    let (_dossier, depot) = depot();
    let a = atelier(&depot, Terrain::default());
    assert_eq!(
        a.commandes
            .accuse_reception(&MessageParent::texte(111, "/atelier")),
        Some(RECHERCHE_EN_COURS)
    );
    assert_eq!(
        a.commandes
            .accuse_reception(&MessageParent::texte(111, "/rapport")),
        None
    );
}

#[tokio::test]
async fn atelier_enfant_hors_ligne_dit_quoi_faire() {
    let (_dossier, depot) = depot();
    let a = atelier(
        &depot,
        Terrain {
            joueur: None,
            ..Terrain::default()
        },
    );
    assert_eq!(texte(&a.commandes, "/atelier").await, ENFANT_ABSENT);
}

const ANCIEN: [i32; 3] = [40, 64, 40];
const MODIFIANTES: [&str; 6] = [
    "fill",
    "setblock",
    "data modify",
    "data remove",
    "forceload",
    "kill @e[type=minecraft:marker,tag=mdr_atelier",
];

fn modifiante(commande: &str) -> bool {
    MODIFIANTES.iter().any(|debut| commande.starts_with(debut))
}

fn terrain_avec_atelier() -> Terrain {
    Terrain {
        atelier: RefCell::new(Some(ANCIEN)),
        ..Terrain::default()
    }
}

fn pose(depot: &Depot) -> i64 {
    let demande = Demande {
        source: Source::Photo,
        demande_par: "1",
        recompense: "minecraft:diamond",
        recompense_nombre: 3,
    };
    let ident = depot
        .ajouter(
            &photo::calcul("Double de 17").unwrap(),
            demande,
            horloge().get(),
        )
        .unwrap();
    depot.marquer_pose(ident, horloge().get()).unwrap();
    ident
}

#[tokio::test]
async fn deplacement_dans_l_ordre() {
    let (_dossier, depot) = depot();
    let a = atelier(&depot, terrain_avec_atelier());
    let reponse = texte(&a.commandes, "/atelier").await;
    assert_eq!(
        reponse,
        "Atelier déplacé en 92 64 192, à 11 blocs de l'enfant."
    );
    assert_eq!(*a.terrain.atelier.borrow(), Some([92, 64, 192]));
    let recues = a.terrain.recues();
    let modifs: Vec<&String> = recues.iter().filter(|c| modifiante(c)).collect();
    let retraits: Vec<String> = ["40 64 40", "39 64 40", "41 64 40", "42 64 38"]
        .iter()
        .map(|p| format!("setblock {p} minecraft:air"))
        .collect();
    assert_eq!(
        modifs[..4].iter().map(|c| c.as_str()).collect::<Vec<_>>(),
        retraits
    );
    assert_eq!(modifs[4], "fill 38 63 38 42 63 42 minecraft:dirt");
    assert!(modifs[5].starts_with("kill") && modifs[5].contains("mdr_atelier"));
    assert_eq!(modifs[6], "forceload remove 38 38 42 42");
    assert_eq!(modifs[7], "data remove storage mine_de_rien:atelier pos");
    assert_eq!(modifs[8], "forceload add 90 190 94 194");
    assert_eq!(
        *modifs.last().unwrap(),
        "data modify storage mine_de_rien:atelier pos set value [92, 64, 192]"
    );
    assert_eq!(
        *a.terrain.dits.borrow(),
        [(
            "Steve_42".to_owned(),
            "Ton atelier vient d'apparaître, à 11 blocs vers l'ouest.".to_owned()
        )]
    );
}

#[tokio::test]
async fn deplacement_refuse_avec_un_coffre_actif() {
    let (_dossier, depot) = depot();
    let ident = pose(&depot);
    let a = atelier(&depot, terrain_avec_atelier());
    let reponse = texte(&a.commandes, "/atelier").await;
    assert!(reponse.contains(&format!("coffre n°{ident} est en cours")));
    assert!(!a.terrain.recues().iter().any(|c| modifiante(c)));
    assert_eq!(*a.terrain.atelier.borrow(), Some(ANCIEN));
}

#[tokio::test]
async fn deplacement_refuse_avec_un_coffre_plein() {
    let (_dossier, depot) = depot();
    let terrain = Terrain {
        blocs: HashMap::from([((40, 64, 40), "minecraft:chest")]),
        coffre_plein: true,
        ..terrain_avec_atelier()
    };
    let a = atelier(&depot, terrain);
    assert!(
        texte(&a.commandes, "/atelier")
            .await
            .contains("contient encore des objets")
    );
    assert!(!a.terrain.recues().iter().any(|c| modifiante(c)));
}

#[tokio::test]
async fn ancien_atelier_intact_si_rien_ne_convient() {
    let (_dossier, depot) = depot();
    let terrain = Terrain {
        relief: Some(|x, z| Y + (x + z).rem_euclid(2)),
        ..terrain_avec_atelier()
    };
    let a = atelier(&depot, terrain);
    assert_eq!(texte(&a.commandes, "/atelier").await, AUCUN_TERRAIN);
    assert!(!a.terrain.recues().iter().any(|c| modifiante(c)));
    assert_eq!(*a.terrain.atelier.borrow(), Some(ANCIEN));
}

/// Pas de coffre posé à l'ancien endroit pendant le déplacement.
#[tokio::test]
async fn atelier_attend_la_fin_du_tour_de_surveillance() {
    let (_dossier, depot) = depot();
    let a = Rc::new(atelier(&depot, terrain_avec_atelier()));
    let local = tokio::task::LocalSet::new();
    let reponse = local
        .run_until(async {
            let verrou = a.coffre.verrou().lock().await;
            let pour_la_tache = Rc::clone(&a);
            let tache = tokio::task::spawn_local(async move {
                texte(&pour_la_tache.commandes, "/atelier").await
            });
            tokio::task::yield_now().await;
            assert!(!tache.is_finished() && a.terrain.recues().is_empty());
            drop(verrou);
            tache.await.unwrap()
        })
        .await;
    assert!(reponse.starts_with("Atelier déplacé"));
}

#[tokio::test]
async fn annuler_refuse_ce_qui_n_est_pas_un_numero() {
    let (_dossier, depot) = depot();
    let c = commandes(&depot, None);
    // Ces deux saisies faisaient planter ou diverger la version Python : écarts assumés.
    for argument in ["²", "99999999999999999999", "-1", "1 2"] {
        let reponse = texte(&c, &format!("/annuler {argument}")).await;
        assert!(
            reponse.starts_with("Préciser le numéro"),
            "{argument} : {reponse}"
        );
    }
}

const PSEUDO: &str = "Zorglub42";

/// Le monde côté RCON : l'enfant est connecté et finit par ouvrir le coffre.
#[derive(Default)]
struct FauxServeur {
    ouvert: Cell<bool>,
}

#[async_trait(?Send)]
impl AdaptateurJeu for FauxServeur {
    async fn commande(&self, c: &str) -> Result<String, ErreurRcon> {
        let ok = |texte: &str| Ok(texte.to_owned());
        if c == "list" {
            return Ok(format!(
                "There are 1 of a max of 20 players online: {PSEUDO}"
            ));
        }
        if c == "data get storage mine_de_rien:atelier pos" {
            return ok("Storage mine_de_rien:atelier has the following contents: [0, -60, 8]");
        }
        if c.starts_with(&format!("clear {PSEUDO}")) && c.ends_with(" 0") {
            return Ok(if self.ouvert.get() {
                format!("Found 1 matching item(s) on player {PSEUDO}")
            } else {
                format!("No items were found on player {PSEUDO}")
            });
        }
        if c.contains("Items[0]") {
            return ok("Test failed");
        }
        if c.starts_with("execute if") {
            return ok("Test passed");
        }
        ok("")
    }

    async fn dire(&self, _: &str, _: &str, _: &[(&str, Style<'_>)]) -> Result<(), ErreurRcon> {
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat ici".to_owned()))
    }
}

#[derive(Default)]
struct Envoyes(RefCell<Vec<String>>);

#[async_trait(?Send)]
impl Notificateur for Envoyes {
    async fn notifier(&self, message: &str) {
        self.0.borrow_mut().push(message.to_owned());
    }
}

#[tokio::test]
async fn aucun_message_aux_parents_ne_contient_le_pseudo() {
    let (_dossier, depot) = depot();
    let serveur = Rc::new(FauxServeur::default());
    let coffre = Rc::new(CoffreDuSoir::new(serveur.clone(), PSEUDO, 3, None).unwrap());
    let envoyes = Rc::new(Envoyes::default());
    let temps: Temps = Rc::new(Cell::new(commun::instant("2026-11-14T18:00:00+00:00")));
    let lu = Rc::clone(&temps);
    let orch = Orchestrateur::new(
        depot.clone(),
        coffre.clone(),
        envoyes.clone(),
        Box::new(move || lu.get()),
        None,
    );
    let c = commandes_avec(&depot, Some(coffre), None, &temps);
    let note = |reponse: String| envoyes.0.borrow_mut().push(reponse);

    for message in ["/coffre 348 ÷ 12", "/coffre division 2", "/file"] {
        note(texte(&c, message).await);
    }
    orch.tour().await.unwrap(); // posé
    temps.set(temps.get() + TimeDelta::minutes(3));
    serveur.ouvert.set(true);
    orch.tour().await.unwrap(); // ouvert
    for message in ["/file", "/rapport", "/annuler 3"] {
        note(texte(&c, message).await);
    }

    let envoyes = envoyes.0.borrow();
    assert!(
        envoyes
            .iter()
            .any(|m| m.contains("Coffre n°1 ouvert en 3 min"))
    );
    assert!(envoyes.iter().all(|m| !m.contains(PSEUDO)), "{envoyes:?}");
}

fn llm_du_scenario(config: &Value) -> Option<Rc<FauxLlm>> {
    let config = config.as_object()?;
    let vrai = |cle: &str| config.get(cle).and_then(Value::as_bool).unwrap_or(false);
    Some(Rc::new(FauxLlm {
        reponse: config.get("reponse").cloned().unwrap_or(Value::Null),
        distant: vrai("distant"),
        bavard: vrai("bavard"),
        consignes: RefCell::new(Vec::new()),
    }))
}

async fn jouer_scenario(cas: &Value, rejeu: Rc<commun::Rejeu>) -> Value {
    let e = &cas["entree"];
    let (_dossier, depot) = depot();
    let t0 = commun::instant("2026-11-08T18:00:00+00:00");
    let demande = Demande {
        source: Source::Expression,
        demande_par: "cli",
        recompense: "minecraft:diamond",
        recompense_nombre: 3,
    };
    for op in e["avant"].as_array().unwrap() {
        let entier = |i: usize| op[i].as_i64().unwrap();
        match op[0].as_str().unwrap() {
            "ajouter" => {
                let exercice: Exercice = serde_json::from_value(op[1].clone()).unwrap();
                depot.ajouter(&exercice, demande, t0).unwrap();
            }
            "poser" => depot
                .marquer_pose(entier(1), t0 + TimeDelta::minutes(1))
                .unwrap(),
            "ouvrir" => depot
                .marquer_ouvert(entier(1), t0 + TimeDelta::minutes(1 + entier(2)))
                .unwrap(),
            "indices" => {
                for _ in 0..entier(2) {
                    depot
                        .noter("indice", Some(entier(1)), t0 + TimeDelta::minutes(2))
                        .unwrap();
                }
            }
            autre => panic!("opération inconnue : {autre}"),
        }
    }
    let coffre = e["jeu"].as_bool().unwrap().then(|| {
        Rc::new(CoffreDuSoir::new(rejeu, e["pseudo"].as_str().unwrap(), 3, None).unwrap())
    });
    let llm = llm_du_scenario(&e["llm"]);
    let temps: Temps = Rc::new(Cell::new(t0 + TimeDelta::minutes(2)));
    let c = commandes_avec(&depot, coffre, llm.clone(), &temps);
    let (mut reponses, mut accuses) = (Vec::new(), Vec::new());
    for message in e["messages"].as_array().unwrap() {
        temps.set(temps.get() + TimeDelta::minutes(message[3].as_i64().unwrap_or(0)));
        let recu = MessageParent {
            expediteur: message[0].as_i64().unwrap(),
            texte: message[1].as_str().unwrap().to_owned(),
            image: message[2].as_bool().unwrap_or(false).then(jpeg),
        };
        accuses.push(c.accuse_reception(&recu));
        reponses.push(match c.traiter(&recu).await {
            Ok(reponse) => reponse,
            Err(erreur) => Some(format!("Erreur : {erreur}")),
        });
    }
    let coffres: Vec<Value> = depot
        .entre(t0 - TimeDelta::days(1), t0 + TimeDelta::days(30))
        .unwrap()
        .iter()
        .map(|s| {
            json!([
                s.id,
                s.statut.nom(),
                s.source.nom(),
                s.demande_par,
                s.exercice.enonce,
                s.exercice.reponse,
                s.recompense,
                s.recompense_nombre
            ])
        })
        .collect();
    let en_attente: serde_json::Map<String, Value> = [111, 222, 333]
        .into_iter()
        .filter_map(|parent| {
            let lecture = c.lecture_en_attente(parent)?;
            let enonces: Vec<&str> = lecture
                .exercices
                .iter()
                .map(|ex| ex.enonce.as_str())
                .collect();
            Some((parent.to_string(), json!(enonces)))
        })
        .collect();
    json!({
        "reponses": reponses,
        "accuses": accuses,
        "coffres": coffres,
        "en_attente": en_attente,
        "consignes": llm.map(|llm| llm.consignes()),
    })
}

/// Vingt-quatre conversations de parents : mêmes réponses que la version Python, au
/// caractère près, mêmes commandes envoyées au jeu, même file à la fin.
#[tokio::test]
async fn oracle_python() {
    for cas in commun::oracle("commandes_parents") {
        let e = &cas["entree"];
        let rejeu = Rc::new(commun::Rejeu::new(&cas));
        match e["f"].as_str().unwrap() {
            "constantes" => {
                let s = &cas["sortie"];
                assert_eq!(s["aide"], AIDE);
                assert_eq!(s["lecture"], LECTURE_EN_COURS);
                assert_eq!(s["recherche"], RECHERCHE_EN_COURS);
                assert_eq!(s["fidelite"], RAPPEL_FIDELITE);
            }
            "commande" => {} // couvert par les conversations : la fonction n'est pas publique
            "scenario" => {
                let sortie = jouer_scenario(&cas, rejeu.clone()).await;
                commun::conclure::<_, String>(&cas, &rejeu, Ok(sortie));
            }
            autre => panic!("fonction inconnue : {autre}"),
        }
    }
}
