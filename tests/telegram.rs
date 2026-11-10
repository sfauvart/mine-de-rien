//! Canal Telegram et commandes des parents, sans réseau : les mises à jour sont écrites au
//! format de l'API Bot, et `teloxide` parle à un faux serveur local.

mod commun;

use std::cell::Cell;
use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use mine_de_rien::adapters::channel::base::{Canal, MessageParent};
use mine_de_rien::adapters::channel::telegram::{TAILLE_MESSAGE, Telegram, Tri, decouper, trier};
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::alea::Alea;
use mine_de_rien::config::Coffre as ConfigCoffre;
use mine_de_rien::games::coffre_du_soir::CoffreDuSoir;
use mine_de_rien::minecraft::texte::Style;
use mine_de_rien::orchestrateur::CommandesParents;
use mine_de_rien::storage::sqlite::Depot;
use serde_json::{Value, json};
use teloxide::Bot;
use teloxide::types::Update;

const PARENTS: [i64; 2] = [111, 222];

fn parents() -> HashSet<i64> {
    PARENTS.into_iter().collect()
}

fn conversation(expediteur: i64, genre: &str) -> Value {
    match genre {
        "private" => json!({"id": expediteur, "type": "private", "first_name": "P"}),
        autre => json!({"id": -1000, "type": autre, "title": "Famille"}),
    }
}

fn message(expediteur: i64, genre: &str, contenu: Value) -> Value {
    let mut message = json!({
        "message_id": 1,
        "date": 1_762_624_800,
        "chat": conversation(expediteur, genre),
        "from": {"id": expediteur, "is_bot": false, "first_name": "P"},
    });
    message
        .as_object_mut()
        .unwrap()
        .extend(contenu.as_object().cloned().unwrap());
    message
}

fn update(numero: u32, message: Value) -> Value {
    json!({"update_id": numero, "message": message})
}

/// Lit une mise à jour comme `teloxide` la reçoit : depuis du texte JSON. (Son désérialiseur
/// ne reconnaît pas le genre de la mise à jour depuis un `Value` déjà analysé.)
fn lire(brut: Value) -> Update {
    serde_json::from_str(&brut.to_string()).unwrap()
}

fn update_texte(expediteur: i64, texte: &str, genre: &str) -> Update {
    lire(update(
        1,
        message(expediteur, genre, json!({"text": texte})),
    ))
}

fn photo(expediteur: i64, legende: Option<&str>, genre: &str) -> Value {
    let mut contenu = json!({"photo": [
        {"file_id": "petite", "file_unique_id": "u1", "width": 90, "height": 60, "file_size": 100},
        {"file_id": "grande", "file_unique_id": "u2", "width": 1280, "height": 885, "file_size": 900},
    ]});
    if let Some(legende) = legende {
        contenu["caption"] = json!(legende);
    }
    message(expediteur, genre, contenu)
}

fn update_photo(expediteur: i64, legende: Option<&str>, genre: &str) -> Update {
    lire(update(1, photo(expediteur, legende, genre)))
}

#[test]
fn seuls_les_parents_en_prive_passent() {
    assert_eq!(
        trier(&update_texte(111, "/coffre 1 + 1", "private"), &parents()),
        Tri::Message(MessageParent::texte(111, "/coffre 1 + 1"))
    );
    assert_eq!(
        trier(&update_texte(333, "/coffre 1 + 1", "private"), &parents()),
        Tri::Ignorer
    );
    assert_eq!(
        trier(&update_texte(111, "/coffre 1 + 1", "group"), &parents()),
        Tri::Ignorer
    );
    assert_eq!(
        trier(
            &update_texte(111, "/coffre 1 + 1", "supergroup"),
            &parents()
        ),
        Tri::Ignorer
    );
}

