mod commun;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use chrono::{DateTime, TimeDelta, Utc};
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::games::atelier::Atelier;
use mine_de_rien::games::coffre_du_soir::{
    Coffre, CoffreDuSoir, ErreurCoffre, Notificateur, commande_cle, commandes_pose,
    commandes_rangement, commandes_retrait, formater_duree, predicat_cle, predicat_preuve,
};
use mine_de_rien::games::paisible::ModePaisible;
use mine_de_rien::minecraft::texte::{Style, lignes_panneau};
use mine_de_rien::orchestrateur::Orchestrateur;
use mine_de_rien::saisie::Source;
use mine_de_rien::storage::sqlite::{Demande, Depot, ExerciceStocke, Statut};
use serde_json::{Value, json};
use tokio::sync::Mutex;

const A: Atelier = Atelier { x: 0, y: -60, z: 8 };

fn t0() -> DateTime<Utc> {
    commun::instant("2026-11-07T18:00:00+00:00")
}

fn stocke_de(exercice: Exercice, ident: i64) -> ExerciceStocke {
    ExerciceStocke {
        id: ident,
        exercice,
        jeu: "coffre-du-soir".to_owned(),
        source: Source::Expression,
        statut: Statut::EnFile,
        demande_par: "cli".to_owned(),
        recompense: "minecraft:diamond".to_owned(),
        recompense_nombre: 3,
        cree_le: t0(),
        pose_le: None,
        ouvert_le: None,
        niveaux_donnes: false,
    }
}

fn stocke(enonce: &str, reponse: &str) -> ExerciceStocke {
    let exercice = Exercice {
        competence: "maths/division-posee".to_owned(),
        enonce: enonce.to_owned(),
        reponse: reponse.to_owned(),
        donnees: Default::default(),
    };
    stocke_de(exercice, 12)
}

fn defaut() -> ExerciceStocke {
    stocke("348 ÷ 12 = ?", "29")
}

#[test]
fn commandes_pour_un_exercice_fixe() {
    assert_eq!(
        commandes_pose(&A, &defaut()).unwrap(),
        [
            "execute unless block 0 -60 8 minecraft:chest run setblock 0 -60 8 minecraft:chest[facing=south]",
            "data modify block 0 -60 8 lock set value {components:{\"minecraft:custom_name\":\"29\"}}",
            "item replace block 0 -60 8 container.13 with minecraft:diamond 3",
            "item replace block 0 -60 8 container.11 with minecraft:paper[minecraft:custom_name=\"Preuve n°12\",minecraft:custom_data={mdr_coffre:12}]",
            "setblock -1 -60 8 minecraft:oak_sign[rotation=0]{front_text:{messages:[\"348 ÷ 12 = ?\",\"\",\"\",\"\"]},is_waxed:1b}",
            "setblock 1 -60 8 minecraft:anvil[facing=east]",
        ]
    );
}

#[test]
fn le_verrou_est_pose_avant_la_recompense() {
    let commandes = commandes_pose(&A, &defaut()).unwrap();
    let verrou = commandes.iter().position(|c| c.contains(" lock ")).unwrap();
    let recompense = commandes
        .iter()
        .position(|c| c.contains("item replace"))
        .unwrap();
    assert!(verrou < recompense);
}

#[test]
fn decoupage_du_panneau() {
    assert_eq!(
        lignes_panneau("Le fermier a 348 œufs en boîtes de 12. Combien ?").unwrap(),
        ["Le fermier a", "348 œufs en", "boîtes de 12.", "Combien ?"]
    );
    assert_eq!(
        lignes_panneau("(12 + 8) × 3 = ?").unwrap(),
        ["(12 + 8) × 3 =", "?", "", ""]
    );
    assert!(lignes_panneau("un mot-beaucoup-trop-long").is_none());
    assert!(lignes_panneau(&["abcdefgh"; 5].join(" ")).is_none());
}

#[test]
fn enonce_piege_echappe_sur_le_panneau() {
    let piege = "l'élève \"a\" \\ ]}";
    let commandes = commandes_pose(&A, &stocke(piege, "29")).unwrap();
    let panneau: Vec<&String> = commandes
        .iter()
        .filter(|c| c.contains("oak_sign"))
        .collect();
    assert_eq!(panneau.len(), 1);
    let debut = panneau[0].find("messages:").unwrap() + "messages:".len();
    let fin = panneau[0].rfind("]}").unwrap() + 1;
    let messages: Value = serde_json::from_str(&panneau[0][debut..fin]).unwrap();
    assert_eq!(messages, json!(["l'élève \"a\" \\", "]}", "", ""]));
}

