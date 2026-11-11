//! Pixel dans le chat, avec un faux jeu, un faux LLM et une vraie base jetable.

mod commun;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use async_trait::async_trait;
use chrono::{DateTime, SecondsFormat, TimeDelta, Utc};
use commun::{FausseDiffusion, VoixDeTest};
use mine_de_rien::adapters::game::base::{AdaptateurJeu, MessageChat};
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::adapters::llm::base::{ErreurLlm, Llm};
use mine_de_rien::dictees::{Lecteur, fichier_du_son};
use mine_de_rien::exercises::base::Exercice;
use mine_de_rien::exercises::indices::indices;
use mine_de_rien::games::atelier::Atelier;
use mine_de_rien::games::coffre_du_soir::{Coffre, CoffreDuSoir};
use mine_de_rien::games::dictee;
use mine_de_rien::games::pixel::{
    EVENEMENT_INDICE, Intention, Pixel, SYSTEME_BAVARDAGE, SYSTEME_CLASSEMENT,
    SYSTEME_REFORMULATION, adresse, classer_par_mots_cles, commande_teleportation,
    normaliser_espaces, propre, verifier_nom,
};
use mine_de_rien::minecraft::texte::Style;
use mine_de_rien::saisie::Source;
use mine_de_rien::storage::sqlite::{Demande, Depot, ExerciceStocke, Statut};
use mine_de_rien::{journal, photo};
use serde_json::{Value, json};

const PSEUDO: &str = "Steve_42";
const ATELIER: Atelier = Atelier {
    x: 10,
    y: 64,
    z: -5,
};

fn debut() -> DateTime<Utc> {
    commun::instant("2026-11-08T18:00:00+00:00")
}

struct FauxJeu {
    commandes: RefCell<Vec<String>>,
    dits: RefCell<Vec<String>>,
    reponse_tp: &'static str,
    atelier: Option<(i32, i32, i32)>,
}

impl FauxJeu {
    fn nouveau() -> Rc<Self> {
        Self::avec(
            "Teleported Steve_42 to 10.5, 64.0, -2.5",
            Some((10, 64, -5)),
        )
    }

    fn avec(reponse_tp: &'static str, atelier: Option<(i32, i32, i32)>) -> Rc<Self> {
        Rc::new(Self {
            commandes: RefCell::new(Vec::new()),
            dits: RefCell::new(Vec::new()),
            reponse_tp,
            atelier,
        })
    }

    fn dits(&self) -> Vec<String> {
        self.dits.borrow().clone()
    }

    fn dernier(&self) -> String {
        self.dits.borrow().last().cloned().unwrap_or_default()
    }

    fn commandes(&self) -> Vec<String> {
        self.commandes.borrow().clone()
    }
}

#[async_trait(?Send)]
impl AdaptateurJeu for FauxJeu {
    async fn commande(&self, commande: &str) -> Result<String, ErreurRcon> {
        if commande == "data get storage mine_de_rien:atelier pos" {
            return Ok(match self.atelier {
                None => "Found no elements matching pos".to_owned(),
                Some((x, y, z)) => format!(
                    "Storage mine_de_rien:atelier has the following contents: [{x}, {y}, {z}]"
                ),
            });
        }
        self.commandes.borrow_mut().push(commande.to_owned());
        Ok(if commande.contains(" tp ") {
            self.reponse_tp.to_owned()
        } else {
            String::new()
        })
    }