#[test]
fn start_repond_a_n_importe_qui_son_identifiant() {
    for texte in ["/start", "/start@MonBot", "/START", "/start bonjour"] {
        assert_eq!(
            trier(&update_texte(333, texte, "private"), &parents()),
            Tri::Identifiant {
                conversation: 333,
                utilisateur: 333
            },
            "{texte}"
        );
    }
    // Un parent aussi : `/start` ne devient pas une commande de l'agent.
    assert!(matches!(
        trier(&update_texte(111, "/start", "private"), &parents()),
        Tri::Identifiant {
            utilisateur: 111,
            ..
        }
    ));
    assert_eq!(
        trier(&update_texte(333, "/start", "group"), &parents()),
        Tri::Ignorer
    );
    assert_eq!(
        trier(&update_texte(333, "/starter", "private"), &parents()),
        Tri::Ignorer
    );
}

#[test]
fn photo_d_un_parent_triee_avec_sa_plus_grande_taille() {
    match trier(
        &update_photo(111, Some("5 exercice 7"), "private"),
        &parents(),
    ) {
        Tri::Photo {
            expediteur,
            legende,
            fichier,
        } => {
            assert_eq!(
                (expediteur, legende.as_str(), fichier.0.as_str()),
                (111, "5 exercice 7", "grande")
            );
        }
        autre => panic!("une photo était attendue, pas {autre:?}"),
    }
    assert!(matches!(
        trier(&update_photo(222, None, "private"), &parents()),
        Tri::Photo { legende, .. } if legende.is_empty()
    ));
}

#[test]
fn photo_d_un_inconnu_ou_d_un_groupe_ignoree() {
    assert_eq!(
        trier(&update_photo(333, Some("3"), "private"), &parents()),
        Tri::Ignorer
    );
    assert_eq!(
        trier(&update_photo(111, Some("3"), "group"), &parents()),
        Tri::Ignorer
    );
}

#[test]
fn autres_mises_a_jour_ignorees() {
    let autocollant = lire(update(
        1,
        message(
            111,
            "private",
            json!({"location": {"latitude": 1.0, "longitude": 2.0}}),
        ),
    ));
    assert_eq!(trier(&autocollant, &parents()), Tri::Ignorer);
    let inconnue = lire(json!({"update_id": 1, "genre_futur": {"x": 1}}));
    assert_eq!(trier(&inconnue, &parents()), Tri::Ignorer);
    // Un message modifié est traité comme un message, comme avec python-telegram-bot.
    let modifie = lire(json!({
        "update_id": 1,
        "edited_message": message(111, "private", json!({"text": "/file", "edit_date": 1_762_624_900})),
    }));
    assert_eq!(
        trier(&modifie, &parents()),
        Tri::Message(MessageParent::texte(111, "/file"))
    );
}

#[test]
fn long_message_decoupe_sur_les_lignes() {
    let lignes: Vec<String> = (0..400)
        .map(|n| format!("• Double de {n} = ? (réponse {})", 2 * n))
        .collect();
    let texte = lignes.join("\n");
    let morceaux = decouper(&texte, 1000);
    assert!(morceaux.iter().all(|m| m.chars().count() <= 1000));
    assert_eq!(morceaux.join("\n"), texte);
    assert_eq!(decouper("court", TAILLE_MESSAGE), ["court"]);
    assert!(
        decouper(&"x".repeat(25), 10)
            .iter()
            .all(|m| m.chars().count() <= 10)
    );
}

/// La taille se compte en caractères : couper « é » en deux octets ferait paniquer.
#[test]
fn decoupe_en_caracteres_pas_en_octets() {
    let morceaux = decouper(&"é".repeat(25), 10);
    assert_eq!(
        morceaux
            .iter()
            .map(|m| m.chars().count())
            .collect::<Vec<_>>(),
        [10, 10, 5]
    );
    let accents = format!("{}\n{}", "à".repeat(8), "ç".repeat(8));
    assert_eq!(decouper(&accents, 10), ["à".repeat(8), "ç".repeat(8)]);
    assert_eq!(decouper(&accents, 17), std::slice::from_ref(&accents));
}

