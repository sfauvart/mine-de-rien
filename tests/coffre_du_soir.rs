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
use mine_de_rien::games::tableau::{Genre, Morceau};
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
    /// Le nom que porte la clé de l'enfant en ce moment.
    nom_cle: RefCell<Option<String>>,
    /// Le jeu de l'enfant est relié au chat vocal.
    relie: Cell<bool>,
    /// La voix ne répond pas : les sons d'une dictée ne se préparent pas.
    voix_en_panne: Cell<bool>,
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
            nom_cle: RefCell::new(None),
            relie: Cell::new(true),
            voix_en_panne: Cell::new(false),
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

    async fn chat_vocal_relie(&self) -> Result<bool, ErreurCoffre> {
        Ok(self.relie.get())
    }

    async fn preparer_la_dictee(&self, _: &ExerciceStocke) -> Result<(), ErreurCoffre> {
        if self.voix_en_panne.get() {
            return Err(ErreurCoffre::Coffre(
                "phrase de dictée : La voix locale est injoignable (essai).".to_owned(),
            ));
        }
        Ok(())
    }

    async fn demander_le_chat_vocal(&self) -> Result<(), ErreurCoffre> {
        self.note("chat vocal ?".to_owned());
        Ok(())
    }

    async fn nom_de_la_cle(&self) -> Result<Option<String>, ErreurCoffre> {
        Ok(self.nom_cle.borrow().clone())
    }

    async fn accepter_la_cle(&self, nom: &str) -> Result<(), ErreurCoffre> {
        self.note(format!("clé juste {nom}"));
        Ok(())
    }

    async fn afficher_tableau(&self, morceaux: &[Morceau]) -> Result<(), ErreurCoffre> {
        let texte: String = morceaux
            .iter()
            .map(|m| match m.genre {
                Genre::Texte => m.texte.clone(),
                Genre::TrouActif => "[?]".to_owned(),
                Genre::TrouAttente => "[…]".to_owned(),
                Genre::Trouve => format!("[{}]", m.texte),
            })
            .collect();
        self.note(format!("tableau {texte}"));
        Ok(())
    }

    async fn effacer_tableau(&self) -> Result<(), ErreurCoffre> {
        self.note("tableau effacé".to_owned());
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
        "retirer" => json!(coffre.retirer(None).await?),
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

/// Un monde dont la file ne contient qu'un coffre de dictée.
fn monde_de_dictee() -> Monde {
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
    let phrases = mine_de_rien::exercises::francais::dictee::decouper("La plus haute tour.");
    let position = mine_de_rien::exercises::francais::dictee::Position { phrase: 0, mot: 1 };
    let exercice = mine_de_rien::dictees::exercice_sonore(&phrases, position, 1).unwrap();
    let demande = Demande {
        source: Source::Dictee,
        ..DEMANDE
    };
    depot.ajouter(&exercice, demande, t0()).unwrap();
    Monde {
        _dossier: dossier,
        depot,
        coffre,
        notifs,
        horloge,
        orch,
    }
}

fn calcul(depot: &Depot) -> i64 {
    let exercice = Exercice {
        competence: "maths/division-posee".to_owned(),
        enonce: "24 ÷ 12 = ?".to_owned(),
        reponse: "2".to_owned(),
        donnees: Default::default(),
    };
    depot.ajouter(&exercice, DEMANDE, t0()).unwrap()
}

const SANS_CHAT_VOCAL: &str = "Coffre n°1 : le jeu de l'enfant n'est pas relié au chat vocal, il \
    n'entendrait pas la dictée (mod Simple Voice Chat, voir docs/rejoindre-le-serveur.md). Les \
    coffres de dictée attendent, les autres continuent.";

#[tokio::test]
async fn dictee_posee_et_annoncee_quand_le_jeu_est_relie() {
    let m = monde_de_dictee();
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(1));
    assert_eq!(m.coffre.compte("annoncer 1"), 1);
    assert_eq!(m.coffre.compte("chat vocal ?"), 0);
    assert_eq!(m.notifs.messages().len(), 1, "{:?}", m.notifs.messages());
}