    async fn dire(
        &self,
        joueur: &str,
        texte: &str,
        _: &[(&str, Style<'_>)],
    ) -> Result<(), ErreurRcon> {
        assert_eq!(joueur, PSEUDO);
        self.dits.borrow_mut().push(texte.to_owned());
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat ici".to_owned()))
    }
}

/// Répond selon le prompt système ; garde chaque appel pour vérifier ce qui part.
struct FauxLlm {
    intention: RefCell<String>,
    reformulation: String,
    bavardage: String,
    distant: bool,
    erreur: bool,
    /// Si présent, le LLM attend ce signal avant de répondre.
    porte: Option<Rc<tokio::sync::Notify>>,
    appels: RefCell<Vec<(String, String)>>,
}

impl Default for FauxLlm {
    fn default() -> Self {
        Self {
            intention: RefCell::new("indice".to_owned()),
            reformulation: "Courage !".to_owned(),
            bavardage: "Je suis Pixel !".to_owned(),
            distant: false,
            erreur: false,
            porte: None,
            appels: RefCell::new(Vec::new()),
        }
    }
}

impl FauxLlm {
    fn intention(intention: &str) -> Self {
        Self {
            intention: RefCell::new(intention.to_owned()),
            ..Self::default()
        }
    }

    fn reformulation(reformulation: &str) -> Self {
        Self {
            reformulation: reformulation.to_owned(),
            ..Self::default()
        }
    }

    fn bavardage(bavardage: &str) -> Self {
        Self {
            bavardage: bavardage.to_owned(),
            ..Self::intention("bavardage")
        }
    }

    fn appels(&self) -> Vec<(String, String)> {
        self.appels.borrow().clone()
    }
}

#[async_trait(?Send)]
impl Llm for FauxLlm {
    fn distant(&self) -> bool {
        self.distant
    }

    async fn completer(
        &self,
        systeme: &str,
        utilisateur: &str,
        image: Option<&[u8]>,
        _en_json: bool,
    ) -> Result<String, ErreurLlm> {
        assert!(image.is_none(), "Pixel n'envoie jamais d'image");
        if let Some(porte) = &self.porte {
            porte.notified().await;
        }
        self.appels
            .borrow_mut()
            .push((systeme.to_owned(), utilisateur.to_owned()));
        if self.erreur {
            return Err(ErreurLlm("injoignable".to_owned()));
        }
        Ok(if systeme.contains("catégorie") {
            json!({"intention": *self.intention.borrow()}).to_string()
        } else if systeme.contains("reformules") {
            self.reformulation.clone()
        } else {
            self.bavardage.clone()
        })
    }
}

fn depot() -> (tempfile::TempDir, Rc<Depot>) {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Rc::new(Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap());
    (dossier, depot)
}

const DEMANDE: Demande<'static> = Demande {
    source: Source::Photo,
    demande_par: "1",
    recompense: "minecraft:diamond",
    recompense_nombre: 3,
};

fn poser_exercice(depot: &Depot, exercice: &Exercice) -> i64 {
    let ident = depot.ajouter(exercice, DEMANDE, debut()).unwrap();
    depot.marquer_pose(ident, debut()).unwrap();
    ident
}

fn double_de_17() -> Exercice {
    photo::calcul("Double de 17").unwrap()
}

fn poser(depot: &Depot) -> i64 {
    poser_exercice(depot, &double_de_17())
}

type Temps = Rc<Cell<DateTime<Utc>>>;

fn pixel(depot: &Rc<Depot>, llm: Option<Rc<FauxLlm>>, jeu: &Rc<FauxJeu>) -> (Rc<Pixel>, Temps) {
    let temps: Temps = Rc::new(Cell::new(debut() + TimeDelta::minutes(1)));
    let lu = Rc::clone(&temps);
    let llm = llm.map(|llm| llm as Rc<dyn Llm>);
    let pixel = Pixel::new(
        jeu.clone(),
        depot.clone(),
        PSEUDO,
        "Pixel",
        llm,
        Box::new(move || lu.get()),
    )
    .unwrap();
    (pixel, temps)
}

async fn dit_par(pixel: &Rc<Pixel>, texte: &str, joueur: &str) {
    let message = MessageChat {
        horodatage: debut(),
        joueur: joueur.to_owned(),
        texte: texte.to_owned(),
    };
    pixel.recevoir(&message).await.unwrap();
    pixel.attendre().await;
}

async fn dit(pixel: &Rc<Pixel>, texte: &str) {
    dit_par(pixel, texte, PSEUDO).await;
}

/// Les tâches de fond de Pixel ont besoin d'un `LocalSet`, comme dans l'agent.
async fn local<F: Future>(scenario: F) -> F::Output {
    tokio::task::LocalSet::new().run_until(scenario).await
}

fn premier_palier() -> String {
    format!("<Pixel> {}", indices(&double_de_17())[0])
}

#[test]
fn adresse_reconnue() {
    for (message, reste) in [
        ("Pixel, aide-moi", "aide-moi"),
        ("pixel: tp", "tp"),
        ("PIXEL ramène-moi", "ramène-moi"),
        ("  Pïxel,  aide ", "aide"),
        ("Pixel", ""),
        ("pixel,", ""),
    ] {
        assert_eq!(adresse(message, "Pixel"), Some(reste), "{message}");
    }
}

#[test]
fn pas_adresse() {
    for message in ["Pixels, aide", "bonjour Pixel", "aide-moi", "Pix, aide"] {
        assert_eq!(adresse(message, "Pixel"), None, "{message}");
    }
}

#[test]
fn mots_cles() {
    use Intention::{Bavardage, Indice, Teleporter};
    for (message, intention) in [
        ("ramène-moi au coffre", Teleporter),
        ("téléporte moi", Teleporter),
        ("tp", Teleporter),
        ("aide-moi", Indice),
        ("je suis bloqué", Indice),
        ("je sais pas", Indice),
        ("donne un indice", Indice),
        ("c'est combien ?", Indice),
        ("ça veut dire quoi", Indice),
        ("je comprends pas", Indice),
        ("tu aimes les creepers ?", Bavardage),
    ] {
        assert_eq!(classer_par_mots_cles(message), intention, "{message}");
    }
}

#[test]
fn commande_de_teleportation() {
    assert_eq!(
        commande_teleportation(&ATELIER, PSEUDO).unwrap(),
        "execute in minecraft:overworld run tp Steve_42 10 64 -3 180 0"
    );
}

#[tokio::test]
async fn message_sans_le_nom_ou_d_un_autre_joueur_ignore() {
    local(async {
        let (_dossier, depot) = depot();
        poser(&depot);
        let (llm, jeu) = (Rc::new(FauxLlm::default()), FauxJeu::nouveau());
        let (pixel, _) = pixel(&depot, Some(llm.clone()), &jeu);
        dit(&pixel, "aide-moi").await;
        dit_par(&pixel, "Pixel, aide-moi", "Copain").await;
        assert!(llm.appels().is_empty() && jeu.dits().is_empty() && jeu.commandes().is_empty());
    })
    .await;
}

#[tokio::test]
async fn teleportation_avec_coffre_actif_puis_patience() {
    local(async {
        let (_dossier, depot) = depot();
        let ident = poser(&depot);
        let jeu = FauxJeu::nouveau();
        let (pixel, temps) = pixel(
            &depot,
            Some(Rc::new(FauxLlm::intention("teleporter"))),
            &jeu,
        );
        dit(&pixel, "Pixel, ramène-moi au coffre").await;
        assert_eq!(
            jeu.commandes(),
            [commande_teleportation(&ATELIER, PSEUDO).unwrap()]
        );
        assert_eq!(
            jeu.dits(),
            ["<Pixel> …", "<Pixel> Te voilà devant le coffre !"]
        );
        assert_eq!(depot.compter("teleportation", ident).unwrap(), 1);
        temps.set(temps.get() + TimeDelta::minutes(4));
        dit(&pixel, "Pixel, ramène-moi au coffre").await;
        assert!(jeu.commandes().len() == 1 && jeu.dernier().contains("patience"));
        temps.set(temps.get() + TimeDelta::minutes(2));
        dit(&pixel, "Pixel, ramène-moi au coffre").await;
        assert_eq!(jeu.commandes().len(), 2);
    })
    .await;
}

#[tokio::test]
async fn teleportation_sans_coffre_refusee() {
    local(async {
        let (_dossier, depot) = depot();
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(
            &depot,
            Some(Rc::new(FauxLlm::intention("teleporter"))),
            &jeu,
        );
        dit(&pixel, "Pixel, tp").await;
        assert!(jeu.commandes().is_empty() && jeu.dernier().contains("Pas de coffre"));
    })
    .await;
}

#[tokio::test]
async fn teleportation_vers_la_position_du_monde() {
    local(async {
        let (_dossier, depot) = depot();
        poser(&depot);
        let jeu = FauxJeu::avec("Teleported Steve_42 to 0.5, -60.0, 10.5", Some((0, -60, 8)));
        let (pixel, _) = pixel(&depot, None, &jeu);
        dit(&pixel, "Pixel, ramène-moi au coffre").await;
        assert_eq!(
            jeu.commandes(),
            [commande_teleportation(&Atelier::new(0, -60, 8), PSEUDO).unwrap()]
        );
    })
    .await;
}

/// Nouveau monde, atelier jamais construit : le bloc d'arrivée peut être de la roche.
#[tokio::test]
async fn teleportation_refusee_sans_atelier_dans_le_monde() {
    local(async {
        let (_dossier, depot) = depot();
        let ident = poser(&depot);
        let jeu = FauxJeu::avec("Teleported", None);
        let (pixel, _) = pixel(&depot, None, &jeu);
        journal::capturer();
        dit(&pixel, "Pixel, ramène-moi au coffre").await;
        let journalise = journal::captures();
        assert!(jeu.commandes().is_empty());
        assert!(jeu.dernier().contains("pas encore construit"));
        assert_eq!(depot.compter("teleportation", ident).unwrap(), 0);
        assert!(
            journalise
                .iter()
                .any(|ligne| ligne.contains("téléportation refusée"))
        );
    })
    .await;
}

#[tokio::test]
async fn teleportation_enfant_deconnecte() {
    local(async {
        let (_dossier, depot) = depot();
        let ident = poser(&depot);
        let jeu = FauxJeu::avec("No entity was found", Some((10, 64, -5)));
        let (pixel, _) = pixel(&depot, None, &jeu);
        dit(&pixel, "Pixel, tp").await;
        assert!(jeu.dernier().contains("réessaie"));
        assert_eq!(depot.compter("teleportation", ident).unwrap(), 0);
    })
    .await;
}

#[tokio::test]
async fn indices_dans_l_ordre_puis_le_dernier_repete() {
    local(async {
        let (_dossier, depot) = depot();
        let ident = poser(&depot);
        let paliers = indices(&double_de_17());
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(&depot, None, &jeu); // sans LLM : indices bruts
        for _ in 0..=paliers.len() {
            dit(&pixel, "Pixel, aide-moi").await;
        }
        let dits: Vec<String> = jeu
            .dits()
            .into_iter()
            .filter(|d| d != "<Pixel> …")
            .collect();
        let mut attendus: Vec<String> = paliers.iter().map(|p| format!("<Pixel> {p}")).collect();
        attendus.push(format!("<Pixel> {}", paliers.last().unwrap()));
        assert_eq!(dits, attendus);
        assert_eq!(
            depot.compter(EVENEMENT_INDICE, ident).unwrap(),
            paliers.len() + 1
        );
    })
    .await;
}

#[tokio::test]
async fn reformulation_acceptee() {
    local(async {
        let (_dossier, depot) = depot();
        poser(&depot);
        let llm = Rc::new(FauxLlm::reformulation(
            "Astuce : ajoute 17 à lui-même, tu vas y arriver !",
        ));
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(&depot, Some(llm.clone()), &jeu);
        dit(&pixel, "Pixel, aide-moi").await;
        assert_eq!(
            jeu.dernier(),
            "<Pixel> Astuce : ajoute 17 à lui-même, tu vas y arriver !"
        );
        let appels = llm.appels();
        let (_, utilisateur) = appels.last().unwrap();
        assert!(utilisateur.contains("Double de 17 = ?") && !utilisateur.contains("34"));
    })
    .await;
}

#[tokio::test]
async fn reformulation_dangereuse_remplacee_par_l_indice_brut() {
    local(async {
        let trop_longue = "x".repeat(300);
        for reformulation in [
            "Ça fait 34 !",
            "C'est trente-quatre.",
            "Le double de 7 = 15, ajoute 10.",
            "",
            trop_longue.as_str(),
        ] {
            let (_dossier, depot) = depot();
            poser(&depot);
            let jeu = FauxJeu::nouveau();
            let (pixel, _) = pixel(
                &depot,
                Some(Rc::new(FauxLlm::reformulation(reformulation))),
                &jeu,
            );
            dit(&pixel, "Pixel, aide-moi").await;
            assert_eq!(jeu.dernier(), premier_palier(), "{reformulation}");
        }
    })
    .await;
}

#[tokio::test]
async fn bavardage_sans_chiffre() {
    local(async {
        let (_dossier, depot) = depot();
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(
            &depot,
            Some(Rc::new(FauxLlm::bavardage("J'adore les moutons !"))),
            &jeu,
        );
        dit(&pixel, "Pixel, tu aimes quoi ?").await;
        assert_eq!(jeu.dernier(), "<Pixel> J'adore les moutons !");
    })
    .await;
}

#[tokio::test]
async fn bavardage_avec_chiffre_remplace() {
    local(async {
        let (_dossier, depot) = depot();
        let jeu = FauxJeu::nouveau();
        let llm = Rc::new(FauxLlm::bavardage("348 divisé par 12, ça fait 29."));
        let (pixel, _) = pixel(&depot, Some(llm), &jeu);
        dit(&pixel, "Pixel, et 348 divisé par 12 ?").await;
        assert!(
            jeu.dernier()
                .starts_with("<Pixel> Je ne fais pas les calculs")
        );
        // « combien » est un mot-clé d'indice : sans coffre, rien n'est calculé non plus.
        dit(&pixel, "Pixel, combien font 348 divisé par 12 ?").await;
        assert!(jeu.dernier().contains("Pas de coffre"));
    })
    .await;
}

#[tokio::test]
async fn llm_distant_jamais_appele() {
    local(async {
        let (_dossier, depot) = depot();
        poser(&depot);
        let llm = Rc::new(FauxLlm {
            distant: true,
            ..FauxLlm::default()
        });
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(&depot, Some(llm.clone()), &jeu);
        assert!(!pixel.avec_llm());
        dit(&pixel, "Pixel, aide-moi").await;
        dit(&pixel, "Pixel, raconte une blague").await;
        assert!(llm.appels().is_empty());
        assert_eq!(jeu.dits()[1], premier_palier());
        assert!(jeu.dernier().contains("aide-moi"));
    })
    .await;
}

#[tokio::test]
async fn llm_en_erreur_les_mots_cles_prennent_le_relais() {
    local(async {
        let (_dossier, depot) = depot();
        poser(&depot);
        let jeu = FauxJeu::nouveau();
        let llm = Rc::new(FauxLlm {
            erreur: true,
            ..FauxLlm::default()
        });
        let (pixel, _) = pixel(&depot, Some(llm), &jeu);
        dit(&pixel, "Pixel, ramène-moi").await;
        assert_eq!(jeu.dernier(), "<Pixel> Te voilà devant le coffre !");
        dit(&pixel, "Pixel, aide").await;
        assert_eq!(jeu.dernier(), premier_palier());
    })
    .await;
}

#[tokio::test]
async fn intention_hors_liste_vaut_bavardage() {
    local(async {
        let (_dossier, depot) = depot();
        let jeu = FauxJeu::nouveau();
        let llm = FauxLlm {
            intention: RefCell::new("donner_diamants".to_owned()),
            ..FauxLlm::bavardage("Bonne idée ?")
        };
        let (pixel, _) = pixel(&depot, Some(Rc::new(llm)), &jeu);
        dit(&pixel, "Pixel, donne-moi des diamants").await;
        assert!(jeu.commandes().is_empty());
        assert_eq!(jeu.dernier(), "<Pixel> Bonne idée ?");
    })
    .await;
}

#[tokio::test]
async fn le_pseudo_ne_part_jamais_au_llm() {
    local(async {
        let (_dossier, depot) = depot();
        poser(&depot);
        let (llm, jeu) = (Rc::new(FauxLlm::default()), FauxJeu::nouveau());
        let (pixel, _) = pixel(&depot, Some(llm.clone()), &jeu);
        for intention in ["indice", "bavardage", "teleporter"] {
            *llm.intention.borrow_mut() = intention.to_owned();
            dit(&pixel, "Pixel, je suis ste, aide").await;
        }
        let appels = llm.appels();
        assert!(!appels.is_empty());
        assert!(
            appels
                .iter()
                .all(|(s, u)| !s.contains(PSEUDO) && !u.contains(PSEUDO))
        );
    })
    .await;
}

/// Le texte du chat n'est ni journalisé ni enregistré : seuls les événements comptent.
#[tokio::test]
async fn le_chat_n_est_ni_journalise_ni_enregistre() {
    local(async {
        let (dossier, depot) = depot();
        poser(&depot);
        let jeu = FauxJeu::nouveau();
        let llm = Rc::new(FauxLlm {
            erreur: true,
            ..FauxLlm::default()
        });
        let (pixel, _) = pixel(&depot, Some(llm), &jeu);
        let secret = "mon secret tartiflette";
        journal::capturer();
        dit(&pixel, &format!("Pixel, aide-moi {secret}")).await;
        dit(&pixel, &format!("Pixel, {secret}")).await;
        dit(&pixel, &format!("Pixel, ramène-moi {secret}")).await;
        let journalise = journal::captures();
        assert!(!journalise.is_empty(), "le LLM en erreur est journalisé");
        assert!(
            journalise
                .iter()
                .all(|ligne| !ligne.contains("tartiflette")),
            "{journalise:?}"
        );
        drop(pixel);
        drop(depot);
        let base = std::fs::read(dossier.path().join("mdr.sqlite")).unwrap();
        assert!(!String::from_utf8_lossy(&base).contains("tartiflette"));
    })
    .await;
}

#[tokio::test]
async fn message_vide() {
    local(async {
        let (_dossier, depot) = depot();
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(&depot, Some(Rc::new(FauxLlm::default())), &jeu);
        dit(&pixel, "Pixel,").await;
        assert!(jeu.dernier().contains("aide-moi"));
    })
    .await;
}

#[tokio::test]
async fn une_demande_a_la_fois() {
    local(async {
        let (_dossier, depot) = depot();
        poser(&depot);
        let porte = Rc::new(tokio::sync::Notify::new());
        let llm = Rc::new(FauxLlm {
            porte: Some(porte.clone()),
            ..FauxLlm::default()
        });
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(&depot, Some(llm), &jeu);
        let message = |texte: &str| MessageChat {
            horodatage: debut(),
            joueur: PSEUDO.to_owned(),
            texte: texte.to_owned(),
        };
        pixel.recevoir(&message("Pixel, aide-moi")).await.unwrap();
        tokio::task::yield_now().await; // la première réponse attend maintenant le LLM
        pixel.recevoir(&message("Pixel, alors ?")).await.unwrap();
        porte.notify_one();
        pixel.attendre().await;
        let dits = jeu.dits();
        assert_eq!(dits[0], "<Pixel> …");
        assert!(dits[1].contains("Je réfléchis encore"));
        assert_eq!(depot.compter(EVENEMENT_INDICE, 1).unwrap(), 1);
    })
    .await;
}

fn coffre_factice() -> ExerciceStocke {
    ExerciceStocke {
        id: 1,
        exercice: double_de_17(),
        jeu: "coffre-du-soir".to_owned(),
        source: Source::Photo,
        statut: Statut::Pose,
        demande_par: "1".to_owned(),
        recompense: "minecraft:diamond".to_owned(),
        recompense_nombre: 3,
        cree_le: debut(),
        pose_le: None,
        ouvert_le: None,
        niveaux_donnes: false,
    }
}

#[tokio::test]
async fn l_annonce_presente_pixel() {
    let jeu = FauxJeu::nouveau();
    let coffre = CoffreDuSoir::new(jeu.clone(), PSEUDO, 3, Some("Pixel")).unwrap();
    coffre.annoncer(&coffre_factice()).await.unwrap();
    assert!(jeu.dits().iter().any(|d| d.contains("« Pixel, aide-moi »")));
    jeu.dits.borrow_mut().clear();
    let coffre = CoffreDuSoir::new(jeu.clone(), PSEUDO, 3, None).unwrap();
    coffre.annoncer(&coffre_factice()).await.unwrap();
    assert!(!jeu.dits().iter().any(|d| d.contains("Pixel")));
}

#[tokio::test]
async fn mots_cles_d_abord_sans_appeler_le_llm() {
    local(async {
        let (_dossier, depot) = depot();
        poser(&depot);
        let (llm, jeu) = (Rc::new(FauxLlm::intention("bavardage")), FauxJeu::nouveau());
        let (pixel, _) = pixel(&depot, Some(llm.clone()), &jeu);
        dit(&pixel, "Pixel, aide-moi").await;
        assert!(!llm.appels().iter().any(|(s, _)| s.contains("catégorie")));
        assert_eq!(jeu.dernier(), "<Pixel> Courage !");
    })
    .await;
}

#[tokio::test]
async fn emojis_retires() {
    local(async {
        let (_dossier, depot) = depot();
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(
            &depot,
            Some(Rc::new(FauxLlm::bavardage("Salut ! 😊✨ ⛏️"))),
            &jeu,
        );
        dit(&pixel, "Pixel, salut").await;
        assert_eq!(jeu.dernier(), "<Pixel> Salut !");
    })
    .await;
}

fn llm_du_scenario(config: &Value) -> Option<Rc<FauxLlm>> {
    let config = config.as_object()?;
    let texte = |cle: &str, defaut: &str| {
        config
            .get(cle)
            .and_then(Value::as_str)
            .unwrap_or(defaut)
            .to_owned()
    };
    let vrai = |cle: &str| config.get(cle).and_then(Value::as_bool).unwrap_or(false);
    Some(Rc::new(FauxLlm {
        intention: RefCell::new(texte("intention", "indice")),
        reformulation: texte("reformulation", "Courage !"),
        bavardage: texte("bavardage", "Je suis Pixel !"),
        distant: vrai("distant"),
        erreur: vrai("erreur"),
        porte: None,
        appels: RefCell::new(Vec::new()),
    }))
}

async fn jouer_scenario(cas: &Value, rejeu: Rc<commun::Rejeu>) -> Value {
    let e = &cas["entree"];
    let (_dossier, depot) = depot();
    if !e["exercice"].is_null() {
        poser_exercice(
            &depot,
            &serde_json::from_value(e["exercice"].clone()).unwrap(),
        );
    }
    let llm = llm_du_scenario(&e["llm"]);
    let temps: Temps = Rc::new(Cell::new(debut() + TimeDelta::minutes(1)));
    let lu = Rc::clone(&temps);
    let pixel = Pixel::new(
        rejeu,
        depot.clone(),
        e["pseudo"].as_str().unwrap(),
        e["assistant"].as_str().unwrap(),
        llm.clone().map(|llm| llm as Rc<dyn Llm>),
        Box::new(move || lu.get()),
    )
    .unwrap();
    for message in e["messages"].as_array().unwrap() {
        let brut = message[0].as_str().unwrap();
        let (joueur, texte) = brut.split_once('|').unwrap_or((PSEUDO, brut));
        temps.set(temps.get() + TimeDelta::minutes(message[1].as_i64().unwrap()));
        dit_par(&pixel, texte, joueur).await;
    }
    let evenements: Vec<Value> = depot
        .evenements_entre(debut() - TimeDelta::days(1), debut() + TimeDelta::days(1))
        .unwrap()
        .iter()
        .map(|ev| {
            json!([
                ev.exercice_id,
                ev.genre,
                ev.quand.to_rfc3339_opts(SecondsFormat::Secs, false)
            ])
        })
        .collect();
    let appels = llm.map(|llm| {
        llm.appels()
            .iter()
            .map(|(s, u)| json!([s, u]))
            .collect::<Vec<_>>()
    });
    json!({"evenements": evenements, "appels": appels})
}

/// Consignes au modèle, reconnaissance du nom, mots-clés, nettoyage, et vingt-quatre
/// conversations : mêmes commandes et mêmes phrases dans le chat que la version Python,
/// mêmes événements notés, et exactement les mêmes textes envoyés au LLM.
#[tokio::test]
async fn oracle_python() {
    local(async {
        for cas in commun::oracle("pixel") {
            let e = &cas["entree"];
            let rejeu = Rc::new(commun::Rejeu::new(&cas));
            let texte = |cle: &str| e[cle].as_str().unwrap();
            match texte("f") {
                "constantes" => {
                    let s = &cas["sortie"];
                    assert_eq!(s["classement"][0], SYSTEME_CLASSEMENT);
                    assert_eq!(s["reformulation"], SYSTEME_REFORMULATION);
                    assert_eq!(s["bavardage"], SYSTEME_BAVARDAGE);
                }
                "adresse" => {
                    assert_eq!(
                        json!(adresse(texte("message"), texte("nom"))),
                        cas["sortie"],
                        "{e}"
                    )
                }
                "classer" => {
                    assert_eq!(
                        classer_par_mots_cles(texte("demande")).nom(),
                        cas["sortie"],
                        "{e}"
                    )
                }
                "normaliser_espaces" => {
                    assert_eq!(normaliser_espaces(texte("demande")), cas["sortie"], "{e}")
                }
                "propre" => assert_eq!(propre(texte("brut")), cas["sortie"], "{e}"),
                "verifier_nom" => commun::comparer(&cas, verifier_nom(texte("nom"))),
                "commande_teleportation" => {
                    commun::comparer(&cas, commande_teleportation(&ATELIER, texte("pseudo")))
                }
                "scenario" => {
                    let sortie = jouer_scenario(&cas, rejeu.clone()).await;
                    commun::conclure::<_, String>(&cas, &rejeu, Ok(sortie));
                }
                autre => panic!("fonction inconnue : {autre}"),
            }
        }
    })
    .await;
}

const DICTEE: &str = "Dans la ville de Dubaï s'élève la plus haute tour jamais construite.";

/// Le trou « plus » d'une dictée enregistrée dans la base.
fn trou_de_dictee(depot: &Depot) -> Exercice {
    commun::trou_en_base(depot, DICTEE, "plus")
}

/// De quoi relire la phrase d'une dictée : la fausse diffusion note ce qui est joué.
fn lecteur(depot: &Rc<Depot>) -> (Rc<Lecteur>, Rc<FausseDiffusion>, Rc<VoixDeTest>) {
    let (diffusion, voix) = (
        Rc::new(FausseDiffusion::new()),
        Rc::new(VoixDeTest::default()),
    );
    let lecteur = Rc::new(Lecteur::new(diffusion.clone(), voix.clone(), depot.clone()));
    (lecteur, diffusion, voix)
}

#[tokio::test]
async fn dictee_trois_paliers_sans_llm_et_lecture_lente() {
    local(async {
        let (_dossier, depot) = depot();
        let ex = trou_de_dictee(&depot);
        assert_eq!(ex.reponse, "plus");
        let ident = poser_exercice(&depot, &ex);
        let jeu = FauxJeu::nouveau();
        // Un LLM local est branché : devant une dictée, il ne doit pas reformuler.
        let llm = Rc::new(FauxLlm::reformulation("Ça s'écrit p, l, u, s !"));
        let (pixel, _) = pixel(&depot, Some(llm.clone()), &jeu);
        let (lecteur, diffusion, voix) = lecteur(&depot);
        pixel.avec_lecteur(lecteur);
        for _ in 0..4 {
            dit(&pixel, "Pixel, aide-moi").await;
        }
        let dits: Vec<String> = jeu
            .dits()
            .into_iter()
            .filter(|d| d != "<Pixel> …")
            .collect();
        assert_eq!(
            dits,
            [
                "<Pixel> Écoute bien, je relis la phrase lentement.",
                "<Pixel> Le mot a 4 lettres.",
                "<Pixel> Il commence par « p » et a 4 lettres.",
                "<Pixel> Il commence par « p » et a 4 lettres.",
            ]
        );
        // Une seule lecture, celle du premier palier, avec le son lent de la phrase, par le
        // chat vocal : plus aucune commande de son n'est envoyée au serveur.
        let (normal, lent) = (
            fichier_du_son(dictee::son(&ex).unwrap()),
            fichier_du_son(dictee::son_lent(&ex).unwrap()),
        );
        assert_eq!(
            diffusion.actions(),
            [
                format!("garder {normal}"),
                format!("garder {lent}"),
                format!("redire {lent}")
            ]
        );
        assert_eq!(
            *voix.lus.borrow(),
            [(DICTEE.to_owned(), false), (DICTEE.to_owned(), true)]
        );
        assert!(jeu.commandes().is_empty(), "{:?}", jeu.commandes());
        assert!(llm.appels().is_empty(), "{:?}", llm.appels());
        assert_eq!(depot.compter(EVENEMENT_INDICE, ident).unwrap(), 4);
    })
    .await;
}

#[tokio::test]
async fn dictee_pixel_ne_bavarde_pas_et_n_interroge_pas_le_llm() {
    local(async {
        let (_dossier, depot) = depot();
        poser_exercice(&depot, &trou_de_dictee(&depot));
        let jeu = FauxJeu::nouveau();
        let llm = Rc::new(FauxLlm::bavardage("Plus s'écrit avec un s."));
        let (pixel, _) = pixel(&depot, Some(llm.clone()), &jeu);
        dit(&pixel, "Pixel, tu aimes quoi ?").await;
        assert!(
            jeu.dernier().starts_with("<Pixel> Pendant une dictée"),
            "{}",
            jeu.dernier()
        );
        // Le message a pu être classé par le LLM, jamais discuté avec lui.
        assert!(
            llm.appels()
                .iter()
                .all(|(systeme, _)| systeme == SYSTEME_CLASSEMENT),
            "{:?}",
            llm.appels()
        );
        assert!(
            !jeu.dits()
                .iter()
                .any(|d| d.to_lowercase().contains("plus s"))
        );
    })
    .await;
}

#[tokio::test]
async fn dictee_sans_son_lent_le_son_normal_puis_rien() {
    local(async {
        let (_dossier, depot) = depot();
        let mut ex = trou_de_dictee(&depot);
        ex.donnees.remove("son_lent");
        poser_exercice(&depot, &ex);
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(&depot, None, &jeu);
        let (lecteur, diffusion, _) = lecteur(&depot);
        pixel.avec_lecteur(lecteur);
        dit(&pixel, "Pixel, aide-moi").await;
        let normal = fichier_du_son(dictee::son(&ex).unwrap());
        assert_eq!(
            diffusion.actions(),
            [format!("garder {normal}"), format!("redire {normal}")]
        );

        let (_dossier, depot) = depot_sans_son();
        let jeu = FauxJeu::nouveau();
        let (muet, _) = crate::pixel(&depot, None, &jeu);
        let (lecteur, diffusion, _) = crate::lecteur(&depot);
        muet.avec_lecteur(lecteur);
        dit(&muet, "Pixel, aide-moi").await;
        assert!(jeu.commandes().is_empty());
        assert!(diffusion.actions().is_empty());
        assert_eq!(
            jeu.dernier(),
            "<Pixel> Je n'arrive pas à relire cette phrase. Demande à un parent."
        );
    })
    .await;
}

#[tokio::test]
async fn dictee_sans_chat_vocal_ou_voix_en_panne_pixel_le_dit_sans_erreur() {
    local(async {
        // Sans lecteur du tout : les dictées ne sont pas réglées.
        let (_dossier, depot) = depot();
        poser_exercice(&depot, &trou_de_dictee(&depot));
        let jeu = FauxJeu::nouveau();
        let (pixel, _) = pixel(&depot, None, &jeu);
        dit(&pixel, "Pixel, aide-moi").await;
        assert_eq!(
            jeu.dernier(),
            "<Pixel> Je n'arrive pas à relire cette phrase. Demande à un parent."
        );

        // Le serveur refuse de jouer, puis la voix tombe : l'indice est dit, puis l'excuse.
        for voix_en_panne in [false, true] {
            let (_dossier, depot) = crate::depot();
            poser_exercice(&depot, &trou_de_dictee(&depot));
            let jeu = FauxJeu::nouveau();
            let (pixel, _) = crate::pixel(&depot, None, &jeu);
            let (lecteur, diffusion, voix) = lecteur(&depot);
            diffusion.en_panne.set(!voix_en_panne);
            voix.en_panne.set(voix_en_panne);
            pixel.avec_lecteur(lecteur);
            dit(&pixel, "Pixel, aide-moi").await;
            let dits = jeu.dits();
            assert_eq!(
                dits[dits.len() - 2..],
                [
                    "<Pixel> Écoute bien, je relis la phrase lentement.",
                    "<Pixel> Je n'arrive pas à relire cette phrase. Demande à un parent."
                ]
            );
        }
    })
    .await;
}

fn depot_sans_son() -> (tempfile::TempDir, Rc<Depot>) {
    let (dossier, depot) = depot();
    let mut ex = trou_de_dictee(&depot);
    ex.donnees.remove("son");
    ex.donnees.remove("son_lent");
    poser_exercice(&depot, &ex);
    (dossier, depot)
}