/// Un faux serveur de l'API Bot : rend les mises à jour qu'on lui a données, puis plus rien,
/// et garde les appels reçus (méthode en minuscules, corps JSON).
struct FauxTelegram {
    url: String,
    appels: Arc<Mutex<Vec<(String, Value)>>>,
}

#[derive(Clone, Default)]
struct Reglages {
    updates: Vec<Value>,
    jeton_refuse: bool,
    fichier_introuvable: bool,
    envoi_refuse: bool,
}

fn repondre_a(
    methode: &str,
    corps: &Value,
    reglages: &Reglages,
    deja_servi: &mut bool,
) -> (u16, Vec<u8>) {
    let ok = |resultat: Value| {
        (
            200,
            json!({"ok": true, "result": resultat})
                .to_string()
                .into_bytes(),
        )
    };
    let refus = |code: u16, raison: &str| {
        (
            code,
            json!({"ok": false, "error_code": code, "description": raison})
                .to_string()
                .into_bytes(),
        )
    };
    match methode {
        "getme" if reglages.jeton_refuse => refus(401, "Unauthorized"),
        "getme" => ok(json!({
            "id": 1, "is_bot": true, "first_name": "Essai", "username": "essai_bot",
            "can_join_groups": false, "can_read_all_group_messages": false,
            "supports_inline_queries": false, "has_main_web_app": false,
        })),
        "getupdates" => {
            if *deja_servi {
                std::thread::sleep(std::time::Duration::from_millis(30));
                return ok(json!([]));
            }
            *deja_servi = true;
            ok(json!(reglages.updates))
        }
        "sendmessage" if reglages.envoi_refuse => {
            refus(403, "Forbidden: bot was blocked by the user")
        }
        "sendmessage" => ok(json!({
            "message_id": 99, "date": 1_762_624_800,
            "chat": {"id": corps["chat_id"], "type": "private", "first_name": "P"},
            "text": corps["text"],
        })),
        "senddocument" if reglages.envoi_refuse => {
            refus(403, "Forbidden: bot was blocked by the user")
        }
        "senddocument" => ok(json!({
            "message_id": 100, "date": 1_762_624_800,
            "chat": {"id": 111, "type": "private", "first_name": "P"},
            "document": {"file_id": "d1", "file_unique_id": "u3", "file_name": "dictee-apercu.ogg"},
        })),
        "getfile" if reglages.fichier_introuvable => refus(400, "Bad Request: file not found"),
        "getfile" => ok(json!({
            "file_id": corps["file_id"], "file_unique_id": "u2", "file_size": 7,
            "file_path": "photos/fiche.jpg",
        })),
        _ => refus(404, "Not Found"),
    }
}

impl FauxTelegram {
    fn nouveau(reglages: Reglages) -> Self {
        let ecoute = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let url = format!("http://127.0.0.1:{}", ecoute.local_addr().unwrap().port());
        let appels = Arc::new(Mutex::new(Vec::new()));
        let recus = Arc::clone(&appels);
        std::thread::spawn(move || {
            let mut deja_servi = false;
            for connexion in ecoute.incoming() {
                let Ok(mut connexion) = connexion else { return };
                let mut lecteur = BufReader::new(connexion.try_clone().unwrap());
                let mut ligne = String::new();
                lecteur.read_line(&mut ligne).unwrap();
                let chemin = ligne.split(' ').nth(1).unwrap_or_default().to_owned();
                let mut longueur = 0;
                loop {
                    let mut entete = String::new();
                    lecteur.read_line(&mut entete).unwrap();
                    let Some((nom, valeur)) = entete.trim_end().split_once(": ") else {
                        break;
                    };
                    if nom.eq_ignore_ascii_case("content-length") {
                        longueur = valeur.parse().unwrap();
                    }
                }
                let mut brut = vec![0; longueur];
                lecteur.read_exact(&mut brut).unwrap();
                let (statut, reponse) = if chemin.starts_with("/file/") {
                    recus
                        .lock()
                        .unwrap()
                        .push(("fichier".to_owned(), json!(chemin)));
                    (200, b"\xff\xd8fiche".to_vec())
                } else {
                    let methode = chemin.rsplit('/').next().unwrap_or_default().to_lowercase();
                    // Un envoi de fichier n'est pas du JSON : son corps est gardé tel quel.
                    let corps: Value = serde_json::from_slice(&brut)
                        .unwrap_or_else(|_| json!(String::from_utf8_lossy(&brut)));
                    recus.lock().unwrap().push((methode.clone(), corps.clone()));
                    repondre_a(&methode, &corps, &reglages, &mut deja_servi)
                };
                let _ = write!(
                    connexion,
                    "HTTP/1.1 {statut} X\r\ncontent-type: application/json\r\n\
                     content-length: {}\r\nconnection: close\r\n\r\n",
                    reponse.len()
                );
                let _ = connexion.write_all(&reponse);
            }
        });
        Self { url, appels }
    }

