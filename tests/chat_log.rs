//! Chat lu dans latest.log : analyse des lignes et suivi du fichier, rotation comprise.

mod commun;

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::time::Duration;

use chrono::{DateTime, Utc};
use mine_de_rien::adapters::game::base::{AdaptateurJeu, MessageChat};
use mine_de_rien::adapters::game::java_rcon::{DELAI, JavaRcon};
use mine_de_rien::adapters::game::log::{FluxChat, SuiviLog, analyser};

fn jour() -> DateTime<Utc> {
    commun::instant("2026-11-08T20:00:00+00:00")
}

const OBSERVEE: &str = "[18:45:26] [Async Chat Thread - #0/INFO]: [Not Secure] <seb> salut";

#[test]
fn ligne_observee_sur_le_serveur_d_essai() {
    // Daté de la réception : l'heure du log est celle du conteneur (UTC).
    let attendu = MessageChat {
        horodatage: jour(),
        joueur: "seb".to_owned(),
        texte: "salut".to_owned(),
    };
    assert_eq!(analyser(OBSERVEE, jour()), Some(attendu));
}

#[test]
fn messages_reconnus() {
    for (ligne, joueur, texte) in [
        (
            "[09:01:02] [Server thread/INFO]: <Steve_42> Pixel, aide-moi\n",
            "Steve_42",
            "Pixel, aide-moi",
        ),
        (
            "[09:01:02] [Async Chat Thread - #3/INFO]: <seb> <autre> bonjour",
            "seb",
            "<autre> bonjour",
        ),
        (
            "[09:01:02] [Async Chat Thread - #0/INFO]: [Not Secure] <seb> août, ça marche\r\n",
            "seb",
            "août, ça marche",
        ),
    ] {
        let message = analyser(ligne, jour()).unwrap();
        assert_eq!(
            (message.joueur.as_str(), message.texte.as_str()),
            (joueur, texte)
        );
    }
}

#[test]
fn autres_lignes_ignorees() {
    for ligne in [
        "[09:01:02] [Server thread/INFO]: [seb] dit par /say",
        "[09:01:02] [Server thread/INFO]: seb joined the game",
        "[09:01:02] [Server thread/INFO]: [Rcon] <seb> faux",
        "[09:01:02] [Server thread/WARN]: <seb> avertissement",
        "[09:01:02] [Server thread/INFO]: <a> pseudo trop court",
        "<seb> sans horodatage",
        "",
    ] {
        assert!(analyser(ligne, jour()).is_none(), "{ligne}");
    }
}

fn ajouter(chemin: &Path, lignes: &[&str]) {
    let mut fichier = OpenOptions::new()
        .create(true)
        .append(true)
        .open(chemin)
        .unwrap();
    for ligne in lignes {
        writeln!(fichier, "{ligne}").unwrap();
    }
}

#[test]
fn ne_rejoue_pas_ce_qui_est_deja_ecrit() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("latest.log");
    ajouter(&log, &["ancien"]);
    let mut suivi = SuiviLog::new(&log);
    assert!(suivi.lire().unwrap().is_empty());
    ajouter(&log, &["nouveau"]);
    assert_eq!(suivi.lire().unwrap(), ["nouveau"]);
    assert!(suivi.lire().unwrap().is_empty());
}

#[test]
fn ligne_incomplete_attend_sa_fin() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("latest.log");
    fs::write(&log, "").unwrap();
    let mut suivi = SuiviLog::new(&log);
    suivi.lire().unwrap();
    OpenOptions::new()
        .append(true)
        .open(&log)
        .unwrap()
        .write_all("début".as_bytes())
        .unwrap();
    assert!(suivi.lire().unwrap().is_empty());
    ajouter(&log, &[" et fin"]);
    assert_eq!(suivi.lire().unwrap(), ["début et fin"]);
}

#[test]
fn rotation_fichier_remplace() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("latest.log");
    ajouter(&log, &["avant"]);
    let mut suivi = SuiviLog::new(&log);
    suivi.lire().unwrap();
    fs::rename(&log, tmp.path().join("2026-11-08-1.log.gz")).unwrap();
    ajouter(&log, &["après redémarrage"]);
    assert_eq!(suivi.lire().unwrap(), ["après redémarrage"]);
}

#[test]
fn rotation_fichier_tronque() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("latest.log");
    ajouter(&log, &["une longue ligne d'avant"]);
    let mut suivi = SuiviLog::new(&log);
    suivi.lire().unwrap();
    fs::write(&log, "court\n").unwrap();
    assert_eq!(suivi.lire().unwrap(), ["court"]);
}

#[test]
fn fichier_absent_puis_cree() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("latest.log");
    let mut suivi = SuiviLog::new(&log);
    assert!(suivi.lire().unwrap().is_empty());
    ajouter(&log, &["premier message du serveur"]);
    assert_eq!(suivi.lire().unwrap(), ["premier message du serveur"]);
}

#[test]
fn octets_invalides_remplaces_sans_erreur() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("latest.log");
    let mut suivi = SuiviLog::new(&log);
    suivi.lire().unwrap();
    fs::write(&log, b"abc\xff\n").unwrap();
    assert_eq!(suivi.lire().unwrap(), ["abc\u{fffd}"]);
}

#[tokio::test]
async fn flux_des_messages() {
    let tmp = tempfile::tempdir().unwrap();
    let log = tmp.path().join("latest.log");
    ajouter(&log, &[OBSERVEE]);
    let mut flux = FluxChat::new(&log, Duration::from_millis(10), jour);
    let local = tokio::task::LocalSet::new();
    let message = local
        .run_until(async {
            let tache = tokio::task::spawn_local(async move { flux.suivant().await });
            tokio::time::sleep(Duration::from_millis(50)).await;
            ajouter(
                &log,
                &[
                    "[20:00:01] [Server thread/INFO]: seb joined the game",
                    "[20:00:02] [Async Chat Thread - #0/INFO]: [Not Secure] <seb> Pixel, aide",
                ],
            );
            tokio::time::timeout(Duration::from_secs(2), tache)
                .await
                .unwrap()
                .unwrap()
        })
        .await;
    assert_eq!(
        (message.joueur.as_str(), message.texte.as_str()),
        ("seb", "Pixel, aide")
    );
    assert_eq!(message.horodatage, jour());
}

#[test]
fn sans_chemin_de_log() {
    let erreur = JavaRcon::new("localhost", 25575, "secret", "", DELAI)
        .chat()
        .err()
        .unwrap();
    assert!(erreur.to_string().contains("log"));
}