#[test]
fn reponse_decimale_dans_le_verrou() {
    let commandes = commandes_pose(&A, &stocke("3,5 + 1,25 = ?", "4,75")).unwrap();
    assert!(commandes[1].contains("\"minecraft:custom_name\":\"4,75\""));
}

#[test]
fn enonce_trop_long_refuse() {
    let erreur = commandes_pose(&A, &stocke(&["abcdefgh"; 5].join(" "), "29")).unwrap_err();
    assert!(matches!(erreur, ErreurCoffre::Coffre(_)));
}

#[test]
fn rangement_et_retrait() {
    assert_eq!(
        commandes_rangement(&A, "Steve", 12).unwrap(),
        [
            "clear Steve minecraft:paper[minecraft:custom_data={mdr_coffre:12}]",
            "setblock -1 -60 8 minecraft:air",
            "data remove block 0 -60 8 lock",
        ]
    );
    assert!(
        commandes_retrait(&A).contains(&"data modify block 0 -60 8 Items set value []".to_owned())
    );
    assert!(commandes_rangement(&A, "@a", 12).is_err());
}

#[test]
fn cle_avec_mode_d_emploi() {
    let commande = commande_cle("Steve").unwrap();
    assert!(
        commande
            .starts_with("give Steve minecraft:trial_key[minecraft:item_name=\"Clé du coffre\",")
    );
    assert!(commande.ends_with(",minecraft:custom_data={mdr_cle:1b}] 1"));
    let debut = commande.find("minecraft:lore=").unwrap() + "minecraft:lore=".len();
    let fin = commande.find("],minecraft:custom_data").unwrap() + 1;
    let lore: Value = serde_json::from_str(&commande[debut..fin]).unwrap();
    assert_eq!(lore.as_array().unwrap().len(), 4);
    assert_eq!(lore[0]["text"], "1. Pose-moi à gauche sur l'enclume");
    assert!(
        lore.as_array()
            .unwrap()
            .iter()
            .all(|ligne| ligne["italic"] == false)
    );
    assert_eq!(
        predicat_cle(),
        "minecraft:trial_key[minecraft:custom_data={mdr_cle:1b}]"
    );
    assert!(commande_cle("@a").is_err());
}

#[test]
fn formater_duree_arrondit_vers_le_bas() {
    assert_eq!(
        formater_duree(Some(TimeDelta::seconds(30))),
        "moins d'une minute"
    );
    assert_eq!(
        formater_duree(Some(TimeDelta::seconds(4 * 60 + 50))),
        "4 min"
    );
    assert_eq!(formater_duree(Some(TimeDelta::minutes(125))), "2 h 05");
    assert_eq!(formater_duree(Some(TimeDelta::days(3))), "3 jours");
    assert_eq!(formater_duree(None), "?");
}

/// Le monde vu par l'orchestrateur : joueur en ligne, coffre présent, preuve sur soi.
struct FauxCoffre {
    en_ligne: Cell<bool>,
    present: Cell<bool>,
    vide: Cell<bool>,
    preuve: Cell<bool>,
    probleme: RefCell<Option<String>>,
    actualisations: Cell<usize>,
    verrou: Mutex<()>,
    actions: RefCell<Vec<String>>,
}

impl FauxCoffre {
    fn nouveau() -> Rc<Self> {
        Rc::new(Self {
            en_ligne: Cell::new(false),
            present: Cell::new(true),
            vide: Cell::new(true),
            preuve: Cell::new(false),
            probleme: RefCell::new(None),
            actualisations: Cell::new(0),
            verrou: Mutex::new(()),
            actions: RefCell::new(Vec::new()),
        })
    }

    fn note(&self, action: String) {
        self.actions.borrow_mut().push(action);
    }

    fn actions(&self) -> Vec<String> {
        self.actions.borrow().clone()
    }

    fn compte(&self, action: &str) -> usize {
        self.actions
            .borrow()
            .iter()
            .filter(|a| *a == action)
            .count()
    }
}

