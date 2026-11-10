//! Adaptateur RCON contre un faux serveur local qui parle le protocole.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::java_rcon::JavaRcon;
use mine_de_rien::adapters::game::rcon::TAILLE_MAX_COMMANDE;
use serde_json::{Value, json};

fn paquet(ident: i32, genre: i32, corps: &[u8]) -> Vec<u8> {
    let mut charge = Vec::new();
    charge.extend_from_slice(&ident.to_le_bytes());
    charge.extend_from_slice(&genre.to_le_bytes());
    charge.extend_from_slice(corps);
    charge.extend_from_slice(&[0, 0]);
    let mut sortie = (charge.len() as i32).to_le_bytes().to_vec();
    sortie.extend_from_slice(&charge);
    sortie
}

#[derive(Default)]
struct Etat {
    reponses: Mutex<HashMap<String, String>>,
    recues: Mutex<Vec<String>>,
    connexions: AtomicUsize,
    paquets_perdus: AtomicUsize,
    /// Nombre de commandes après lesquelles le serveur ferme la connexion ; 0 : jamais.
    couper_apres: AtomicUsize,
}

/// Imite le serveur vanilla : réponses découpées en morceaux de 4096 caractères, et un
/// seul paquet lu par `read` ; ce qui suit dans le même tampon est perdu, et la connexion se
/// ferme si le paquet est incomplet.
struct FauxServeur {
    etat: Arc<Etat>,
    port: u16,
}

fn client(mut conn: TcpStream, etat: &Etat) {
    let mut commandes = 0;
    let mut tampon = [0u8; 1460];
    loop {
        let Ok(lus) = conn.read(&mut tampon) else {
            return;
        };
        if lus < 4 {
            return;
        }
        let longueur = i32::from_le_bytes(tampon[..4].try_into().unwrap()) as usize;
        if lus < 4 + longueur {
            return;
        }
        if lus > 4 + longueur {
            etat.paquets_perdus.fetch_add(1, Ordering::SeqCst);
        }
        let charge = &tampon[4..4 + longueur];
        let ident = i32::from_le_bytes(charge[..4].try_into().unwrap());
        let genre = i32::from_le_bytes(charge[4..8].try_into().unwrap());
        let corps = String::from_utf8_lossy(&charge[8..longueur - 2]).into_owned();
        let envoi = match genre {
            3 => conn.write_all(&paquet(if corps == "essai" { ident } else { -1 }, 2, b"")),
            2 => {
                etat.recues.lock().unwrap().push(corps.clone());
                let reponse = etat
                    .reponses
                    .lock()
                    .unwrap()
                    .get(&corps)
                    .cloned()
                    .unwrap_or_default();
                let caracteres: Vec<char> = reponse.chars().collect();
                let mut morceaux: Vec<String> = caracteres
                    .chunks(4096)
                    .map(|m| m.iter().collect())
                    .collect();
                if morceaux.is_empty() {
                    morceaux.push(String::new());
                }
                let mut resultat = Ok(());
                for morceau in morceaux {
                    resultat = resultat
                        .and_then(|_| conn.write_all(&paquet(ident, 0, morceau.as_bytes())));
                }
                commandes += 1;
                let limite = etat.couper_apres.load(Ordering::SeqCst);
                if limite != 0 && commandes >= limite {
                    return;
                }
                resultat
            }
            // Le vrai serveur répond « Unknown request » aux types inconnus.
            _ => conn.write_all(&paquet(ident, 0, b"Unknown request 0")),
        };
        if envoi.is_err() {
            return;
        }
    }
}

impl FauxServeur {
    fn nouveau() -> Self {
        let etat = Arc::new(Etat::default());
        etat.reponses.lock().unwrap().extend([
            (
                "list".to_owned(),
                "There are 0 of a max of 20 players online: ".to_owned(),
            ),
            ("long".to_owned(), "abcdefghij".repeat(1000) + "é"),
            ("pile".to_owned(), "x".repeat(4096)),
        ]);
        let ecoute = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let port = ecoute.local_addr().unwrap().port();
        let partage = Arc::clone(&etat);
        std::thread::spawn(move || {
            for conn in ecoute.incoming() {
                let Ok(conn) = conn else { return };
                partage.connexions.fetch_add(1, Ordering::SeqCst);
                let etat = Arc::clone(&partage);
                std::thread::spawn(move || client(conn, &etat));
            }
        });
        Self { etat, port }
    }

    fn adaptateur(&self, mot_de_passe: &str) -> JavaRcon {
        JavaRcon::new(
            "127.0.0.1",
            self.port,
            mot_de_passe,
            "",
            Duration::from_secs(2),
        )
    }

