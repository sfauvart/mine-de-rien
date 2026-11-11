//! Dire un son à l'enfant par le chat vocal : le fichier déposé, la commande envoyée.

mod commun;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use async_trait::async_trait;
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::{ErreurRcon, TAILLE_MAX_COMMANDE};
use mine_de_rien::diffusion::{
    ChatVocal, Diffusion, Etat, commande_couper, commande_etat, commande_jouer, verifier_fichier,
};
use mine_de_rien::minecraft::texte::Style;
use mine_de_rien::son::Pcm;

use commun::FausseDiffusion;

/// Un serveur qui répond ce qu'on lui a dit de répondre, et relève à chaque `mdrvoix jouer`
/// ce que contient le dossier partagé à cet instant : c'est là que le plugin lit le son.
struct FauxServeur {
    dossier: PathBuf,
    reponse: RefCell<Result<String, ErreurRcon>>,
    commandes: RefCell<Vec<String>>,
    /// Nom et contenu des fichiers présents quand la commande est arrivée.
    vus: RefCell<Vec<(String, Vec<u8>)>>,
}

impl FauxServeur {
    fn new(dossier: &Path, reponse: &str) -> Rc<Self> {
        Rc::new(Self {
            dossier: dossier.to_owned(),
            reponse: RefCell::new(Ok(reponse.to_owned())),
            commandes: RefCell::new(Vec::new()),
            vus: RefCell::new(Vec::new()),
        })
    }
}

fn fichiers(dossier: &Path) -> Vec<String> {
    let Ok(entrees) = std::fs::read_dir(dossier) else {
        return Vec::new();
    };
    let mut noms: Vec<String> = entrees
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    noms.sort();
    noms
}

#[async_trait(?Send)]
impl AdaptateurJeu for FauxServeur {
    async fn commande(&self, commande: &str) -> Result<String, ErreurRcon> {
        self.commandes.borrow_mut().push(commande.to_owned());
        if commande.starts_with("mdrvoix jouer") {
            for nom in fichiers(&self.dossier) {
                let octets = std::fs::read(self.dossier.join(&nom)).unwrap();
                self.vus.borrow_mut().push((nom, octets));
            }
        }
        self.reponse.borrow().clone()
    }