#[async_trait(?Send)]
impl Coffre for FauxCoffre {
    fn verrou(&self) -> &Mutex<()> {
        &self.verrou
    }

    fn atelier(&self) -> Option<Atelier> {
        if self.probleme.borrow().is_some() {
            None
        } else {
            Some(A)
        }
    }

    fn verifier_atelier(&self) -> Result<Atelier, ErreurCoffre> {
        match self.probleme.borrow().as_ref() {
            Some(probleme) => Err(ErreurCoffre::AtelierAbsent(probleme.clone())),
            None => Ok(A),
        }
    }

    async fn joueur_en_ligne(&self) -> Result<bool, ErreurCoffre> {
        Ok(self.en_ligne.get())
    }

    async fn actualiser_atelier(&self) -> Result<(), ErreurCoffre> {
        self.actualisations.set(self.actualisations.get() + 1);
        Ok(())
    }

    async fn poser(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        self.verifier_atelier()?;
        self.note(format!("poser {}", s.id));
        self.present.set(true);
        self.vide.set(false);
        self.preuve.set(false);
        Ok(())
    }

    async fn annoncer(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        self.note(format!("annoncer {}", s.id));
        Ok(())
    }

    async fn equiper(&self) -> Result<(), ErreurCoffre> {
        self.note("équiper".to_owned());
        Ok(())
    }

    async fn rappeler(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        self.note(format!("rappel {}", s.id));
        Ok(())
    }

    async fn demander_de_vider(&self) -> Result<(), ErreurCoffre> {
        self.note("vider ?".to_owned());
        Ok(())
    }

    async fn preuve_trouvee(&self, _: &ExerciceStocke) -> Result<bool, ErreurCoffre> {
        Ok(self.preuve.get())
    }

    async fn coffre_present(&self) -> Result<bool, ErreurCoffre> {
        Ok(self.present.get())
    }

    async fn coffre_vide(&self) -> Result<bool, ErreurCoffre> {
        Ok(self.vide.get())
    }

    async fn ranger(&self, s: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        self.note(format!("ranger {}", s.id));
        Ok(())
    }
}

#[derive(Default)]
struct Notifs {
    messages: RefCell<Vec<String>>,
}

#[async_trait(?Send)]
impl Notificateur for Notifs {
    async fn notifier(&self, message: &str) {
        self.messages.borrow_mut().push(message.to_owned());
    }
}

impl Notifs {
    fn messages(&self) -> Vec<String> {
        self.messages.borrow().clone()
    }

    fn dernier(&self) -> String {
        self.messages.borrow().last().cloned().unwrap_or_default()
    }
}

type Horloge = Rc<Cell<DateTime<Utc>>>;

fn avancer(horloge: &Horloge, duree: TimeDelta) {
    horloge.set(horloge.get() + duree);
}

struct Monde {
    _dossier: tempfile::TempDir,
    depot: Rc<Depot>,
    coffre: Rc<FauxCoffre>,
    notifs: Rc<Notifs>,
    horloge: Horloge,
    orch: Orchestrateur,
}

const DEMANDE: Demande<'static> = Demande {
    source: Source::Expression,
    demande_par: "cli",
    recompense: "minecraft:diamond",
    recompense_nombre: 3,
};

fn monde() -> Monde {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Rc::new(Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap());
    let (coffre, notifs) = (FauxCoffre::nouveau(), Rc::new(Notifs::default()));
    let horloge: Horloge = Rc::new(Cell::new(t0()));
    let lue = Rc::clone(&horloge);
    let orch = Orchestrateur::new(
        depot.clone(),
        coffre.clone(),
        notifs.clone(),
        Box::new(move || lue.get()),
        None,
    );
    for n in [1, 2] {
        let exercice = Exercice {
            competence: "maths/division-posee".to_owned(),
            enonce: format!("{} ÷ 12 = ?", n * 12),
            reponse: n.to_string(),
            donnees: Default::default(),
        };
        depot.ajouter(&exercice, DEMANDE, t0()).unwrap();
    }
    Monde {
        _dossier: dossier,
        depot,
        coffre,
        notifs,
        horloge,
        orch,
    }
}