#[tokio::test]
async fn dictee_attend_le_chat_vocal_et_les_parents_sont_prevenus_une_fois_par_jour() {
    let m = monde_de_dictee();
    m.coffre.en_ligne.set(true);
    m.coffre.relie.set(false);
    // Il vient de se connecter : son jeu est peut-être encore en train de se relier.
    m.orch.tour().await.unwrap();
    avancer(&m.horloge, TimeDelta::seconds(15));
    m.orch.tour().await.unwrap();
    assert!(m.coffre.actions().is_empty(), "{:?}", m.coffre.actions());
    assert!(m.notifs.messages().is_empty());
    assert_eq!(actif(&m.depot), None);

    // Vingt secondes plus tard, toujours rien : il n'a pas le chat vocal.
    avancer(&m.horloge, TimeDelta::seconds(15));
    for _ in 0..3 {
        m.orch.tour().await.unwrap();
    }
    assert_eq!(m.coffre.actions(), ["chat vocal ?"]);
    assert_eq!(m.notifs.messages(), [SANS_CHAT_VOCAL]);
    assert_eq!(actif(&m.depot), None);

    // Il part : la dictée attend son retour, sans être posée.
    m.coffre.en_ligne.set(false);
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), None);
    // Une reconnexion le même jour, sans le mod : rien de plus.
    avancer(&m.horloge, TimeDelta::hours(1));
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    avancer(&m.horloge, TimeDelta::seconds(30));
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.compte("chat vocal ?"), 1);
    assert_eq!(m.notifs.messages().len(), 1, "{:?}", m.notifs.messages());
    assert_eq!(actif(&m.depot), None);

    // Le lendemain, toujours sans le mod : un nouveau signalement.
    avancer(&m.horloge, TimeDelta::days(1));
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.compte("chat vocal ?"), 2);

    // Le mod est installé : le coffre est posé et annoncé.
    m.coffre.relie.set(true);
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(1));
    assert_eq!(m.coffre.compte("annoncer 1"), 1);
}

#[tokio::test]
async fn sans_chat_vocal_le_coffre_de_calcul_passe_devant_la_dictee() {
    let m = monde_de_dictee();
    let maths = calcul(&m.depot);
    m.coffre.en_ligne.set(true);
    m.coffre.relie.set(false);
    m.orch.tour().await.unwrap();
    // Pendant le délai de grâce, personne ne double : la dictée garde sa place.
    assert_eq!(actif(&m.depot), None);
    avancer(&m.horloge, TimeDelta::seconds(30));
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(maths));
    assert_eq!(m.coffre.compte(&format!("annoncer {maths}")), 1);
    assert_eq!(m.coffre.compte("chat vocal ?"), 1);
    assert_eq!(m.coffre.compte("poser 1"), 0);
    assert!(m.notifs.messages().contains(&SANS_CHAT_VOCAL.to_owned()));
}

/// Un coffre de dictée posé pendant une première partie, puis l'enfant s'en va.
async fn dictee_posee_puis_depart() -> Monde {
    let m = monde_de_dictee();
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(1));
    m.coffre.en_ligne.set(false);
    m.orch.tour().await.unwrap();
    m
}

#[tokio::test]
async fn dictee_absente_n_est_pas_posee_et_attend_le_retour() {
    let m = monde_de_dictee();
    let maths = calcul(&m.depot);
    for _ in 0..3 {
        m.orch.tour().await.unwrap();
    }
    // Ni la dictée, ni le calcul qui la suit : la file garde son ordre.
    assert_eq!(actif(&m.depot), None);
    assert!(m.coffre.actions().is_empty());
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(1));
    assert_eq!(m.coffre.compte(&format!("poser {maths}")), 0);
}

#[tokio::test]
async fn au_retour_le_rappel_d_une_dictee_attend_le_chat_vocal() {
    let m = dictee_posee_puis_depart().await;
    // Il revient : son jeu n'est pas encore relié, la phrase ne s'entendrait pas.
    m.coffre.en_ligne.set(true);
    m.coffre.relie.set(false);
    avancer(&m.horloge, TimeDelta::minutes(5));
    m.orch.tour().await.unwrap();
    assert_eq!(m.coffre.compte("rappel 1"), 0);
    // Trois secondes plus tard, il l'est : le rappel part, une seule fois.
    avancer(&m.horloge, TimeDelta::seconds(3));
    m.coffre.relie.set(true);
    for _ in 0..3 {
        m.orch.tour().await.unwrap();
    }
    assert_eq!(m.coffre.compte("rappel 1"), 1);
    assert_eq!(m.coffre.compte("chat vocal ?"), 0);
}

