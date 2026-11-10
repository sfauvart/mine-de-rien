//! Le service HTTP du pack de sons : un seul fichier, rien d'autre.

use std::path::Path;
use std::time::Duration;

use mine_de_rien::service_pack::{adresse, ecouter, servir};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Lance le service sur un port libre, pour le fichier `pack`.
async fn lancer(pack: &Path) -> (u16, tokio::task::JoinHandle<()>) {
    let ecoute = ecouter("127.0.0.1", 0).await.unwrap();
    let port = ecoute.local_addr().unwrap().port();
    let pack = pack.to_path_buf();
    let tache = tokio::spawn(async move { servir(ecoute, &pack).await });
    (port, tache)
}

/// Envoie une requête brute et rend toute la réponse.
async fn demander(port: u16, requete: &str) -> Vec<u8> {
    let mut connexion = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    connexion.write_all(requete.as_bytes()).await.unwrap();
    let mut reponse = Vec::new();
    tokio::time::timeout(Duration::from_secs(5), connexion.read_to_end(&mut reponse))
        .await
        .unwrap()
        .unwrap();
    reponse
}

fn statut(reponse: &[u8]) -> String {
    let fin = reponse.iter().position(|&o| o == b'\r').unwrap();
    String::from_utf8_lossy(&reponse[..fin]).into_owned()
}

fn corps(reponse: &[u8]) -> &[u8] {
    let fin = reponse.windows(4).position(|f| f == b"\r\n\r\n").unwrap();
    &reponse[fin + 4..]
}

fn get(chemin: &str) -> String {
    format!("GET {chemin} HTTP/1.1\r\nHost: maison\r\n\r\n")
}

#[tokio::test]
async fn sert_le_pack_a_son_adresse() {
    let dossier = tempfile::tempdir().unwrap();
    let pack = dossier.path().join("mine-de-rien.zip");
    let contenu: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
    std::fs::write(&pack, &contenu).unwrap();
    let (port, _tache) = lancer(&pack).await;

    assert_eq!(adresse(), "/mine-de-rien.zip");
    let reponse = demander(port, &get("/mine-de-rien.zip")).await;
    assert_eq!(statut(&reponse), "HTTP/1.1 200 OK");
    let entetes = String::from_utf8_lossy(&reponse[..200]).to_lowercase();
    assert!(entetes.contains("content-type: application/zip"));
    assert!(entetes.contains("content-length: 200000"));
    assert_eq!(corps(&reponse), contenu);

    // Le jeu peut ajouter des paramètres : ils sont ignorés.
    let reponse = demander(port, &get("/mine-de-rien.zip?v=2")).await;
    assert_eq!(corps(&reponse), contenu);
}

#[tokio::test]
async fn head_annonce_la_taille_sans_le_contenu() {
    let dossier = tempfile::tempdir().unwrap();
    let pack = dossier.path().join("mine-de-rien.zip");
    std::fs::write(&pack, b"PK-contenu").unwrap();
    let (port, _tache) = lancer(&pack).await;
    let reponse = demander(port, "HEAD /mine-de-rien.zip HTTP/1.1\r\n\r\n").await;
    assert_eq!(statut(&reponse), "HTTP/1.1 200 OK");
    assert!(String::from_utf8_lossy(&reponse).contains("content-length: 10"));
    assert!(corps(&reponse).is_empty());
}