fn actif(depot: &Depot) -> Option<i64> {
    depot.actif().unwrap().map(|s| s.id)
}

#[tokio::test]
async fn scenario_complet() {
    let m = monde();

    // Enfant absent : le coffre est posé quand même, sans annonce ni niveaux.
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.actions(), ["poser 1"]);
    assert_eq!(
        m.notifs.messages(),
        ["Coffre n°1 posé : 12 ÷ 12 = ? (réponse 1)."]
    );
    assert_eq!(actif(&m.depot), Some(1));

    // Il se connecte : clé et niveaux une seule fois, puis rappel.
    m.coffre.en_ligne.set(true);
    avancer(&m.horloge, TimeDelta::minutes(1));
    m.orch.tour().await.unwrap();
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.actions()[1..], ["équiper", "rappel 1"]);

    // Il ouvre : coffre rangé, parents prévenus avec la durée, pas encore de suivant.
    m.coffre.preuve.set(true);
    avancer(&m.horloge, TimeDelta::minutes(4));
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.actions().last().unwrap(), "ranger 1");
    assert_eq!(
        m.notifs.dernier(),
        "Coffre n°1 ouvert en 5 min, sans indice (12 ÷ 12 = ? → 1)."
    );
    assert_eq!(m.depot.lire(1).unwrap().unwrap().statut, Statut::Ouvert);

    // Coffre pas vidé : on attend, et on le lui demande une fois.
    avancer(&m.horloge, TimeDelta::seconds(11));
    m.orch.tour().await.unwrap();
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.actions().last().unwrap(), "vider ?");
    assert_eq!(m.coffre.compte("vider ?"), 1);
    assert_eq!(actif(&m.depot), None);

    // Vidé : le second coffre apparaît, clé et niveaux donnés avant l'annonce.
    m.coffre.vide.set(true);
    m.orch.tour().await.unwrap();
    let actions = m.coffre.actions();
    assert_eq!(
        actions[actions.len() - 3..],
        ["poser 2", "équiper", "annoncer 2"]
    );
    let second = m.depot.actif().unwrap().unwrap();
    assert_eq!(second.id, 2);
    assert!(second.niveaux_donnes);
}

#[tokio::test]
async fn suivant_attend_dix_secondes() {
    let m = monde();
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    m.coffre.preuve.set(true);
    m.orch.tour().await.unwrap();
    m.coffre.vide.set(true);
    avancer(&m.horloge, TimeDelta::seconds(5));
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), None);
    avancer(&m.horloge, TimeDelta::seconds(6));
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(2));
}

#[tokio::test]
async fn coffre_disparu_repose_et_note() {
    let m = monde();
    m.orch.tour().await.unwrap();
    m.coffre.present.set(false);
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.actions().last().unwrap(), "poser 1");
    assert_eq!(
        m.notifs.dernier(),
        "Coffre n°1 disparu sans être ouvert : il a été reposé."
    );
    let evenements = m
        .depot
        .evenements_entre(t0() - TimeDelta::days(1), t0() + TimeDelta::days(1))
        .unwrap();
    assert_eq!(evenements.len(), 1);
    assert_eq!(
        (evenements[0].exercice_id, evenements[0].genre.as_str()),
        (Some(1), "coffre_disparu")
    );
    assert_eq!(actif(&m.depot), Some(1));
}

#[tokio::test]
async fn preuve_ignoree_hors_ligne() {
    let m = monde();
    m.orch.tour().await.unwrap();
    m.coffre.preuve.set(true); // impossible en vrai, mais l'orchestrateur ne doit pas interroger
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(1));
}

const PAS_CONSTRUIT: &str = "l'atelier n'est pas construit en 0 -60 8 dans ce monde.";
const AILLEURS: &str = "l'atelier de ce monde est en 120 64 -35, mais l'agent attend 0 -60 8.";

/// Serveur neuf : `mine-de-rien atelier` n'a jamais été lancé, rien n'est enregistré.
#[derive(Default)]
struct MondeSansAtelier {
    pos: RefCell<Option<[i32; 3]>>,
    recues: RefCell<Vec<String>>,
}