#[tokio::test]
async fn au_retour_sans_chat_vocal_le_rappel_part_quand_meme() {
    let m = dictee_posee_puis_depart().await;
    m.coffre.en_ligne.set(true);
    m.coffre.relie.set(false);
    avancer(&m.horloge, TimeDelta::minutes(5));
    m.orch.tour().await.unwrap();
    avancer(&m.horloge, TimeDelta::seconds(30));
    for _ in 0..3 {
        m.orch.tour().await.unwrap();
    }
    // La phrase à trou est écrite, l'enfant sait pourquoi il n'entend rien, les parents aussi.
    assert_eq!(m.coffre.compte("rappel 1"), 1);
    assert_eq!(m.coffre.compte("chat vocal ?"), 1);
    assert_eq!(m.notifs.dernier(), SANS_CHAT_VOCAL);
}

#[tokio::test]
async fn voix_en_panne_la_dictee_attend_et_le_calcul_continue() {
    let m = monde_de_dictee();
    let maths = calcul(&m.depot);
    m.coffre.en_ligne.set(true);
    m.coffre.voix_en_panne.set(true);
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(maths));
    assert_eq!(m.coffre.compte("poser 1"), 0);
    // L'enfant n'y peut rien : rien ne lui est demandé. Les parents sont prévenus une fois.
    assert_eq!(m.coffre.compte("chat vocal ?"), 0);
    let pannes = |m: &Monde| {
        m.notifs
            .messages()
            .iter()
            .filter(|n| n.contains("ne peut pas être lue"))
            .count()
    };
    assert_eq!(pannes(&m), 1, "{:?}", m.notifs.messages());
    m.coffre.preuve.set(true);
    m.orch.tour().await.unwrap();
    m.coffre.vide.set(true);
    avancer(&m.horloge, TimeDelta::minutes(1));
    for _ in 0..3 {
        m.orch.tour().await.unwrap();
    }
    assert_eq!(pannes(&m), 1);
    assert_eq!(m.coffre.compte("poser 1"), 0);
    // La voix revient : la dictée est posée.
    m.coffre.voix_en_panne.set(false);
    m.coffre.preuve.set(false);
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(1));
}

#[tokio::test]
async fn un_coffre_de_maths_n_attend_jamais_le_chat_vocal() {
    let m = monde_de_dictee();
    m.depot.annuler(1).unwrap();
    let ident = calcul(&m.depot);
    m.coffre.en_ligne.set(true);
    m.coffre.relie.set(false);
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(ident));
    assert_eq!(m.coffre.compte("chat vocal ?"), 0);
}

#[test]
fn nom_lu_sur_la_cle() {
    use mine_de_rien::games::coffre_du_soir::{commande_nom_cle, lire_nom_cle};
    assert_eq!(
        commande_nom_cle("Steve").unwrap(),
        "data get entity Steve Inventory[{components:{\"minecraft:custom_data\":{mdr_cle:1b}}}].components.\"minecraft:custom_name\""
    );
    assert!(commande_nom_cle("@a").is_err());
    let lu = |donnee: &str| lire_nom_cle(&format!("Steve has the following entity data: {donnee}"));
    assert_eq!(lu("\"29\"").as_deref(), Some("29"));
    assert_eq!(lu("\"Dubaï\"").as_deref(), Some("Dubaï"));
    assert_eq!(
        lu("\"s'élève \\\"x\\\"\"").as_deref(),
        Some("s'élève \"x\"")
    );
    assert_eq!(lu("'il dit \"oui\"'").as_deref(), Some("il dit \"oui\""));
    assert_eq!(lu("\"a\\\\b\"").as_deref(), Some("a\\b"));
    // Rien à lire : clé sans nom, absente, en double, enfant déconnecté, réponse tronquée.
    for reponse in [
        "Found no elements matching Inventory[{components:{\"minecraft:custom_data\":{mdr_cle:1b}}}]",
        "No entity was found",
        "This argument accepts a single NBT value",
        "",
        "Steve has the following entity data: \"\"",
        "Steve has the following entity data: \"pas fini",
        "Steve has the following entity data: {text: \"29\"}",
    ] {
        assert_eq!(lire_nom_cle(reponse), None, "{reponse}");
    }
}

