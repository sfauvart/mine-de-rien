mod commun;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mine_de_rien::datapack::{self, ErreurDatapack};

fn source(racine: &Path) -> PathBuf {
    let source = racine.join("source").join("mine_de_rien");
    fs::create_dir_all(source.join("data").join("f")).unwrap();
    fs::write(source.join("pack.mcmeta"), "{\"pack\": {}}").unwrap();
    fs::write(source.join("data/f/tick.mcfunction"), "say tick").unwrap();
    source
}

fn arbre(dossier: &Path) -> BTreeMap<String, String> {
    fn parcourir(racine: &Path, dossier: &Path, sortie: &mut BTreeMap<String, String>) {
        for entree in fs::read_dir(dossier).unwrap() {
            let chemin = entree.unwrap().path();
            if chemin.is_dir() {
                parcourir(racine, &chemin, sortie);
            } else {
                let relatif = chemin
                    .strip_prefix(racine)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned();
                sortie.insert(relatif, fs::read_to_string(&chemin).unwrap());
            }
        }
    }
    let mut sortie = BTreeMap::new();
    parcourir(dossier, dossier, &mut sortie);
    sortie
}

fn commande(dossier: &Path) -> Command {
    let mut commande = Command::new(env!("CARGO_BIN_EXE_mine-de-rien"));
    commande.current_dir(dossier); // aucun config.toml ici
    commande
}

#[test]
fn le_datapack_du_depot_est_la_source_par_defaut() {
    assert!(datapack::EMBARQUE.get_file("pack.mcmeta").is_some());
}

#[test]
fn le_datapack_embarque_ne_joue_plus_aucun_son() {
    // La voix passe par le chat vocal : plus de fonction de lecture, plus de playsound.
    assert!(
        datapack::EMBARQUE
            .get_file("data/mine_de_rien/function/dictee/jouer.mcfunction")
            .is_none()
    );
    let redire = datapack::EMBARQUE
        .get_file("data/mine_de_rien/function/dictee/redire.mcfunction")
        .and_then(|f| f.contents_utf8())
        .unwrap();
    assert!(!redire.contains("playsound") && !redire.contains("dictee/jouer"));
    // L'Écouteur réaffiche toujours la phrase à trou, pour le seul joueur qui a cliqué.
    assert!(redire.contains("tellraw @s"));
    assert!(redire.contains("unless data storage mine_de_rien:dictee phrase"));
}

#[test]
fn le_datapack_embarque_est_celui_du_depot() {
    let tmp = tempfile::tempdir().unwrap();
    let cible = datapack::recopier(&tmp.path().join("sortie"), None).unwrap();
    assert_eq!(
        arbre(&cible),
        arbre(&commun::racine().join("datapacks/mine_de_rien"))
    );
}

#[test]
fn recopie_toute_l_arborescence() {
    let tmp = tempfile::tempdir().unwrap();
    let source = source(tmp.path());
    let cible = datapack::recopier(&tmp.path().join("sortie"), Some(&source)).unwrap();
    assert_eq!(
        cible,
        tmp.path()
            .join("sortie")
            .canonicalize()
            .unwrap()
            .join("mine_de_rien")
    );
    assert_eq!(arbre(&cible), arbre(&source));
}

#[test]
fn un_second_appel_remplace_sans_dupliquer() {
    let tmp = tempfile::tempdir().unwrap();
    let source = source(tmp.path());
    let sortie = tmp.path().join("sortie");
    datapack::recopier(&sortie, Some(&source)).unwrap();
    fs::write(source.join("pack.mcmeta"), "{\"pack\": {\"v\": 2}}").unwrap();
    datapack::recopier(&sortie, Some(&source)).unwrap();
    let noms: Vec<_> = fs::read_dir(&sortie)
        .unwrap()
        .map(|e| e.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(noms, ["mine_de_rien"]);
    assert_eq!(arbre(&sortie.join("mine_de_rien")), arbre(&source));
}

#[test]
fn un_fichier_retire_de_la_source_disparait() {
    let tmp = tempfile::tempdir().unwrap();
    let source = source(tmp.path());
    let sortie = tmp.path().join("sortie");
    datapack::recopier(&sortie, Some(&source)).unwrap();
    fs::remove_file(source.join("data/f/tick.mcfunction")).unwrap();
    datapack::recopier(&sortie, Some(&source)).unwrap();
    assert!(!sortie.join("mine_de_rien/data/f/tick.mcfunction").exists());
}

#[test]
fn refuse_une_source_sans_pack_mcmeta() {
    let tmp = tempfile::tempdir().unwrap();
    let source = source(tmp.path());
    fs::remove_file(source.join("pack.mcmeta")).unwrap();
    let erreur = datapack::recopier(&tmp.path().join("sortie"), Some(&source)).unwrap_err();
    assert!(matches!(erreur, ErreurDatapack::Refus(_)));
    assert!(erreur.to_string().contains("pack.mcmeta"));
    assert!(!tmp.path().join("sortie/mine_de_rien").exists());
}

#[test]
fn refuse_de_s_ecraser_elle_meme() {
    let tmp = tempfile::tempdir().unwrap();
    let source = source(tmp.path());
    let erreur = datapack::recopier(source.parent().unwrap(), Some(&source)).unwrap_err();
    assert!(erreur.to_string().contains("recouvre"));
    assert!(source.join("pack.mcmeta").is_file());
}

#[test]
fn la_commande_ne_lit_aucune_config() {
    let tmp = tempfile::tempdir().unwrap();
    let source = source(tmp.path());
    let sortie = commande(tmp.path())
        .args(["datapack", "sortie", "--source"])
        .arg(&source)
        .output()
        .unwrap();
    assert!(sortie.status.success());
    assert!(tmp.path().join("sortie/mine_de_rien/pack.mcmeta").is_file());
    assert!(String::from_utf8_lossy(&sortie.stdout).contains("Datapack recopié"));
}

#[test]
fn la_commande_sans_source_ecrit_le_datapack_embarque() {
    let tmp = tempfile::tempdir().unwrap();
    let sortie = commande(tmp.path())
        .args(["datapack", "sortie"])
        .output()
        .unwrap();
    assert!(sortie.status.success());
    assert_eq!(
        arbre(&tmp.path().join("sortie/mine_de_rien")),
        arbre(&commun::racine().join("datapacks/mine_de_rien"))
    );
}

#[test]
fn la_commande_signale_une_source_invalide() {
    let tmp = tempfile::tempdir().unwrap();
    let sortie = commande(tmp.path())
        .args(["datapack", "sortie", "--source"])
        .arg(tmp.path())
        .output()
        .unwrap();
    assert_eq!(sortie.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&sortie.stderr).contains("pack.mcmeta"));
}

#[test]
fn la_version_est_celle_du_paquet() {
    let tmp = tempfile::tempdir().unwrap();
    let sortie = commande(tmp.path()).arg("--version").output().unwrap();
    assert_eq!(
        String::from_utf8_lossy(&sortie.stdout).trim(),
        format!("mine-de-rien {}", mine_de_rien::VERSION)
    );
}