    fn canal(&self) -> Telegram {
        let bot = Bot::new("123:jeton").set_api_url(self.url.parse().unwrap());
        Telegram::avec_bot(bot, &PARENTS)
    }

    fn appels(&self, methode: &str) -> Vec<Value> {
        let appels = self.appels.lock().unwrap();
        appels
            .iter()
            .filter(|(m, _)| m == methode)
            .map(|(_, corps)| corps.clone())
            .collect()
    }

    /// Les messages envoyés : destinataire et texte.
    fn envoyes(&self) -> Vec<(i64, String)> {
        self.appels("sendmessage")
            .iter()
            .map(|c| {
                (
                    c["chat_id"].as_i64().unwrap(),
                    c["text"].as_str().unwrap().to_owned(),
                )
            })
            .collect()
    }
}

#[tokio::test]
async fn le_jeton_est_verifie_au_demarrage() {
    assert!(
        FauxTelegram::nouveau(Reglages::default())
            .canal()
            .demarrer()
            .await
            .is_ok()
    );
    let refuse = FauxTelegram::nouveau(Reglages {
        jeton_refuse: true,
        ..Reglages::default()
    });
    let erreur = refuse.canal().demarrer().await.unwrap_err();
    assert!(erreur.to_string().contains("jeton"));
    // Le jeton ne s'affiche jamais, même dans un message d'erreur.
    assert!(!erreur.to_string().contains("123:jeton"));
}

#[tokio::test]
async fn message_d_un_inconnu_n_entre_pas_dans_la_file() {
    let serveur = FauxTelegram::nouveau(Reglages {
        updates: vec![
            update(
                10,
                message(333, "private", json!({"text": "/coffre 1 + 1"})),
            ),
            update(11, message(111, "group", json!({"text": "/coffre 2 + 2"}))),
            update(12, message(333, "private", json!({"text": "/start"}))),
            update(13, message(222, "private", json!({"text": "/file"}))),
        ],
        ..Reglages::default()
    });
    let canal = serveur.canal();
    assert_eq!(
        canal.recevoir().await,
        Some(MessageParent::texte(222, "/file"))
    );
    // `/start` : son identifiant à l'inconnu, et rien d'autre à personne.
    assert_eq!(
        serveur.envoyes(),
        [(333, "Ton identifiant Telegram : 333".to_owned())]
    );
    // La demande suivante part du numéro qui suit la dernière mise à jour traitée.
    canal.interroger().await;
    let demandes = serveur.appels("getupdates");
    assert_eq!(demandes[0]["offset"], 0);
    assert_eq!(demandes[1]["offset"], 14);
}