#[tokio::test]
async fn toute_autre_demande_recoit_404() {
    let dossier = tempfile::tempdir().unwrap();
    let pack = dossier.path().join("mine-de-rien.zip");
    std::fs::write(&pack, b"PK-contenu").unwrap();
    std::fs::write(dossier.path().join("mine-de-rien.sqlite"), b"la base").unwrap();
    let (port, _tache) = lancer(&pack).await;

    for requete in [
        get("/"),
        get("/mine-de-rien.sqlite"),
        get("/../mine-de-rien.sqlite"),
        get("/mine-de-rien.zip/../mine-de-rien.sqlite"),
        get("/mine-de-rien.zipx"),
        get("//mine-de-rien.zip"),
        "POST /mine-de-rien.zip HTTP/1.1\r\ncontent-length: 0\r\n\r\n".to_owned(),
        "DELETE /mine-de-rien.zip HTTP/1.1\r\n\r\n".to_owned(),
        "n'importe quoi\r\n\r\n".to_owned(),
    ] {
        let reponse = demander(port, &requete).await;
        assert_eq!(statut(&reponse), "HTTP/1.1 404 Not Found", "{requete}");
        assert!(corps(&reponse).is_empty(), "{requete}");
    }
}

#[tokio::test]
async fn le_pack_remplace_est_servi_sans_relancer() {
    let dossier = tempfile::tempdir().unwrap();
    let pack = dossier.path().join("mine-de-rien.zip");
    std::fs::write(&pack, b"premier").unwrap();
    let (port, _tache) = lancer(&pack).await;
    assert_eq!(
        corps(&demander(port, &get("/mine-de-rien.zip")).await),
        b"premier"
    );
    mine_de_rien::pack::ecrire(&pack, b"second pack").unwrap();
    assert_eq!(
        corps(&demander(port, &get("/mine-de-rien.zip")).await),
        b"second pack"
    );
}

#[tokio::test]
async fn pack_absent_404_et_le_service_continue() {
    let dossier = tempfile::tempdir().unwrap();
    let pack = dossier.path().join("mine-de-rien.zip");
    let (port, _tache) = lancer(&pack).await;
    assert_eq!(
        statut(&demander(port, &get("/mine-de-rien.zip")).await),
        "HTTP/1.1 404 Not Found"
    );
    std::fs::write(&pack, b"arrive").unwrap();
    assert_eq!(
        corps(&demander(port, &get("/mine-de-rien.zip")).await),
        b"arrive"
    );
}

#[tokio::test]
async fn requete_demesuree_ou_muette_ne_bloque_pas_les_autres() {
    let dossier = tempfile::tempdir().unwrap();
    let pack = dossier.path().join("mine-de-rien.zip");
    std::fs::write(&pack, b"pack").unwrap();
    let (port, _tache) = lancer(&pack).await;

    // Une connexion qui ne dit rien reste ouverte pendant qu'une autre est servie.
    let _muette = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    assert_eq!(
        corps(&demander(port, &get("/mine-de-rien.zip")).await),
        b"pack"
    );

    // Des en-têtes sans fin : la connexion est fermée sans réponse.
    let mut bavarde = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
    let ligne = format!(
        "GET /mine-de-rien.zip HTTP/1.1\r\nx: {}\r\n",
        "a".repeat(1000)
    );
    let mut ferme = false;
    for _ in 0..50 {
        if bavarde.write_all(ligne.as_bytes()).await.is_err() {
            ferme = true;
            break;
        }
    }
    let mut reste = Vec::new();
    let lu = tokio::time::timeout(Duration::from_secs(5), bavarde.read_to_end(&mut reste)).await;
    assert!(
        ferme || lu.is_ok(),
        "la connexion bavarde aurait dû être fermée"
    );
    assert!(!reste.starts_with(b"HTTP/1.1 200"));
}

#[tokio::test]
async fn le_service_rend_la_main_a_l_arret() {
    let dossier = tempfile::tempdir().unwrap();
    let pack = dossier.path().join("mine-de-rien.zip");
    std::fs::write(&pack, b"pack").unwrap();
    let (port, tache) = lancer(&pack).await;
    assert_eq!(
        corps(&demander(port, &get("/mine-de-rien.zip")).await),
        b"pack"
    );
    tache.abort();
    assert!(tache.await.unwrap_err().is_cancelled());
    // Le port est libéré : plus personne n'y répond.
    assert!(TcpStream::connect(("127.0.0.1", port)).await.is_err());
}
