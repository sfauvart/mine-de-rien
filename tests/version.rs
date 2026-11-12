//! La version n'a qu'une source : `Cargo.toml`. C'est elle qu'affiche `mine-de-rien --version`,
//! et les images citées dans `docker-compose.yml` doivent porter le même numéro : ce fichier
//! est celui que les installations téléchargent depuis le tag. Le plugin Paper livré dans
//! l'image (`plugin/pom.xml`) porte ce numéro lui aussi.
//!
//! À la publication, le workflow lance ce test avec `MDR_TAG` (le tag git poussé) : un tag
//! qui ne correspond pas à la version déclarée ne publie rien.

mod commun;

use std::sync::LazyLock;

use regex::Regex;

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^v([0-9]+\.[0-9]+\.[0-9]+)$").unwrap());
/// L'image de l'agent, et elle seule : celle de la voix (`mine-de-rien-voix`) porte le numéro
/// de Piper, pas celui de l'agent.
static IMAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?m)^\s*image:\s*["']?ghcr\.io/sfauvart/mine-de-rien(?::([^\s"']+))?(?:["'\s]|$)"#,
    )
    .unwrap()
});

static VERSION_DU_PLUGIN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"<artifactId>mine-de-rien-voix</artifactId>\s*(?:<!--.*?-->\s*)*<version>([^<]+)</version>",
    )
    .unwrap()
});

/// La version que déclare le plugin, ou `None` si son pom ne se lit pas.
fn version_du_plugin(pom: &str) -> Option<&str> {
    VERSION_DU_PLUGIN
        .captures(pom)
        .and_then(|m| m.get(1))
        .map(|v| v.as_str())
}

fn pom_du_depot() -> String {
    std::fs::read_to_string(commun::racine().join("plugin/pom.xml")).unwrap()
}

/// Liste des écarts entre le tag et les versions déclarées ; vide si tout concorde.
fn ecarts(tag: &str, version_du_paquet: &str, compose: &str) -> Vec<String> {
    let Some(attendue) = TAG.captures(tag).map(|m| m[1].to_owned()) else {
        return vec![format!("Le tag « {tag} » n'a pas la forme vX.Y.Z.")];
    };
    let mut ecarts = Vec::new();
    if version_du_paquet != attendue {
        ecarts.push(format!(
            "Cargo.toml annonce {version_du_paquet}, le tag {attendue}."
        ));
    }
    let images: Vec<&str> = IMAGE
        .captures_iter(compose)
        .map(|m| m.get(1).map_or("sans tag", |tag| tag.as_str()))
        .collect();
    if images.is_empty() {
        ecarts.push(
            "docker-compose.yml ne cite pas l'image ghcr.io/sfauvart/mine-de-rien.".to_owned(),
        );
    }
    for image in images {
        if image != attendue {
            ecarts.push(format!(
                "docker-compose.yml cite l'image {image}, le tag {attendue}."
            ));
        }
    }
    ecarts
}

/// Un faux compose ; les images suivent le tag v1.2.3 des tests par défaut.
fn compose(images: &[&str]) -> String {
    let services: String = images
        .iter()
        .enumerate()
        .map(|(i, tag)| format!("  s{i}:\n    image: ghcr.io/sfauvart/mine-de-rien{tag}\n"))
        .collect();
    format!("services:\n  minecraft:\n    image: itzg/minecraft-server:2026.10.1\n{services}")
}

fn compose_du_depot() -> String {
    std::fs::read_to_string(commun::racine().join("docker-compose.yml")).unwrap()
}

#[test]
fn le_depot_est_coherent_avec_sa_propre_version() {
    let version = mine_de_rien::VERSION;
    assert_eq!(
        ecarts(&format!("v{version}"), version, &compose_du_depot()),
        Vec::<String>::new()
    );
}

#[test]
fn l_image_de_la_voix_suit_piper_pas_l_agent() {
    let avec_voix = format!(
        "{}  voix:\n    image: ghcr.io/sfauvart/mine-de-rien-voix:1.8.0\n",
        compose(&[":1.2.3"])
    );
    assert_eq!(ecarts("v1.2.3", "1.2.3", &avec_voix), Vec::<String>::new());
    // Seule, elle ne tient pas lieu d'image de l'agent.
    let voix_seule = "services:\n  voix:\n    image: ghcr.io/sfauvart/mine-de-rien-voix:1.8.0\n";
    assert_eq!(ecarts("v1.2.3", "1.2.3", voix_seule).len(), 1);
}

/// Lancé par le workflow de publication : `MDR_TAG=v0.2.0 cargo test --test version`.
#[test]
fn le_tag_publie_est_celui_de_la_version() {
    let Ok(tag) = std::env::var("MDR_TAG") else {
        return;
    };
    assert_eq!(
        ecarts(&tag, mine_de_rien::VERSION, &compose_du_depot()),
        Vec::<String>::new(),
        "le tag {tag} ne publie rien tant que ces écarts restent"
    );
    assert_eq!(
        version_du_plugin(&pom_du_depot()).map(|v| format!("v{v}")),
        Some(tag.clone()),
        "plugin/pom.xml n'annonce pas la version du tag {tag}"
    );
}

#[test]
fn le_plugin_porte_la_version_du_paquet() {
    assert_eq!(
        version_du_plugin(&pom_du_depot()),
        Some(mine_de_rien::VERSION),
        "plugin/pom.xml et Cargo.toml doivent annoncer la même version"
    );
    // La version d'une dépendance ne compte pas, ni un pom sans version.
    let autre = "<artifactId>paper-api</artifactId>\n<version>26.3</version>";
    assert_eq!(version_du_plugin(autre), None);
    assert_eq!(
        version_du_plugin(
            "<artifactId>mine-de-rien-voix</artifactId>\n  <!-- note -->\n  <version>1.2.3</version>"
        ),
        Some("1.2.3")
    );
}

#[test]
fn tout_concorde() {
    assert!(ecarts("v1.2.3", "1.2.3", &compose(&[":1.2.3", ":1.2.3"])).is_empty());
}

#[test]
fn version_du_paquet_en_retard() {
    assert_eq!(
        ecarts("v1.2.3", "1.2.2", &compose(&[":1.2.3", ":1.2.3"])),
        ["Cargo.toml annonce 1.2.2, le tag 1.2.3."]
    );
}

#[test]
fn une_image_du_compose_oubliee() {
    assert_eq!(
        ecarts("v1.2.3", "1.2.3", &compose(&[":1.2.3", ":1.2.2"])),
        ["docker-compose.yml cite l'image 1.2.2, le tag 1.2.3."]
    );
}

#[test]
fn une_image_du_compose_sans_tag() {
    assert_eq!(
        ecarts("v1.2.3", "1.2.3", &compose(&["", ":latest"])),
        [
            "docker-compose.yml cite l'image sans tag, le tag 1.2.3.",
            "docker-compose.yml cite l'image latest, le tag 1.2.3.",
        ]
    );
}

#[test]
fn le_compose_doit_citer_l_image() {
    assert_eq!(
        ecarts("v1.2.3", "1.2.3", &compose(&[])),
        ["docker-compose.yml ne cite pas l'image ghcr.io/sfauvart/mine-de-rien."]
    );
}

#[test]
fn tag_mal_forme() {
    for tag in ["1.2.3", "v1.2", "v1.2.3-rc1", "main"] {
        let ecarts = ecarts(tag, "1.2.3", &compose(&[":1.2.3"]));
        assert!(ecarts.len() == 1 && ecarts[0].contains("vX.Y.Z"), "{tag}");
    }
}