#[tokio::test]
async fn photo_d_un_parent_telechargee_en_memoire() {
    let serveur = FauxTelegram::nouveau(Reglages {
        updates: vec![
            update(1, photo(333, Some("3"), "private")),
            update(2, photo(111, Some("3"), "group")),
            update(3, photo(111, Some("5 exercice 7"), "private")),
            update(4, photo(222, None, "private")),
        ],
        ..Reglages::default()
    });
    let canal = serveur.canal();
    let recu = canal.recevoir().await.unwrap();
    assert_eq!(
        recu,
        MessageParent {
            expediteur: 111,
            texte: "5 exercice 7".to_owned(),
            image: Some(b"\xff\xd8fiche".to_vec()),
        }
    );
    assert_eq!(canal.recevoir().await.unwrap().texte, "");
    // Seules les photos des parents sont demandées à Telegram, dans leur plus grande taille.
    let demandes = serveur.appels("getfile");
    assert_eq!(demandes.len(), 2);
    assert!(demandes.iter().all(|d| d["file_id"] == "grande"));
    assert_eq!(serveur.appels("fichier").len(), 2);
    assert!(serveur.envoyes().is_empty());
}

#[tokio::test]
async fn photo_non_telechargee_previent_le_parent() {
    let serveur = FauxTelegram::nouveau(Reglages {
        updates: vec![update(1, photo(111, Some("3"), "private"))],
        fichier_introuvable: true,
        ..Reglages::default()
    });
    let canal = serveur.canal();
    canal.interroger().await;
    let envoyes = serveur.envoyes();
    assert_eq!(envoyes.len(), 1);
    assert_eq!(
        envoyes[0],
        (
            111,
            "Je n'ai pas pu récupérer la photo, réessayer.".to_owned()
        )
    );
    assert!(serveur.appels("fichier").is_empty());
}

#[tokio::test]
async fn repondre_et_envoyer_ne_parlent_qu_aux_parents() {
    let serveur = FauxTelegram::nouveau(Reglages::default());
    let canal = serveur.canal();
    canal.repondre(333, "pour un inconnu").await;
    assert!(serveur.envoyes().is_empty());
    canal.repondre(222, "août, ça marche").await;
    canal.envoyer("Coffre n°1 posé").await;
    assert_eq!(
        serveur.envoyes(),
        [
            (222, "août, ça marche".to_owned()),
            (111, "Coffre n°1 posé".to_owned()),
            (222, "Coffre n°1 posé".to_owned()),
        ]
    );
}