    async fn dire(&self, _: &str, _: &str, _: &[(&str, Style<'_>)]) -> Result<(), ErreurRcon> {
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat ici".to_owned()))
    }
}

/// Un dixième de seconde de voix, à la fréquence de Piper.
fn son() -> Pcm {
    Pcm {
        frequence: 22_050,
        echantillons: (0..2_205).map(|i| (i as f32 / 10.0).sin() * 0.5).collect(),
    }
}

#[test]
fn commandes_du_plugin() {
    assert_eq!(
        commande_jouer("Steve", "son-12-0.pcm").unwrap(),
        "mdrvoix jouer Steve son-12-0.pcm"
    );
    assert_eq!(commande_couper("Steve").unwrap(), "mdrvoix couper Steve");
    assert_eq!(commande_etat("Steve").unwrap(), "mdrvoix etat Steve");
}

#[test]
fn le_pseudo_est_controle_avant_d_entrer_dans_une_commande() {
    for pseudo in [
        "",
        "@a",
        "Steve couper",
        "Steve\nstop",
        "Stève",
        "a".repeat(17).as_str(),
    ] {
        assert!(commande_jouer(pseudo, "son-1-0.pcm").is_err(), "{pseudo:?}");
        assert!(commande_couper(pseudo).is_err(), "{pseudo:?}");
        assert!(commande_etat(pseudo).is_err(), "{pseudo:?}");
    }
}

#[test]
fn le_nom_de_fichier_est_controle_avant_d_entrer_dans_une_commande() {
    for nom in ["son-1-0.pcm", "7.pcm", "phrase-12-lent.pcm"] {
        assert_eq!(verifier_fichier(nom).unwrap(), nom);
    }
    let trop_long = format!("{}.pcm", "a".repeat(65));
    for nom in [
        "",
        ".pcm",
        "../secret.pcm",
        "a/b.pcm",
        "/etc/passwd",
        "a\\b.pcm",
        "son.ogg",
        "son.pcm.tmp",
        "Son.pcm",
        "été.pcm",
        "a b.pcm",
        "a.pcm ",
        "a..pcm",
        "a.pcm\nstop",
        "a.pcm stop",
        trop_long.as_str(),
    ] {
        assert!(verifier_fichier(nom).is_err(), "{nom:?}");
        assert!(commande_jouer("Steve", nom).is_err(), "{nom:?}");
    }
}

#[test]
fn les_commandes_tiennent_dans_un_paquet_rcon() {
    // Le pseudo le plus long, le nom de fichier le plus long.
    let pseudo = "W".repeat(16);
    let fichier = format!("{}.pcm", "a".repeat(64));
    let commande = commande_jouer(&pseudo, &fichier).unwrap();
    assert!(commande.len() <= TAILLE_MAX_COMMANDE, "{}", commande.len());
    assert!(commande.len() < 128);
}

#[tokio::test]
async fn dire_depose_le_son_a_48_000_hz_puis_le_designe_au_plugin() {
    let racine = tempfile::tempdir().unwrap();
    // Le dossier partagé n'existe pas encore : l'agent le crée.
    let dossier = racine.path().join("sons");
    let serveur = FauxServeur::new(&dossier, "ok\n");
    let chat_vocal = ChatVocal::new(serveur.clone(), "Steve", &dossier).unwrap();

    chat_vocal.dire(&son()).await.unwrap();

    let commandes = serveur.commandes.borrow();
    assert_eq!(commandes.len(), 1);
    let nom = commandes[0].strip_prefix("mdrvoix jouer Steve ").unwrap();
    verifier_fichier(nom).unwrap();
    // Un numéro, jamais un mot de ce qui est lu.
    assert!(nom.starts_with("son-"));

    // Quand la commande est arrivée, le son était là, entier, et seul.
    let vus = serveur.vus.borrow();
    assert_eq!(vus.len(), 1);
    assert_eq!(vus[0].0, nom);
    assert_eq!(vus[0].1, son().reechantillonner(48_000).en_pcm16());
    assert_eq!(vus[0].1.len(), 2 * 4_800);

    // Le plugin l'a lu en répondant : il ne reste rien sur le disque.
    assert!(fichiers(&dossier).is_empty());
}

#[tokio::test]
async fn deux_sons_ne_portent_jamais_le_meme_nom() {
    let racine = tempfile::tempdir().unwrap();
    let serveur = FauxServeur::new(racine.path(), "ok");
    let chat_vocal = ChatVocal::new(serveur.clone(), "Steve", racine.path()).unwrap();
    for _ in 0..20 {
        chat_vocal.dire(&son()).await.unwrap();
    }
    let mut commandes = serveur.commandes.borrow().clone();
    commandes.sort();
    commandes.dedup();
    assert_eq!(commandes.len(), 20);
}

#[tokio::test]
async fn un_refus_du_serveur_est_une_erreur_et_le_fichier_est_efface() {
    let racine = tempfile::tempdir().unwrap();
    for reponse in [
        "refusé : sans chat vocal",
        "Unknown or incomplete command, see below for error",
        "",
    ] {
        let serveur = FauxServeur::new(racine.path(), reponse);
        let chat_vocal = ChatVocal::new(serveur.clone(), "Steve", racine.path()).unwrap();
        let erreur = chat_vocal.dire(&son()).await.unwrap_err();
        assert!(
            erreur.0.starts_with("Le serveur n'a pas joué le son"),
            "{erreur}"
        );
        assert!(fichiers(racine.path()).is_empty());
    }
    // Serveur injoignable : même ménage.
    let serveur = FauxServeur::new(racine.path(), "");
    *serveur.reponse.borrow_mut() = Err(ErreurRcon("serveur injoignable".to_owned()));
    let chat_vocal = ChatVocal::new(serveur.clone(), "Steve", racine.path()).unwrap();
    assert_eq!(
        chat_vocal.dire(&son()).await.unwrap_err().0,
        "serveur injoignable"
    );
    assert!(fichiers(racine.path()).is_empty());
}

#[tokio::test]
async fn un_son_vide_n_est_ni_depose_ni_envoye() {
    let racine = tempfile::tempdir().unwrap();
    let serveur = FauxServeur::new(racine.path(), "ok");
    let chat_vocal = ChatVocal::new(serveur.clone(), "Steve", racine.path()).unwrap();
    let vide = Pcm {
        frequence: 22_050,
        echantillons: Vec::new(),
    };
    assert!(chat_vocal.dire(&vide).await.is_err());
    assert!(serveur.commandes.borrow().is_empty());
    assert!(fichiers(racine.path()).is_empty());
}

#[tokio::test]
async fn l_etat_vient_de_la_reponse_du_plugin() {
    let racine = tempfile::tempdir().unwrap();
    for (reponse, etat, relie) in [
        ("relié", Etat::Relie, true),
        ("relié\n", Etat::Relie, true),
        ("sans chat vocal", Etat::SansChatVocal, false),
        ("absent", Etat::Absent, false),
        (
            "Unknown or incomplete command",
            Etat::Inconnu("Unknown or incomplete command".to_owned()),
            false,
        ),
        ("", Etat::Inconnu(String::new()), false),
    ] {
        let serveur = FauxServeur::new(racine.path(), reponse);
        let chat_vocal = ChatVocal::new(serveur.clone(), "Steve", racine.path()).unwrap();
        assert_eq!(chat_vocal.etat().await.unwrap(), etat);
        assert_eq!(chat_vocal.relie().await.unwrap(), relie);
        assert_eq!(serveur.commandes.borrow()[0], "mdrvoix etat Steve");
    }
}

#[tokio::test]
async fn couper_envoie_la_commande() {
    let racine = tempfile::tempdir().unwrap();
    let serveur = FauxServeur::new(racine.path(), "ok");
    let chat_vocal = ChatVocal::new(serveur.clone(), "Steve", racine.path()).unwrap();
    chat_vocal.couper().await.unwrap();
    assert_eq!(*serveur.commandes.borrow(), ["mdrvoix couper Steve"]);
}

#[test]
fn sans_dossier_partage_ou_sans_pseudo_sur_la_diffusion_ne_se_cree_pas() {
    let racine = tempfile::tempdir().unwrap();
    let serveur = FauxServeur::new(racine.path(), "ok");
    assert!(ChatVocal::new(serveur.clone(), "Steve", Path::new("")).is_err());
    assert!(ChatVocal::new(serveur, "@a", racine.path()).is_err());
}

#[tokio::test]
async fn la_fausse_diffusion_note_ce_qui_est_dit() {
    let fausse = FausseDiffusion::new();
    let diffusion: &dyn Diffusion = &fausse;
    assert!(diffusion.relie().await.unwrap());
    diffusion.dire(&son()).await.unwrap();
    assert_eq!(*fausse.dits.borrow(), [son()]);
    diffusion.couper().await.unwrap();
    assert_eq!(fausse.coupures.get(), 1);
    fausse.reliee.set(false);
    assert!(!diffusion.relie().await.unwrap());
}