fn cle(m: &Monde, nom: Option<&str>) {
    *m.coffre.nom_cle.borrow_mut() = nom.map(str::to_owned);
}

fn essais(m: &Monde, ident: i64) -> Vec<(String, bool)> {
    m.depot
        .tentatives(ident)
        .unwrap()
        .into_iter()
        .map(|t| (t.proposition, t.correcte))
        .collect()
}

#[tokio::test]
async fn chaque_nouveau_nom_de_la_cle_est_un_essai() {
    let m = monde(); // le coffre n°1 attend « 1 », le n°2 « 2 »
    m.coffre.en_ligne.set(true);
    // La clé porte encore la réponse d'un vieux coffre : ce n'est pas un essai.
    cle(&m, Some("29"));
    m.orch.tour().await.unwrap(); // le coffre n°1 est posé, « 29 » est le point de départ
    m.orch.tour().await.unwrap();
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(1));
    assert!(essais(&m, 1).is_empty());

    // Il écrit « 7 », le laisse deux tours, pose la clé sur l'enclume, puis écrit « 1 ».
    cle(&m, Some("7"));
    m.orch.tour().await.unwrap();
    m.orch.tour().await.unwrap();
    cle(&m, None);
    m.orch.tour().await.unwrap();
    cle(&m, Some("7"));
    m.orch.tour().await.unwrap();
    assert_eq!(essais(&m, 1), [("7".to_owned(), false)]);
    cle(&m, Some("1"));
    m.coffre.preuve.set(true);
    avancer(&m.horloge, TimeDelta::minutes(4));
    m.orch.tour().await.unwrap();
    assert_eq!(
        essais(&m, 1),
        [("7".to_owned(), false), ("1".to_owned(), true)]
    );
    assert_eq!(
        m.notifs.dernier(),
        "Coffre n°1 ouvert en 4 min, sans indice (12 ÷ 12 = ? → 1). Essais : 7."
    );

    // Coffre suivant : « 1 », resté sur la clé, est son point de départ.
    m.coffre.preuve.set(false);
    m.coffre.vide.set(true);
    avancer(&m.horloge, TimeDelta::seconds(15));
    m.orch.tour().await.unwrap();
    assert_eq!(actif(&m.depot), Some(2));
    m.orch.tour().await.unwrap();
    assert!(essais(&m, 2).is_empty());
    cle(&m, Some("2"));
    m.coffre.preuve.set(true);
    avancer(&m.horloge, TimeDelta::minutes(1));
    m.orch.tour().await.unwrap();
    assert_eq!(essais(&m, 2), [("2".to_owned(), true)]);
    // Sans essai raté, la notification est celle d'avant, au caractère près.
    assert_eq!(
        m.notifs.dernier(),
        "Coffre n°2 ouvert en 1 min, sans indice (24 ÷ 12 = ? → 2)."
    );
}

#[tokio::test]
async fn les_essais_ne_sont_lus_que_si_l_enfant_est_la() {
    let m = monde();
    cle(&m, Some("7"));
    m.orch.tour().await.unwrap();
    m.orch.tour().await.unwrap();
    cle(&m, Some("8"));
    m.orch.tour().await.unwrap();
    assert!(essais(&m, 1).is_empty());
}

#[tokio::test]
async fn un_mot_de_dictee_rate_garde_sa_graphie_et_ne_va_pas_au_journal() {
    mine_de_rien::journal::capturer();
    let m = monde_de_dictee(); // « La plus haute tour. », le mot est « plus »
    m.orch.tour().await.unwrap();
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    for graphie in ["plu", "pluss", "Plus", " pl us", "plus"] {
        cle(&m, Some(graphie));
        m.orch.tour().await.unwrap();
    }
    assert_eq!(
        essais(&m, 1),
        [
            ("plu".to_owned(), false),
            ("pluss".to_owned(), false),
            // Ni la majuscule ni les espaces ne comptent.
            ("Plus".to_owned(), true),
            (" pl us".to_owned(), true),
            ("plus".to_owned(), true),
        ]
    );
    // Le verrou ne connaît qu'un nom exact : il prend chaque fois celui que l'enfant a écrit.
    let acceptees: Vec<String> = m
        .coffre
        .actions()
        .into_iter()
        .filter(|a| a.starts_with("clé juste"))
        .collect();
    assert_eq!(
        acceptees,
        ["clé juste Plus", "clé juste  pl us", "clé juste plus"]
    );
    m.coffre.preuve.set(true);
    m.orch.tour().await.unwrap();
    assert!(
        m.notifs
            .dernier()
            .ends_with("→ plus). Essais : plu, pluss."),
        "{}",
        m.notifs.dernier()
    );
    // Ce qu'il a tapé va aux parents et dans la base, jamais dans le journal de l'agent.
    let journal = mine_de_rien::journal::captures().join("\n");
    assert!(!journal.contains("pluss"), "{journal}");
    // Ni dans le jeu : le faux coffre n'a reçu aucun texte à dire.
    assert!(!m.coffre.actions().iter().any(|a| a.contains("pluss")));
    assert!(!m.coffre.actions().iter().any(|a| a.ends_with("plu")));
}