#[async_trait(?Send)]
impl AdaptateurJeu for MondeSansAtelier {
    async fn commande(&self, c: &str) -> Result<String, ErreurRcon> {
        self.recues.borrow_mut().push(c.to_owned());
        if c == "data get storage mine_de_rien:atelier pos" {
            return Ok(match *self.pos.borrow() {
                None => "Found no elements matching pos".to_owned(),
                Some([x, y, z]) => format!(
                    "Storage mine_de_rien:atelier has the following contents: [{x}, {y}, {z}]"
                ),
            });
        }
        Ok(String::new())
    }

    async fn dire(&self, _: &str, _: &str, _: &[(&str, Style<'_>)]) -> Result<(), ErreurRcon> {
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat ici".to_owned()))
    }
}

#[tokio::test]
async fn pose_refusee_sans_atelier_dans_le_monde() {
    let monde = Rc::new(MondeSansAtelier::default());
    let coffre = CoffreDuSoir::new(monde.clone(), "Steve", 3, None).unwrap();
    coffre.actualiser_atelier().await.unwrap();
    let erreur = coffre.poser(&defaut()).await.unwrap_err();
    assert!(matches!(&erreur, ErreurCoffre::AtelierAbsent(m) if m.contains("pas construit")));
    assert_eq!(
        *monde.recues.borrow(),
        ["data get storage mine_de_rien:atelier pos"]
    );
}

/// Atelier construit ou déplacé pendant que l'agent tourne : pas de redémarrage.
#[tokio::test]
async fn la_position_suit_le_monde_sans_recreer_le_coffre() {
    let monde = Rc::new(MondeSansAtelier::default());
    *monde.pos.borrow_mut() = Some([120, 64, -35]);
    let coffre = CoffreDuSoir::new(monde.clone(), "Steve", 3, None).unwrap();
    coffre.actualiser_atelier().await.unwrap();
    assert_eq!(
        coffre.verifier_atelier().unwrap(),
        Atelier::new(120, 64, -35)
    );
    *monde.pos.borrow_mut() = Some([-27, 68, -16]);
    coffre.actualiser_atelier().await.unwrap();
    assert_eq!(
        coffre.verifier_atelier().unwrap(),
        Atelier::new(-27, 68, -16)
    );
    let commandes = commandes_pose(&coffre.atelier().unwrap(), &defaut()).unwrap();
    assert!(commandes.iter().any(|c| c.contains("-27 68 -16")));
    *monde.pos.borrow_mut() = None;
    coffre.actualiser_atelier().await.unwrap();
    assert!(matches!(
        coffre.verifier_atelier(),
        Err(ErreurCoffre::AtelierAbsent(_))
    ));
}

fn probleme(m: &Monde, raison: Option<&str>) {
    *m.coffre.probleme.borrow_mut() = raison.map(str::to_owned);
}

#[tokio::test]
async fn sans_atelier_parents_prevenus_une_fois_puis_pose_quand_il_existe() {
    let m = monde();
    probleme(&m, Some(PAS_CONSTRUIT));
    m.orch.tour().await.unwrap();
    m.orch.tour().await.unwrap();
    assert!(m.coffre.actions().is_empty() && actif(&m.depot).is_none());
    assert_eq!(
        m.notifs.messages(),
        [format!("Aucun coffre ne peut être posé : {PAS_CONSTRUIT}")]
    );
    probleme(&m, None);
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.actions(), ["poser 1"]);
    assert_eq!(actif(&m.depot), Some(1));
}

/// Le cas de l'essai du 2026-11-09 : nouveau serveur, coffre actif, atelier jamais construit.
#[tokio::test]
async fn coffre_actif_d_un_autre_monde_jamais_repose_dans_la_roche() {
    let m = monde();
    m.orch.tour().await.unwrap();
    m.coffre.present.set(false);
    probleme(&m, Some(PAS_CONSTRUIT));
    m.orch.tour().await.unwrap();
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.actions(), ["poser 1"]);
    assert!(
        m.notifs
            .dernier()
            .starts_with("Aucun coffre ne peut être posé")
    );
    let signalements = m
        .notifs
        .messages()
        .iter()
        .filter(|msg| msg.starts_with("Aucun coffre"))
        .count();
    assert_eq!(signalements, 1);
}

