//! La version n'a qu'une source : `Cargo.toml`. C'est elle qu'affiche `mine-de-rien --version`,
//! et les images citées dans `docker-compose.yml` doivent porter le même numéro : ce fichier
//! est celui que les installations téléchargent depuis le tag.
//!
//! À la publication, le workflow lance ce test avec `MDR_TAG` (le tag git poussé) : un tag
//! qui ne correspond pas à la version déclarée ne publie rien.

mod commun;

use std::sync::LazyLock;

use regex::Regex;

static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^v([0-9]+\.[0-9]+\.[0-9]+)$").unwrap());
static IMAGE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?m)^\s*image:\s*["']?ghcr\.io/sfauvart/mine-de-rien(?::([^\s"']+))?"#).unwrap()
});

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
