mod commun;

use std::collections::HashMap;

use mine_de_rien::config::{charger_config, lire_config};

const MINIMALE: &str = "[enfant]\npseudo_minecraft = \"Steve\"\n";

fn sans_variable(_: &str) -> Option<String> {
    None
}

#[test]
fn substitution_des_variables() {
    let texte =
        "[enfant]\npseudo_minecraft = \"Steve\"\n[jeu]\nmot_de_passe = \"${RCON_PASSWORD}\"\n";
    let config = lire_config(texte, &|nom| {
        (nom == "RCON_PASSWORD").then(|| "secret".to_owned())
    })
    .unwrap();
    assert_eq!(config.jeu.mot_de_passe, "secret");
    assert_eq!(config.enfant.alias_llm, "l'élève");
}

#[test]
fn exemple_fourni_se_charge() {
    let config = charger_config(&commun::racine().join("config.example.toml")).unwrap();
    assert_eq!(config.llm.adaptateur, "anthropic");
}

#[test]
fn config_essai_vise_le_serveur_d_essai() {
    let config = charger_config(&commun::racine().join("config.essai.example.toml")).unwrap();
    assert_eq!(config.jeu.hote, "localhost");
    assert_eq!(config.jeu.port, 25576); // jamais 25575, le monde de l'enfant
}

#[test]
fn assistant_par_defaut_et_nom_verifie() {
    let config = lire_config(MINIMALE, &sans_variable).unwrap();
    assert_eq!(
        (config.assistant.nom.as_str(), config.assistant.actif),
        ("Pixel", true)
    );
    let erreur = lire_config(
        &format!("{MINIMALE}[assistant]\nnom = \"@a tp\"\n"),
        &sans_variable,
    )
    .unwrap_err();
    assert!(erreur.to_string().contains("assistant"));
}

#[test]
fn paisible_par_defaut_et_difficulte_verifiee() {
    let config = lire_config(MINIMALE, &sans_variable).unwrap();
    assert_eq!(
        (
            config.paisible.actif,
            config.paisible.difficulte_normale.as_str()
        ),
        (true, "easy")
    );
    let erreur = lire_config(
        &format!("{MINIMALE}[paisible]\ndifficulte_normale = \"peaceful\"\n"),
        &sans_variable,
    )
    .unwrap_err();
    assert!(erreur.to_string().contains("paisible"));
}

/// Chaque configuration lue par Python donne la même chose en Rust, refus compris.
#[test]
fn oracle_python() {
    for cas in commun::oracle("config") {
        let entree = &cas["entree"];
        let variables: HashMap<String, String> =
            serde_json::from_value(entree["variables"].clone()).unwrap();
        let lue = lire_config(entree["toml"].as_str().unwrap(), &|nom| {
            variables.get(nom).cloned()
        });
        match cas.get("erreur") {
            Some(message) => {
                assert_eq!(
                    lue.unwrap_err().to_string(),
                    message.as_str().unwrap(),
                    "{entree}"
                )
            }
            None => assert_eq!(
                serde_json::to_value(lue.unwrap()).unwrap(),
                cas["sortie"],
                "{entree}"
            ),
        }
    }
}

#[test]
fn une_configuration_qui_porte_encore_pack_se_charge() {
    // La section a disparu avec le pack de sons : elle est ignorée, quel que soit son contenu.
    let ancienne = format!(
        "{MINIMALE}[pack]\nport = 9000\necoute = \"0.0.0.0\"\nadresse = \"192.168.1.20:25565\"\n"
    );
    let config = lire_config(&ancienne, &sans_variable).unwrap();
    assert!(config.pack.is_some());
    assert_eq!(config.enfant.pseudo_minecraft, "Steve");
    // Elle ne se réécrit pas : une configuration relue puis écrite ne la porte plus.
    let ecrite = toml::to_string(&config).unwrap();
    assert!(!ecrite.contains("pack"), "{ecrite}");
    // Sans elle, rien à signaler.
    assert!(
        lire_config(MINIMALE, &sans_variable)
            .unwrap()
            .pack
            .is_none()
    );
}

#[test]
fn la_voix_dit_le_chat_par_defaut_et_le_reglage_se_coupe() {
    let voix = |reste: &str| {
        lire_config(
            &format!("{MINIMALE}[voix]\nurl = \"http://voix:5000\"\n{reste}"),
            &sans_variable,
        )
        .unwrap()
        .voix
        .unwrap()
    };
    let par_defaut = voix("");
    assert!(par_defaut.dire_le_chat);
    assert_eq!(par_defaut.dossier_du_jeu, "");
    let regle = voix("dossier_du_jeu = \"/sons\"\ndire_le_chat = false\n");
    assert!(!regle.dire_le_chat);
    assert_eq!(regle.dossier_du_jeu, "/sons");
    // Aux valeurs d'usage, ni l'un ni l'autre ne s'écrit : même relecture qu'avant.
    let ecrite = toml::to_string(&par_defaut).unwrap();
    assert!(
        !ecrite.contains("dire_le_chat") && !ecrite.contains("dossier_du_jeu"),
        "{ecrite}"
    );
    assert!(
        toml::to_string(&regle)
            .unwrap()
            .contains("dire_le_chat = false")
    );
}