/// L'essai du 2026-11-09 : un coffre emmuré, plein, ne doit pas retarder le message.
#[tokio::test]
async fn coffre_plein_d_un_autre_monde_ne_fait_pas_attendre() {
    let m = monde();
    m.coffre.en_ligne.set(true);
    m.coffre.vide.set(false);
    probleme(&m, Some(PAS_CONSTRUIT));
    m.orch.tour().await.unwrap();
    assert!(m.coffre.actions().is_empty()); // pas de « vide le coffre » pour un coffre inaccessible
    assert_eq!(
        m.notifs.messages(),
        [format!("Aucun coffre ne peut être posé : {PAS_CONSTRUIT}")]
    );
}

#[tokio::test]
async fn coffre_actif_present_mais_atelier_absent_parents_prevenus_sans_rappel() {
    let m = monde();
    m.orch.tour().await.unwrap();
    m.coffre.en_ligne.set(true);
    probleme(&m, Some(PAS_CONSTRUIT));
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.compte("rappel 1"), 0);
    assert_eq!(
        m.notifs.dernier(),
        format!("Aucun coffre ne peut être posé : {PAS_CONSTRUIT}")
    );
}

#[tokio::test]
async fn chaque_tour_relit_l_atelier() {
    let m = monde();
    m.orch.tour().await.unwrap();
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.actualisations.get(), 2);
}

/// Le premier message ne doit pas rendre l'agent muet sur un problème différent.
#[tokio::test]
async fn un_nouveau_probleme_d_atelier_est_signale_meme_apres_le_premier() {
    let m = monde();
    probleme(&m, Some(PAS_CONSTRUIT));
    m.orch.tour().await.unwrap();
    probleme(&m, Some(AILLEURS));
    m.orch.tour().await.unwrap();
    m.orch.tour().await.unwrap();
    assert_eq!(
        m.notifs.messages(),
        [
            format!("Aucun coffre ne peut être posé : {PAS_CONSTRUIT}"),
            format!("Aucun coffre ne peut être posé : {AILLEURS}"),
        ]
    );
}

const PSEUDO: &str = "Zorglub42";

fn atelier_de(cas: &Value) -> Atelier {
    let c = &cas["entree"]["atelier"];
    let n = |i: usize| c[i].as_i64().unwrap() as i32;
    Atelier::new(n(0), n(1), n(2))
}

async fn appeler(
    coffre: &CoffreDuSoir,
    methode: &str,
    s: &ExerciceStocke,
) -> Result<Value, ErreurCoffre> {
    if methode != "actualiser_atelier" {
        coffre.actualiser_atelier().await?;
    }
    let resultat = match methode {
        "joueur_en_ligne" => json!(coffre.joueur_en_ligne().await?),
        "actualiser_atelier" => json!(coffre.actualiser_atelier().await?),
        "poser" => json!(coffre.poser(s).await?),
        "annoncer" => json!(coffre.annoncer(s).await?),
        "equiper" => json!(coffre.equiper().await?),
        "rappeler" => json!(coffre.rappeler(s).await?),
        "demander_de_vider" => json!(coffre.demander_de_vider().await?),
        "preuve_trouvee" => json!(coffre.preuve_trouvee(s).await?),
        "coffre_present" => json!(coffre.coffre_present().await?),
        "coffre_vide" => json!(coffre.coffre_vide().await?),
        "ranger" => json!(coffre.ranger(s).await?),
        "retirer" => json!(coffre.retirer().await?),
        "a_une_cle" => json!(coffre.a_une_cle().await?),
        "donner_cle" => json!(coffre.donner_cle().await?),
        autre => panic!("méthode inconnue : {autre}"),
    };
    let atelier = coffre.atelier().map(|a| [a.x, a.y, a.z]);
    Ok(json!({"resultat": resultat, "atelier": atelier}))
}