/// Un monde sans coffre en file, avec une dictée de trois phrases confirmée la veille pour
/// dans cinq jours, et les séances quotidiennes réglées sur le fuseau de Paris.
fn monde_avec_seances() -> Monde {
    use mine_de_rien::storage::sqlite::PhraseSonore;
    let dossier = tempfile::tempdir().unwrap();
    let depot = Rc::new(Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap());
    let (coffre, notifs) = (FauxCoffre::nouveau(), Rc::new(Notifs::default()));
    let horloge: Horloge = Rc::new(Cell::new(t0())); // samedi 7 novembre, 19 h à Paris
    let lue = Rc::clone(&horloge);
    let orch = Orchestrateur::new(
        depot.clone(),
        coffre.clone(),
        notifs.clone(),
        Box::new(move || lue.get()),
        None,
    )
    .avec_seances(
        4,
        chrono_tz::Europe::Paris,
        mine_de_rien::config::Coffre::default(),
        mine_de_rien::alea::Alea::graine(3),
    );
    let texte = "Dans la ville de Dubaï s'élève la plus haute tour. Deux-millions de personnes \
                 la visitent chaque année. Elles rient aux éclats.";
    let phrases: Vec<PhraseSonore> = mine_de_rien::exercises::francais::dictee::decouper(texte)
        .into_iter()
        .map(|p| PhraseSonore {
            texte: p.texte,
            son: b"OggS".to_vec(),
            son_lent: b"OggS".to_vec(),
        })
        .collect();
    let jeudi = chrono::NaiveDate::from_ymd_opt(2026, 11, 12).unwrap();
    depot
        .ajouter_dictee(jeudi, texte, &phrases, "111", t0() - TimeDelta::days(1))
        .unwrap();
    Monde {
        _dossier: dossier,
        depot,
        coffre,
        notifs,
        horloge,
        orch,
    }
}

fn coffres_de_dictee(m: &Monde) -> usize {
    m.depot.coffres_de_dictee(1).unwrap().len()
}

#[tokio::test]
async fn la_premiere_connexion_du_jour_cree_la_seance_de_dictee() {
    let m = monde_avec_seances();
    // Personne : rien.
    m.orch.tour().await.unwrap();
    assert_eq!(coffres_de_dictee(&m), 0);
    assert!(m.notifs.messages().is_empty());

    // Il se connecte samedi : quatre coffres, les parents sont prévenus, le premier est posé.
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    assert_eq!(coffres_de_dictee(&m), 4);
    assert_eq!(
        m.notifs.messages()[0],
        "Séance de dictée du jour : 4 coffre(s), dictée n°1 pour le jeudi 12 novembre."
    );
    assert!(actif(&m.depot).is_some());
    assert!(m.notifs.messages().iter().all(|n| !n.contains("Zorglub")));

    // Les tours suivants, puis une reconnexion le même jour : rien de plus.
    m.orch.tour().await.unwrap();
    m.coffre.en_ligne.set(false);
    m.orch.tour().await.unwrap();
    m.coffre.en_ligne.set(true);
    avancer(&m.horloge, TimeDelta::hours(2));
    m.orch.tour().await.unwrap();
    assert_eq!(coffres_de_dictee(&m), 4);

    // Resté connecté après minuit : pas de séance tant qu'il ne se reconnecte pas.
    avancer(&m.horloge, TimeDelta::hours(12));
    m.orch.tour().await.unwrap();
    assert_eq!(coffres_de_dictee(&m), 4);
    // Dimanche, il revient : quatre autres.
    m.coffre.en_ligne.set(false);
    m.orch.tour().await.unwrap();
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    assert_eq!(coffres_de_dictee(&m), 8);
    let annonces = m
        .notifs
        .messages()
        .iter()
        .filter(|n| n.starts_with("Séance de dictée du jour"))
        .count();
    assert_eq!(annonces, 2);
}