#[test]
fn cadeaux_par_defaut_et_liste_reglee() {
    let config = lire_config(MINIMALE, &sans_variable).unwrap();
    assert_eq!(config.coffre.cadeaux_par_coffre, 2);
    assert_eq!(config.coffre.cadeaux.len(), 13);
    assert!(config.coffre.cadeaux.iter().all(|c| c.nom.is_some()));
    // Aux valeurs d'usage, rien de nouveau ne s'écrit.
    let ecrite = toml::to_string(&config).unwrap();
    assert!(!ecrite.contains("cadeaux"), "{ecrite}");

    let reglee = lire_config(
        &format!(
            "{MINIMALE}[coffre]\ncadeaux_par_coffre = 1\ncadeaux = [\n  \
             {{ objet = \"minecraft:diamond\", nombre = 3, poids = 10, nom = \"diamant\" }},\n  \
             {{ objet = \"saddle\" }},\n]\n"
        ),
        &sans_variable,
    )
    .unwrap();
    assert_eq!(reglee.coffre.cadeaux_par_coffre, 1);
    let selle = &reglee.coffre.cadeaux[1];
    assert_eq!(
        (selle.objet.as_str(), selle.nombre, selle.poids, &selle.nom),
        ("saddle", 1, 1, &None)
    );
    let ecrite = toml::to_string(&reglee).unwrap();
    assert!(ecrite.contains("cadeaux_par_coffre"), "{ecrite}");
    assert_eq!(lire_config(&ecrite, &sans_variable).unwrap(), reglee);
}

#[test]
fn chaque_cadeau_mal_regle_est_refuse() {
    let refus = |coffre: &str| {
        lire_config(&format!("{MINIMALE}[coffre]\n{coffre}\n"), &sans_variable)
            .unwrap_err()
            .to_string()
    };
    let deux = "{ objet = \"diamond\" }, { objet = \"emerald\" }";
    for (coffre, attendu) in [
        (
            "cadeaux_par_coffre = 0",
            "cadeaux_par_coffre doit être entre 1 et 5",
        ),
        (
            "cadeaux_par_coffre = 6",
            "cadeaux_par_coffre doit être entre 1 et 5",
        ),
        (
            "cadeaux = [{ objet = \"diamond\" }]",
            "il en faut au moins 2",
        ),
        ("cadeaux = []", "il en faut au moins 2"),
        (
            "cadeaux = [{ objet = \"Diamant\" }, { objet = \"emerald\" }]",
            "objet invalide : 'Diamant'",
        ),
        (
            "cadeaux = [{ objet = \"diamond 64\" }, { objet = \"emerald\" }]",
            "objet invalide",
        ),
        (
            "cadeaux = [{ objet = \"diamond\", nombre = 65 }, { objet = \"emerald\" }]",
            "nombre doit être entre 1 et 64 (diamond)",
        ),
        (
            "cadeaux = [{ objet = \"diamond\", nombre = 0 }, { objet = \"emerald\" }]",
            "nombre doit être entre 1 et 64 (diamond)",
        ),
        (
            "cadeaux = [{ objet = \"diamond\", poids = 0 }, { objet = \"emerald\" }]",
            "poids doit être entre 1 et 100 (diamond)",
        ),
        (
            "cadeaux = [{ objet = \"diamond\", poids = 101 }, { objet = \"emerald\" }]",
            "poids doit être entre 1 et 100 (diamond)",
        ),
        (
            "cadeaux = [{ objet = \"diamond\", nom = \" \" }, { objet = \"emerald\" }]",
            "nom de 1 à 30 caractères (diamond)",
        ),
        (
            "cadeaux = [{ objet = \"diamond\", nom = \"éééééééééééééééééééééééééééééééé\" }, { objet = \"emerald\" }]",
            "nom de 1 à 30 caractères (diamond)",
        ),
        (
            "cadeaux = [{ objet = \"diamond\" }, { objet = \"minecraft:diamond\" }]",
            "minecraft:diamond figure deux fois",
        ),
        (
            "cadeaux = [{ objet = \"diamond\", couleur = \"bleu\" }, { objet = \"emerald\" }]",
            "couleur",
        ),
    ] {
        let message = refus(coffre);
        assert!(message.contains(attendu), "{coffre} → {message}");
    }
    // Un nom de 30 lettres accentuées passe : la longueur se compte en caractères.
    let trente = "é".repeat(30);
    assert!(
        lire_config(
            &format!(
                "{MINIMALE}[coffre]\ncadeaux = [{{ objet = \"diamond\", nom = \"{trente}\" }}, {deux}]\n"
            ),
            &sans_variable
        )
        .is_err() // « diamond » y figure deux fois
    );
    assert!(
        lire_config(
            &format!(
                "{MINIMALE}[coffre]\ncadeaux = [{{ objet = \"cake\", nom = \"{trente}\" }}, {deux}]\n"
            ),
            &sans_variable
        )
        .is_ok()
    );
}