    fn recues(&self) -> Vec<String> {
        self.etat.recues.lock().unwrap().clone()
    }

    fn repondre(&self, commande: &str, reponse: &str) {
        self.etat
            .reponses
            .lock()
            .unwrap()
            .insert(commande.to_owned(), reponse.to_owned());
    }

    fn connexions(&self) -> usize {
        self.etat.connexions.load(Ordering::SeqCst)
    }

    fn paquets_perdus(&self) -> usize {
        self.etat.paquets_perdus.load(Ordering::SeqCst)
    }
}

#[tokio::test]
async fn commande_et_reponse() {
    let serveur = FauxServeur::nouveau();
    let jeu = serveur.adaptateur("essai");
    assert!(
        jeu.commande("list")
            .await
            .unwrap()
            .starts_with("There are 0")
    );
}

#[tokio::test]
async fn reponse_en_plusieurs_paquets_recollee() {
    let serveur = FauxServeur::nouveau();
    let jeu = serveur.adaptateur("essai");
    assert_eq!(
        jeu.commande("long").await.unwrap(),
        "abcdefghij".repeat(1000) + "é"
    );
    // Un morceau pile plein : la sentinelle sert à savoir qu'il n'y en a pas d'autre.
    assert_eq!(jeu.commande("pile").await.unwrap(), "x".repeat(4096));
    assert_eq!(serveur.paquets_perdus(), 0);
}

/// Régression : commande et sentinelle envoyées ensemble se collaient côté serveur.
#[tokio::test]
async fn rafale_de_commandes_sans_paquet_perdu() {
    let serveur = FauxServeur::nouveau();
    let jeu = serveur.adaptateur("essai");
    for _ in 0..200 {
        assert!(
            jeu.commande("list")
                .await
                .unwrap()
                .starts_with("There are 0")
        );
    }
    assert_eq!(serveur.paquets_perdus(), 0);
    assert_eq!(serveur.connexions(), 1);
}

#[tokio::test]
async fn mauvais_mot_de_passe() {
    let serveur = FauxServeur::nouveau();
    let erreur = serveur
        .adaptateur("faux")
        .commande("list")
        .await
        .unwrap_err();
    assert!(erreur.to_string().contains("mot de passe"));
}

#[tokio::test]
async fn serveur_injoignable() {
    let libre = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = libre.local_addr().unwrap().port();
    drop(libre);
    let jeu = JavaRcon::new("127.0.0.1", port, "essai", "", Duration::from_secs(1));
    assert!(
        jeu.commande("list")
            .await
            .unwrap_err()
            .to_string()
            .contains("injoignable")
    );
}

#[tokio::test]
async fn commandes_simultanees_ne_se_melangent_pas() {
    let serveur = FauxServeur::nouveau();
    for i in 0..20 {
        serveur.repondre(&format!("c{i}"), &format!("r{i}").repeat(20));
    }
    let jeu = Rc::new(serveur.adaptateur("essai"));
    let local = tokio::task::LocalSet::new();
    let taches: Vec<_> = (0..20)
        .map(|i| {
            let jeu = Rc::clone(&jeu);
            local.spawn_local(async move { jeu.commande(&format!("c{i}")).await.unwrap() })
        })
        .collect();
    local
        .run_until(async {
            for (i, tache) in taches.into_iter().enumerate() {
                assert_eq!(tache.await.unwrap(), format!("r{i}").repeat(20));
            }
        })
        .await;
    assert_eq!(serveur.connexions(), 1);
}

#[tokio::test]
async fn reconnexion_si_la_connexion_est_tombee() {
    let serveur = FauxServeur::nouveau();
    serveur.etat.couper_apres.store(1, Ordering::SeqCst);
    let jeu = serveur.adaptateur("essai");
    let premiere = jeu.commande("list").await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await; // le serveur a fermé entre-temps
    let seconde = jeu.commande("list").await.unwrap();
    assert_eq!(premiere, seconde);
    assert_eq!(serveur.connexions(), 2);
    // Aucune commande rejouée.
    assert_eq!(serveur.recues(), ["list", "list"]);
}

#[tokio::test]
async fn commande_trop_longue_refusee() {
    let serveur = FauxServeur::nouveau();
    let jeu = serveur.adaptateur("essai");
    let erreur = jeu
        .commande(&format!("say {}", "x".repeat(TAILLE_MAX_COMMANDE)))
        .await
        .unwrap_err();
    assert!(erreur.to_string().contains("trop longue"));
    assert!(serveur.recues().is_empty());
}

