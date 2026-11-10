//! Petit service HTTP qui ne sert qu'un fichier : le pack de sons des dictées.
//!
//! Le jeu de l'enfant télécharge le pack à l'adresse annoncée par le serveur Minecraft, à
//! chaque connexion. Le service écoute sur le réseau de la maison, sans mot de passe : le
//! pack ne contient que des phrases de dictée lues, numérotées (`pack.rs`).
//!
//! Écrit à la main sur `tokio` : une seule adresse, `GET` ou `HEAD`, tout le reste reçoit
//! 404. Le fichier est relu à chaque requête, le pack peut donc changer sans relancer
//! l'agent. Rien n'est journalisé de qui télécharge.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

use crate::journal;
use crate::pack::NOM_FICHIER;

/// Une requête plus longue que cela n'est pas celle d'un jeu qui demande le pack.
const ENTETES_MAX: usize = 8 * 1024;
const DELAI_REQUETE: Duration = Duration::from_secs(10);
const DELAI_ENVOI: Duration = Duration::from_secs(120);

/// L'adresse du pack, telle que le jeu la demande.
pub fn adresse() -> String {
    format!("/{NOM_FICHIER}")
}

/// Ouvre le port du service. `hote` vaut « 0.0.0.0 » pour le réseau de la maison.
pub async fn ecouter(hote: &str, port: u16) -> std::io::Result<TcpListener> {
    TcpListener::bind((hote, port)).await
}

/// La première ligne d'une requête : méthode et chemin, sans les paramètres après « ? ».
fn lire_demande(entetes: &[u8]) -> Option<(&str, &str)> {
    let ligne = entetes.split(|&o| o == b'\r' || o == b'\n').next()?;
    let ligne = std::str::from_utf8(ligne).ok()?;
    let mut morceaux = ligne.split(' ');
    let methode = morceaux.next()?;
    let cible = morceaux.next()?;
    let chemin = cible.split(['?', '#']).next()?;
    Some((methode, chemin))
}

async fn lire_entetes(connexion: &mut TcpStream) -> Option<Vec<u8>> {
    let mut recu = Vec::new();
    let mut tampon = [0u8; 1024];
    while !recu.windows(4).any(|f| f == b"\r\n\r\n") {
        if recu.len() > ENTETES_MAX {
            return None;
        }
        match connexion.read(&mut tampon).await {
            Ok(0) | Err(_) => return None,
            Ok(n) => recu.extend_from_slice(&tampon[..n]),
        }
    }
    Some(recu)
}

async fn repondre(
    connexion: &mut TcpStream,
    statut: &str,
    genre: &str,
    longueur: usize,
    corps: Option<&[u8]>,
) -> std::io::Result<()> {
    let entetes = format!(
        "HTTP/1.1 {statut}\r\ncontent-type: {genre}\r\ncontent-length: {longueur}\r\n\
         cache-control: no-store\r\nconnection: close\r\n\r\n"
    );
    connexion.write_all(entetes.as_bytes()).await?;
    if let Some(corps) = corps {
        connexion.write_all(corps).await?;
    }
    connexion.shutdown().await
}

async fn traiter(mut connexion: TcpStream, pack: PathBuf) {
    let Ok(Some(entetes)) = tokio::time::timeout(DELAI_REQUETE, lire_entetes(&mut connexion)).await
    else {
        return;
    };
    let demande = lire_demande(&entetes);
    let envoi = async {
        match demande {
            Some((methode @ ("GET" | "HEAD"), chemin)) if chemin == adresse() => {
                match tokio::fs::read(&pack).await {
                    Ok(contenu) => {
                        let corps = (methode == "GET").then_some(contenu.as_slice());
                        repondre(
                            &mut connexion,
                            "200 OK",
                            "application/zip",
                            contenu.len(),
                            corps,
                        )
                        .await
                    }
                    Err(e) => {
                        journal::avertissement(format!("pack de sons illisible : {e}"));
                        repondre(&mut connexion, "404 Not Found", "text/plain", 0, None).await
                    }
                }
            }
            _ => repondre(&mut connexion, "404 Not Found", "text/plain", 0, None).await,
        }
    };
    // Un jeu qui ferme la connexion en cours de route n'est pas une erreur à signaler.
    let _ = tokio::time::timeout(DELAI_ENVOI, envoi).await;
}

/// Sert le pack jusqu'à ce que la tâche soit abandonnée : l'appelant l'arrête avec l'agent.
pub async fn servir(ecoute: TcpListener, pack: &Path) {
    loop {
        match ecoute.accept().await {
            Ok((connexion, _)) => {
                tokio::spawn(traiter(connexion, pack.to_path_buf()));
            }
            Err(e) => {
                journal::avertissement(format!("service du pack de sons : {e}"));
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
}