async fn jouer_scenario(cas: &Value, exercices: &[Exercice], rejeu: Rc<commun::Rejeu>) -> Value {
    let e = &cas["entree"];
    let dossier = tempfile::tempdir().unwrap();
    let depot = Rc::new(Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap());
    for indice in e["exercices"].as_array().unwrap() {
        depot
            .ajouter(&exercices[indice.as_u64().unwrap() as usize], DEMANDE, t0())
            .unwrap();
    }
    let niveaux = e["niveaux"].as_i64().unwrap();
    let coffre = Rc::new(
        CoffreDuSoir::new(rejeu.clone(), PSEUDO, niveaux, e["assistant"].as_str()).unwrap(),
    );
    let mode = e["paisible"]
        .as_bool()
        .unwrap()
        .then(|| ModePaisible::new(rejeu.clone(), PSEUDO, "easy").unwrap());
    let notifs = Rc::new(Notifs::default());
    let horloge: Horloge = Rc::new(Cell::new(t0()));
    let lue = Rc::clone(&horloge);
    let orch = Orchestrateur::new(
        depot.clone(),
        coffre,
        notifs.clone(),
        Box::new(move || lue.get()),
        mode,
    );
    for etape in e["etapes"].as_array().unwrap() {
        avancer(
            &horloge,
            TimeDelta::seconds(etape["avance"].as_i64().unwrap_or(0)),
        );
        for _ in 0..etape["indices"].as_u64().unwrap_or(0) {
            let actif = depot.actif().unwrap().unwrap();
            depot
                .noter("indice", Some(actif.id), horloge.get())
                .unwrap();
        }
        if let Err(erreur) = orch.tour().await {
            notifs
                .messages
                .borrow_mut()
                .push(format!("ERREUR {erreur}"));
        }
    }
    let (debut, fin) = (t0() - TimeDelta::days(1), t0() + TimeDelta::days(1));
    let coffres: Vec<Value> = depot
        .entre(debut, fin)
        .unwrap()
        .iter()
        .map(|s| {
            json!([
                s.id,
                s.statut.nom(),
                s.niveaux_donnes,
                formater_duree(s.duree())
            ])
        })
        .collect();
    let evenements: Vec<Value> = depot
        .evenements_entre(debut, fin)
        .unwrap()
        .iter()
        .map(|ev| json!([ev.exercice_id, ev.genre]))
        .collect();
    json!({"notifications": notifs.messages(), "coffres": coffres, "evenements": evenements})
}

/// Commandes, méthodes et onze scénarios de surveillance complets : mêmes commandes dans le
/// même ordre que la version Python, mêmes messages à l'enfant, mêmes notifications aux
/// parents, même état de la base à la fin.
#[tokio::test]
async fn oracle_python() {
    let tous = commun::oracle("coffre_du_soir");
    let exercices: Vec<Exercice> = serde_json::from_value(
        tous.iter()
            .find(|c| c["entree"]["f"] == "exercices")
            .unwrap()["sortie"]
            .clone(),
    )
    .unwrap();
    let exercice = |cas: &Value| {
        stocke_de(
            exercices[cas["entree"]["exercice"].as_u64().unwrap() as usize].clone(),
            12,
        )
    };
    for cas in &tous {
        let e = &cas["entree"];
        let rejeu = Rc::new(commun::Rejeu::new(cas));
        match e["f"].as_str().unwrap() {
            "exercices" => {}
            "commandes_pose" => commun::conclure(
                cas,
                &rejeu,
                commandes_pose(&atelier_de(cas), &exercice(cas)),
            ),
            "commandes_retrait" => {
                commun::conclure::<_, String>(cas, &rejeu, Ok(commandes_retrait(&atelier_de(cas))))
            }
            "commandes_rangement" => commun::conclure(
                cas,
                &rejeu,
                commandes_rangement(&atelier_de(cas), e["pseudo"].as_str().unwrap(), 12),
            ),
            "commande_cle" => {
                commun::conclure(cas, &rejeu, commande_cle(e["pseudo"].as_str().unwrap()))
            }
            "predicats" => commun::conclure::<_, String>(
                cas,
                &rejeu,
                Ok([predicat_cle(), predicat_preuve(12)]),
            ),
            "methode" => {
                let coffre =
                    CoffreDuSoir::new(rejeu.clone(), PSEUDO, 3, e["assistant"].as_str()).unwrap();
                let s = stocke_de(exercices[3].clone(), 12);
                let resultat = appeler(&coffre, e["methode"].as_str().unwrap(), &s).await;
                commun::conclure(cas, &rejeu, resultat);
            }
            "scenario" => {
                let sortie = jouer_scenario(cas, &exercices, rejeu.clone()).await;
                commun::conclure::<_, String>(cas, &rejeu, Ok(sortie));
            }
            autre => panic!("fonction inconnue : {autre}"),
        }
    }
}