/// La limite du serveur est en octets : « é » en pèse deux.
#[tokio::test]
async fn la_limite_de_commande_se_compte_en_octets() {
    let serveur = FauxServeur::nouveau();
    let jeu = serveur.adaptateur("essai");
    let accents = "é".repeat(TAILLE_MAX_COMMANDE / 2 + 1);
    assert!(
        jeu.commande(&accents)
            .await
            .unwrap_err()
            .to_string()
            .contains("trop longue")
    );
    assert!(
        jeu.commande(&"é".repeat(TAILLE_MAX_COMMANDE / 2))
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn dire_envoie_un_tellraw_serialise() {
    let serveur = FauxServeur::nouveau();
    let jeu = serveur.adaptateur("essai");
    let message = "l'élève a dit \"bonjour\" \\ ]}";
    jeu.dire("Steve", message, &[]).await.unwrap();
    let recues = serveur.recues();
    assert_eq!(recues.len(), 1);
    let composant: Value =
        serde_json::from_str(recues[0].strip_prefix("tellraw Steve ").unwrap()).unwrap();
    assert_eq!(composant, json!({"text": message}));
}

#[tokio::test]
async fn dire_remonte_le_refus_du_serveur() {
    let serveur = FauxServeur::nouveau();
    serveur.repondre(
        "tellraw Steve {\"text\": \"coucou\"}",
        "No player was found",
    );
    let jeu = serveur.adaptateur("essai");
    let erreur = jeu.dire("Steve", "coucou", &[]).await.unwrap_err();
    assert!(erreur.to_string().contains("No player was found"));
}

#[tokio::test]
async fn dire_refuse_un_selecteur() {
    let serveur = FauxServeur::nouveau();
    let jeu = serveur.adaptateur("essai");
    assert!(jeu.dire("@a", "coucou", &[]).await.is_err());
    assert!(serveur.recues().is_empty());
}

/// Contre le vrai serveur d'essai (`/essai-en-jeu`, RCON sur 25576) : rafale de commandes
/// et reconnexion après un redémarrage. Redémarre le serveur d'essai, donc lancé à la main :
///
/// ```text
/// cargo test --test java_rcon -- --ignored --nocapture
/// ```
///
/// La réponse longue (plus de 4096 caractères, recollée grâce à la sentinelle) ne se vérifie
/// pas ici : Paper 26.3 abrège lui-même ses longues sorties (« [a], [b], ... »), que ce soit
/// `data get` ou la liste de 300 objectifs ci-dessous. Elle reste couverte par le faux serveur.
#[tokio::test]
#[ignore = "demande le serveur d'essai lancé, et le redémarre"]
async fn essai_rafale_et_reconnexion() {
    let jeu = JavaRcon::new("localhost", 25576, "essai", "", Duration::from_secs(5));
    let version = jeu.commande("version").await.unwrap();
    println!("serveur : {}", version.lines().last().unwrap_or_default());

    for n in 0..300 {
        let reponse = jeu
            .commande(&format!("scoreboard objectives add mdr_essai_{n:03} dummy"))
            .await
            .unwrap();
        assert!(
            reponse.contains("Created") || reponse.contains("already exists"),
            "{reponse}"
        );
    }
    let liste = jeu.commande("scoreboard objectives list").await.unwrap();
    println!("liste des objectifs : {} caractères", liste.chars().count());
    for n in 0..300 {
        let reponse = jeu
            .commande(&format!("scoreboard objectives remove mdr_essai_{n:03}"))
            .await
            .unwrap();
        assert!(reponse.contains("Removed"), "{reponse}");
    }
    assert!(
        liste.starts_with("There are 300 objective(s): [mdr_essai_"),
        "{liste}"
    );
    // Accents à l'aller et au retour.
    jeu.commande("data modify storage mine_de_rien:essai mot set value \"août, ça marche ÷\"")
        .await
        .unwrap();
    let mot = jeu
        .commande("data get storage mine_de_rien:essai mot")
        .await
        .unwrap();
    assert!(mot.ends_with("\"août, ça marche ÷\""), "{mot}");
    jeu.commande("data remove storage mine_de_rien:essai mot")
        .await
        .unwrap();

    let racine = env!("CARGO_MANIFEST_DIR");
    let redemarrage = std::process::Command::new("docker")
        .args([
            "compose",
            "-f",
            "docker/essai.compose.yml",
            "restart",
            "minecraft",
        ])
        .current_dir(racine)
        .status()
        .unwrap();
    assert!(redemarrage.success());
    let mut reponse = Err(mine_de_rien::adapters::game::rcon::ErreurRcon(
        "jamais essayé".into(),
    ));
    for _ in 0..60 {
        tokio::time::sleep(Duration::from_secs(3)).await;
        reponse = jeu.commande("list").await;
        if reponse.is_ok() {
            break;
        }
    }
    println!("après redémarrage : {reponse:?}");
    assert!(reponse.unwrap().contains("players online"));
}