#[tokio::test]
async fn un_long_message_part_en_plusieurs_morceaux() {
    let serveur = FauxTelegram::nouveau(Reglages::default());
    let lignes: Vec<String> = (0..300).map(|n| format!("• ligne {n} : éàç")).collect();
    let texte = lignes.join("\n");
    serveur.canal().repondre(111, &texte).await;
    let envoyes = serveur.envoyes();
    assert!(envoyes.len() > 1);
    assert!(
        envoyes
            .iter()
            .all(|(_, m)| m.chars().count() <= TAILLE_MESSAGE)
    );
    assert_eq!(
        envoyes
            .iter()
            .map(|(_, m)| m.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        texte
    );
}

/// Bot bloqué par le parent : le message est perdu, l'agent continue.
#[tokio::test]
async fn un_envoi_refuse_ne_fait_pas_tomber_l_agent() {
    let serveur = FauxTelegram::nouveau(Reglages {
        envoi_refuse: true,
        ..Reglages::default()
    });
    mine_de_rien::journal::capturer();
    serveur.canal().envoyer("Coffre n°1 posé").await;
    let journalise = mine_de_rien::journal::captures();
    assert_eq!(serveur.appels("sendmessage").len(), 2);
    assert_eq!(journalise.len(), 2);
    assert!(
        journalise
            .iter()
            .all(|l| l.contains("message Telegram non remis"))
    );
}

/// Un jeu qui compte les coffres retirés (`Items set value []`) et peut refuser de répondre.
#[derive(Default)]
struct JeuCompteur {
    retraits: Cell<usize>,
    coupe: Cell<bool>,
}

#[async_trait(?Send)]
impl AdaptateurJeu for JeuCompteur {
    async fn commande(&self, c: &str) -> Result<String, ErreurRcon> {
        if self.coupe.get() {
            return Err(ErreurRcon("RCON coupé".to_owned()));
        }
        if c.starts_with("data get storage") {
            return Ok(
                "Storage mine_de_rien:atelier has the following contents: [0, -60, 8]".to_owned(),
            );
        }
        if c.ends_with("Items set value []") {
            self.retraits.set(self.retraits.get() + 1);
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

struct Monde {
    _dossier: tempfile::TempDir,
    depot: Rc<Depot>,
    jeu: Rc<JeuCompteur>,
    c: CommandesParents,
}

fn monde() -> Monde {
    let dossier = tempfile::tempdir().unwrap();
    let depot = Rc::new(Depot::ouvrir(&dossier.path().join("mdr.sqlite")).unwrap());
    let jeu = Rc::new(JeuCompteur::default());
    let coffre = Rc::new(CoffreDuSoir::new(jeu.clone(), "Steve", 3, None).unwrap());
    let c = CommandesParents::new(
        depot.clone(),
        ConfigCoffre::default(),
        Some(coffre),
        &PARENTS,
        Box::new(mine_de_rien::storage::sqlite::maintenant),
        None,
        Alea::graine(0),
    );
    Monde {
        _dossier: dossier,
        depot,
        jeu,
        c,
    }
}

async fn traiter(m: &Monde, expediteur: i64, texte: &str) -> Option<String> {
    m.c.traiter(&MessageParent::texte(expediteur, texte))
        .await
        .unwrap()
}

async fn dit(m: &Monde, texte: &str) -> String {
    traiter(m, 111, texte).await.unwrap()
}

fn poser(m: &Monde, ident: i64) {
    m.depot
        .marquer_pose(ident, mine_de_rien::storage::sqlite::maintenant())
        .unwrap();
}

#[tokio::test]
async fn inconnu_ne_produit_aucun_exercice() {
    let m = monde();
    assert_eq!(traiter(&m, 333, "/coffre 1 + 1").await, None);
    assert_eq!(traiter(&m, 333, "/rapport").await, None);
    assert!(m.depot.en_file().unwrap().is_empty());
}

#[tokio::test]
async fn coffre_file_annuler() {
    let m = monde();
    assert_eq!(
        dit(&m, "/coffre 348 ÷ 12").await,
        "Coffre n°1 en file : 348 ÷ 12 = ? (réponse 29)"
    );
    assert!(
        traiter(&m, 222, "/coffre@MonBot 12 x 7")
            .await
            .unwrap()
            .starts_with("Coffre n°2 en file")
    );
    let premier = m.depot.lire(1).unwrap().unwrap();
    assert_eq!(
        (premier.demande_par.as_str(), premier.recompense.as_str()),
        ("111", "minecraft:diamond")
    );

    poser(&m, 1);
    let file = dit(&m, "/file").await;
    assert!(file.contains("Coffre actif : n°1 348 ÷ 12 = ?") && file.contains("• n°2 12 × 7 = ?"));

    assert_eq!(dit(&m, "/annuler 2").await, "Coffre n°2 annulé.");
    assert_eq!(m.jeu.retraits.get(), 0);
    assert_eq!(dit(&m, "/annuler 1").await, "Coffre n°1 annulé.");
    assert_eq!(m.jeu.retraits.get(), 1);
    assert!(
        dit(&m, "/annuler 1")
            .await
            .contains("ni actif ni en attente")
    );
    assert!(dit(&m, "/annuler").await.contains("numéro"));
}

#[tokio::test]
async fn annuler_tout() {
    let m = monde();
    assert!(dit(&m, "/annuler tout").await.starts_with("Rien à annuler"));
    for calcul in ["1 + 1", "2 + 2", "3 + 3"] {
        dit(&m, &format!("/coffre {calcul}")).await;
    }
    poser(&m, 1);
    assert_eq!(
        dit(&m, "/annuler Tout").await,
        "Coffre actif n°1 annulé.\nEn attente annulés : n°2, n°3"
    );
    assert_eq!(m.jeu.retraits.get(), 1);
    assert!(m.depot.actif().unwrap().is_none() && m.depot.en_file().unwrap().is_empty());
    assert!(dit(&m, "/annuler tout").await.starts_with("Rien à annuler"));
}

#[tokio::test]
async fn annuler_tout_garde_l_actif_si_le_jeu_refuse() {
    let m = monde();
    dit(&m, "/coffre 1 + 1").await;
    dit(&m, "/coffre 2 + 2").await;
    poser(&m, 1);
    m.jeu.coupe.set(true);
    let reponse = dit(&m, "/annuler tout").await;
    assert!(
        reponse.contains("En attente annulés : n°2") && reponse.contains("n'a pas pu être retiré")
    );
    assert_eq!(m.depot.actif().unwrap().unwrap().id, 1);
    assert!(m.depot.en_file().unwrap().is_empty());
}

#[tokio::test]
async fn saisie_refusee_explique() {
    let m = monde();
    assert!(dit(&m, "/coffre 10 ÷ 3").await.starts_with("Refusé : "));
    assert!(m.depot.en_file().unwrap().is_empty());
}

#[tokio::test]
async fn message_inconnu_donne_l_aide() {
    let m = monde();
    assert!(dit(&m, "bonjour").await.contains("/coffre"));
}

/// De Telegram jusqu'à la file : une mise à jour au format de l'API devient un coffre, et la
/// réponse repart vers le bon parent.
#[tokio::test]
async fn de_telegram_a_la_file() {
    let serveur = FauxTelegram::nouveau(Reglages {
        updates: vec![
            update(1, message(333, "private", json!({"text": "/coffre 9 x 9"}))),
            update(
                2,
                message(222, "private", json!({"text": "/coffre 348 ÷ 12"})),
            ),
        ],
        ..Reglages::default()
    });
    let canal = serveur.canal();
    let m = monde();
    let recu = canal.recevoir().await.unwrap();
    let reponse = m.c.traiter(&recu).await.unwrap().unwrap();
    canal.repondre(recu.expediteur, &reponse).await;
    assert_eq!(
        serveur.envoyes(),
        [(
            222,
            "Coffre n°1 en file : 348 ÷ 12 = ? (réponse 29)".to_owned()
        )]
    );
    assert_eq!(m.depot.en_file().unwrap().len(), 1);
}

#[tokio::test]
async fn l_apercu_audio_part_en_un_seul_envoi_au_bon_parent() {
    let serveur = FauxTelegram::nouveau(Reglages::default());
    let canal = serveur.canal();
    canal
        .envoyer_audio(333, "dictee-apercu.ogg", b"OggS pour un inconnu")
        .await;
    assert!(serveur.appels("senddocument").is_empty());

    canal
        .envoyer_audio(222, "dictee-apercu.ogg", b"OggS-le-son-de-la-dictee")
        .await;
    let envois = serveur.appels("senddocument");
    assert_eq!(envois.len(), 1);
    let corps = envois[0].as_str().unwrap();
    assert!(corps.contains("name=\"chat_id\""), "{corps}");
    assert!(corps.contains("\r\n\r\n222\r\n"), "{corps}");
    assert!(corps.contains("filename=\"dictee-apercu.ogg\""), "{corps}");
    assert!(corps.contains("OggS-le-son-de-la-dictee"), "{corps}");
    // Aucun message texte ne l'accompagne : c'est l'orchestrateur qui écrit la réponse.
    assert!(serveur.envoyes().is_empty());
}

#[tokio::test]
async fn un_apercu_refuse_ne_fait_pas_tomber_l_agent() {
    let serveur = FauxTelegram::nouveau(Reglages {
        envoi_refuse: true,
        ..Reglages::default()
    });
    serveur
        .canal()
        .envoyer_audio(111, "dictee-apercu.ogg", b"OggS")
        .await;
    assert_eq!(serveur.appels("senddocument").len(), 1);
}