#[tokio::test]
async fn sans_reglage_ou_sans_dictee_aucune_seance() {
    // Les séances ne sont pas réglées : une dictée en base ne crée rien.
    let m = monde_de_dictee();
    m.depot.annuler(1).unwrap();
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    assert!(m.depot.en_file().unwrap().is_empty());

    // Réglées, mais la dictée est annulée : rien non plus, et la surveillance continue.
    let m = monde_avec_seances();
    m.depot.annuler_dictee(1).unwrap();
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    assert_eq!(coffres_de_dictee(&m), 0);
    assert!(m.notifs.messages().is_empty());
}

#[tokio::test]
async fn le_tableau_de_la_dictee_suit_les_coffres() {
    use mine_de_rien::exercises::francais::dictee::decouper;
    use mine_de_rien::storage::sqlite::PhraseSonore;

    let m = monde_de_dictee();
    m.depot.annuler(1).unwrap(); // le coffre de dictée sans dictée enregistrée
    let texte = "La plus haute tour.";
    let phrases: Vec<PhraseSonore> = decouper(texte)
        .into_iter()
        .map(|p| PhraseSonore {
            texte: p.texte,
            son: b"OggS".to_vec(),
            son_lent: b"OggS".to_vec(),
        })
        .collect();
    let jour = chrono::NaiveDate::from_ymd_opt(2026, 11, 13).unwrap();
    let ident = m
        .depot
        .ajouter_dictee(jour, texte, &phrases, "111", t0())
        .unwrap();
    let dictee = m.depot.lire_dictee(ident).unwrap().unwrap();
    let demande = Demande {
        source: Source::Dictee,
        ..DEMANDE
    };
    // Les quatre mots du texte.
    let coffres = mine_de_rien::dictees::seance(
        &m.depot,
        &dictee,
        4,
        &mut mine_de_rien::alea::Alea::graine(1),
        &std::collections::HashMap::new(),
        demande,
        t0(),
    )
    .unwrap();
    assert_eq!(coffres.len(), 4);
    // Puis un coffre de calcul.
    let calcul = Exercice {
        competence: "maths/division-posee".to_owned(),
        enonce: "12 ÷ 12 = ?".to_owned(),
        reponse: "1".to_owned(),
        donnees: Default::default(),
    };
    m.depot.ajouter(&calcul, DEMANDE, t0()).unwrap();

    let tableaux = |m: &Monde| -> Vec<String> {
        m.coffre
            .actions()
            .into_iter()
            .filter(|a| a.starts_with("tableau"))
            .collect()
    };
    m.coffre.en_ligne.set(true);
    m.orch.tour().await.unwrap();
    // Posé : un trou jaune, les trois autres gris.
    let vus = tableaux(&m);
    assert_eq!(vus.len(), 1);
    assert_eq!(vus[0].matches("[?]").count(), 1, "{}", vus[0]);
    assert_eq!(vus[0].matches("[…]").count(), 3, "{}", vus[0]);

    for _ in 0..4 {
        m.coffre.preuve.set(true);
        m.orch.tour().await.unwrap(); // ouvert : le mot s'écrit
        m.coffre.preuve.set(false);
        m.coffre.vide.set(true);
        avancer(&m.horloge, TimeDelta::seconds(15));
        m.orch.tour().await.unwrap(); // le suivant est posé
    }
    let vus = tableaux(&m);
    // Quatre poses et quatre ouvertures, puis le coffre de calcul efface le tableau.
    assert_eq!(vus.len(), 9, "{vus:?}");
    assert_eq!(vus[7], "tableau [La] [plus] [haute] [tour].");
    assert_eq!(vus[8], "tableau effacé");
    // Le coffre de calcul suivant ne l'efface pas une seconde fois.
    assert_eq!(m.coffre.compte("tableau effacé"), 1);
}
