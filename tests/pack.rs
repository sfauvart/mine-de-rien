//! Le pack de sons des dictées : une archive que Minecraft sait lire, sans un mot dedans.

use std::collections::BTreeMap;
use std::io::{Cursor, Read};

use mine_de_rien::pack::{FORMAT, assurer, chemin, construire, ecrire, evenement};
use serde_json::Value;

const OGG_A: &[u8] = b"OggS\0premier son, qui n'est pas du vrai Vorbis";
const OGG_B: &[u8] = b"OggS\0second son";

fn sons() -> BTreeMap<u32, Vec<u8>> {
    BTreeMap::from([(3, OGG_A.to_vec()), (12, OGG_B.to_vec())])
}

/// Le contenu de l'archive, relu par une bibliothèque zip indépendante de notre écriture.
fn relire(archive: &[u8]) -> BTreeMap<String, Vec<u8>> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive)).unwrap();
    let mut fichiers = BTreeMap::new();
    for i in 0..zip.len() {
        let mut fichier = zip.by_index(i).unwrap();
        let mut contenu = Vec::new();
        // La lecture vérifie la somme de contrôle de chaque fichier.
        fichier.read_to_end(&mut contenu).unwrap();
        fichiers.insert(fichier.name().to_owned(), contenu);
    }
    fichiers
}

#[test]
fn le_pack_contient_les_trois_sortes_de_fichiers() {
    let fichiers = relire(&construire(&sons()).unwrap());
    assert_eq!(
        fichiers.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "assets/mine_de_rien/sounds.json",
            "assets/mine_de_rien/sounds/dictee/12.ogg",
            "assets/mine_de_rien/sounds/dictee/3.ogg",
            "pack.mcmeta",
        ]
    );
    assert_eq!(fichiers["assets/mine_de_rien/sounds/dictee/3.ogg"], OGG_A);
    assert_eq!(fichiers["assets/mine_de_rien/sounds/dictee/12.ogg"], OGG_B);
}

#[test]
fn pack_mcmeta_annonce_le_format_de_la_version() {
    let fichiers = relire(&construire(&sons()).unwrap());
    let meta: Value = serde_json::from_slice(&fichiers["pack.mcmeta"]).unwrap();
    assert_eq!(meta["pack"]["min_format"], FORMAT);
    assert_eq!(meta["pack"]["max_format"], FORMAT);
    assert!(meta["pack"]["description"].is_string());
    assert_eq!(FORMAT, 97); // Minecraft Java 26.3
}

#[test]
fn sounds_json_declare_un_evenement_par_son() {
    let fichiers = relire(&construire(&sons()).unwrap());
    let index: Value =
        serde_json::from_slice(&fichiers["assets/mine_de_rien/sounds.json"]).unwrap();
    let evenements = index.as_object().unwrap();
    assert_eq!(evenements.len(), 2);
    for numero in [3, 12] {
        let son = &evenements[&format!("dictee.{numero}")]["sounds"][0];
        assert_eq!(son["name"], format!("mine_de_rien:dictee/{numero}"));
        assert_eq!(son["stream"], true);
    }
    assert_eq!(evenement(3), "mine_de_rien:dictee.3");
}

#[test]
fn aucun_nom_de_son_ne_porte_autre_chose_qu_un_numero() {
    let archive = construire(&sons()).unwrap();
    for nom in relire(&archive).keys() {
        if let Some(fichier) = nom.strip_prefix("assets/mine_de_rien/sounds/dictee/") {
            let numero = fichier.strip_suffix(".ogg").unwrap();
            assert!(numero.chars().all(|c| c.is_ascii_digit()), "{nom}");
        }
    }
    // L'index ne donne ni sous-titre ni texte : rien que l'enfant puisse lire.
    let fichiers = relire(&archive);
    let index = String::from_utf8(fichiers["assets/mine_de_rien/sounds.json"].clone()).unwrap();
    assert!(!index.contains("subtitle"));
}

#[test]
fn sans_son_le_pack_est_valide_et_vide() {
    let fichiers = relire(&construire(&BTreeMap::new()).unwrap());
    assert_eq!(fichiers.len(), 2);
    let index: Value =
        serde_json::from_slice(&fichiers["assets/mine_de_rien/sounds.json"]).unwrap();
    assert_eq!(index, serde_json::json!({}));
}

#[test]
fn memes_sons_meme_pack() {
    assert_eq!(construire(&sons()).unwrap(), construire(&sons()).unwrap());
}

#[test]
fn un_son_qui_n_est_pas_de_l_ogg_est_refuse() {
    let sons = BTreeMap::from([(1, b"RIFF....WAVE".to_vec())]);
    let erreur = construire(&sons).unwrap_err();
    assert_eq!(erreur.to_string(), "Le son n°1 n'est pas un fichier Ogg.");
}

#[test]
fn le_pack_vit_a_cote_de_la_base() {
    use std::path::Path;
    assert_eq!(
        chemin(Path::new("/data/mine-de-rien.sqlite")),
        Path::new("/data/mine-de-rien.zip")
    );
    assert_eq!(
        chemin(Path::new("base.sqlite")),
        Path::new("./mine-de-rien.zip")
    );
}

#[test]
fn l_ecriture_remplace_le_pack_sans_laisser_de_fichier_temporaire() {
    let dossier = tempfile::tempdir().unwrap();
    let cible = dossier.path().join("sous-dossier").join("mine-de-rien.zip");
    let premier = construire(&BTreeMap::new()).unwrap();
    let second = construire(&sons()).unwrap();

    ecrire(&cible, &premier).unwrap();
    assert_eq!(std::fs::read(&cible).unwrap(), premier);
    ecrire(&cible, &second).unwrap();
    assert_eq!(std::fs::read(&cible).unwrap(), second);

    let restes: Vec<_> = std::fs::read_dir(cible.parent().unwrap())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(restes, ["mine-de-rien.zip"]);
}

#[test]
fn une_ecriture_qui_echoue_laisse_l_ancien_pack_entier() {
    let dossier = tempfile::tempdir().unwrap();
    let cible = dossier.path().join("mine-de-rien.zip");
    let ancien = construire(&sons()).unwrap();
    ecrire(&cible, &ancien).unwrap();
    // La cible devient un dossier non vide : le renommage final échoue.
    let bloquee = dossier.path().join("bloquee");
    std::fs::create_dir_all(bloquee.join("plein")).unwrap();
    assert!(ecrire(&bloquee, b"nouveau").is_err());
    assert_eq!(std::fs::read(&cible).unwrap(), ancien);
    assert!(bloquee.join("plein").is_dir());
}

#[test]
fn assurer_ecrit_un_pack_vide_une_seule_fois() {
    let dossier = tempfile::tempdir().unwrap();
    let cible = dossier.path().join("mine-de-rien.zip");
    assurer(&cible).unwrap();
    assert_eq!(relire(&std::fs::read(&cible).unwrap()).len(), 2);
    // Un pack déjà là n'est pas remplacé.
    let plein = construire(&sons()).unwrap();
    ecrire(&cible, &plein).unwrap();
    assurer(&cible).unwrap();
    assert_eq!(std::fs::read(&cible).unwrap(), plein);
}
