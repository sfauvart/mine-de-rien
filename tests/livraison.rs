//! Ce que l'agent livre au serveur avant son démarrage : le plugin et les réglages du chat vocal.

mod commun;

use mine_de_rien::livraison::{NOM_DU_PLUGIN, REGLAGES_DU_CHAT_VOCAL, recopier_plugin};

/// Le début d'une archive zip : ce à quoi se reconnaît un .jar.
const JAR: &[u8] = b"PK\x03\x04 le plugin";

#[test]
fn le_plugin_et_les_reglages_sont_recopies() {
    let dossier = tempfile::tempdir().unwrap();
    let source = dossier.path().join("construit.jar");
    std::fs::write(&source, JAR).unwrap();
    // Le dossier du serveur n'existe pas encore : première installation.
    let plugins = dossier.path().join("data/plugins");

    let cible = recopier_plugin(&plugins, &source).unwrap();
    assert_eq!(cible, plugins.join(NOM_DU_PLUGIN));
    assert_eq!(std::fs::read(&cible).unwrap(), JAR);
    let reglages = plugins.join("voicechat/voicechat-server.properties");
    assert_eq!(
        std::fs::read_to_string(&reglages).unwrap(),
        REGLAGES_DU_CHAT_VOCAL
    );
    // Rien d'autre : pas de fichier provisoire oublié.
    let mut noms: Vec<String> = std::fs::read_dir(&plugins)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    noms.sort();
    assert_eq!(noms, [NOM_DU_PLUGIN, "voicechat"]);
}

#[test]
fn une_nouvelle_version_remplace_le_plugin_mais_garde_les_reglages_du_parent() {
    let dossier = tempfile::tempdir().unwrap();
    let source = dossier.path().join("construit.jar");
    std::fs::write(&source, JAR).unwrap();
    let plugins = dossier.path().join("plugins");
    recopier_plugin(&plugins, &source).unwrap();

    let reglages = plugins.join("voicechat/voicechat-server.properties");
    std::fs::write(&reglages, "port=24454\nvoice_host=192.168.1.20\n").unwrap();
    std::fs::write(&source, b"PK\x03\x04 la version suivante").unwrap();
    recopier_plugin(&plugins, &source).unwrap();

    assert_eq!(
        std::fs::read(plugins.join(NOM_DU_PLUGIN)).unwrap(),
        b"PK\x03\x04 la version suivante"
    );
    assert_eq!(
        std::fs::read_to_string(&reglages).unwrap(),
        "port=24454\nvoice_host=192.168.1.20\n"
    );
}

#[test]
fn un_plugin_absent_ou_qui_n_est_pas_un_jar_est_refuse_sans_rien_ecrire() {
    let dossier = tempfile::tempdir().unwrap();
    let plugins = dossier.path().join("plugins");
    let absent = recopier_plugin(&plugins, &dossier.path().join("absent.jar")).unwrap_err();
    assert!(
        absent.to_string().contains("plugin introuvable"),
        "{absent}"
    );
    let texte = dossier.path().join("texte.jar");
    std::fs::write(&texte, "pas une archive").unwrap();
    let faux = recopier_plugin(&plugins, &texte).unwrap_err();
    assert!(
        faux.to_string().contains("n'est pas un fichier .jar"),
        "{faux}"
    );
    assert!(!plugins.exists());
}

#[test]
fn les_reglages_du_chat_vocal_ferment_les_groupes_et_suivent_le_compose() {
    let cles: Vec<&str> = REGLAGES_DU_CHAT_VOCAL
        .lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .collect();
    assert_eq!(cles, ["port=24454", "enable_groups=false", "voice_host="]);
    // Le port réglé est celui que le compose de production publie, en UDP.
    let compose = std::fs::read_to_string(commun::racine().join("docker-compose.yml")).unwrap();
    assert!(compose.contains("\"24454:24454/udp\""));
    // Le service ponctuel livre le datapack et le plugin, sans interpréteur de commandes.
    assert!(compose.contains(
        r#"command: ["mine-de-rien", "livrer", "--datapack", "/datapacks", "--plugin", "/plugins"]"#
    ));
    // Le serveur ne fait que lire les sons ; l'agent les écrit.
    assert!(compose.contains("- ./data/sons:/sons:ro"));
    assert!(compose.contains("- ./data/sons:/sons "));
}
