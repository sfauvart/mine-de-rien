//! Recherche d'un emplacement, sur un terrain simulé qui répond comme le serveur 26.3.

mod commun;

use std::collections::{HashMap, HashSet};

use mine_de_rien::games::atelier::{Atelier, ErreurAtelier};
use mine_de_rien::games::emplacement::{
    AUCUN_TERRAIN, AUTRE_DIMENSION, DISTANCE_MIN, ECART_VERTICAL, ENFANT_ABSENT, RAYON, centres,
    chercher, commande_hauteur, construire_pres_de, deplacer_pres_de, direction, position_de,
};
use serde_json::json;

use commun::terrain::{PSEUDO, Terrain, X, Y, Z};

async fn cherche(terrain: &Terrain) -> Result<Option<Atelier>, ErreurAtelier> {
    chercher(terrain, X, Y, Z).await
}

const PREMIER: (i32, i32) = (X - DISTANCE_MIN, Z - DISTANCE_MIN);

fn refus<T: std::fmt::Debug>(resultat: Result<T, ErreurAtelier>) -> String {
    match resultat {
        Err(ErreurAtelier::Refus(message)) => message,
        autre => panic!("un refus était attendu, pas {autre:?}"),
    }
}

#[test]
fn couronnes_du_plus_proche_au_plus_loin_sans_doublon() {
    let tous = centres(0, 0, DISTANCE_MIN, RAYON);
    let uniques: HashSet<(i32, i32)> = tous.iter().copied().collect();
    let attendu = ((2 * RAYON + 1).pow(2) - (2 * DISTANCE_MIN - 1).pow(2)) as usize;
    assert_eq!((tous.len(), uniques.len()), (attendu, attendu));
    let distances: Vec<i32> = tous.iter().map(|(x, z)| x.abs().max(z.abs())).collect();
    assert!(distances.is_sorted());
    assert_eq!(
        (distances[0], *distances.last().unwrap()),
        (DISTANCE_MIN, RAYON)
    );
}

#[tokio::test]
async fn terrain_plat_premier_centre() {
    let trouve = cherche(&Terrain::default()).await.unwrap();
    assert_eq!(trouve, Some(Atelier::new(PREMIER.0, Y, PREMIER.1)));
}

#[tokio::test]
async fn ordre_deterministe() {
    assert_eq!(
        cherche(&Terrain::default()).await,
        cherche(&Terrain::default()).await
    );
}

#[test]
fn commande_de_hauteur() {
    assert_eq!(
        commande_hauteur(-3, 12),
        "execute positioned -3 0 12 positioned over motion_blocking_no_leaves \
         run tp @e[type=minecraft:marker,tag=mdr_sonde,limit=1] ~ ~ ~"
    );
}

#[tokio::test]
async fn une_bosse_d_un_bloc_ecarte_les_centres_qui_la_couvrent() {
    let (px, pz) = PREMIER;
    let terrain = Terrain {
        sols: HashMap::from([((px, pz), Y + 1)]),
        ..Terrain::default()
    };
    assert_eq!(
        cherche(&terrain).await.unwrap(),
        Some(Atelier::new(px + 3, Y, pz))
    );
}

#[tokio::test]
async fn l_eau_au_niveau_du_plancher_est_refusee() {
    let (px, pz) = PREMIER;
    let terrain = Terrain {
        blocs: HashMap::from([((px, Y - 1, pz), "minecraft:water")]),
        ..Terrain::default()
    };
    assert_eq!(
        cherche(&terrain).await.unwrap(),
        Some(Atelier::new(px + 3, Y, pz))
    );
}

#[tokio::test]
async fn une_construction_au_dessus_du_sol_est_refusee() {
    let (px, pz) = PREMIER;
    let terrain = Terrain {
        blocs: HashMap::from([((px, Y + 1, pz), "minecraft:oak_planks")]),
        ..Terrain::default()
    };
    assert_eq!(
        cherche(&terrain).await.unwrap(),
        Some(Atelier::new(px + 3, Y, pz))
    );
}

#[tokio::test]
async fn jamais_sur_l_enfant_ni_dans_sa_zone() {
    let a = cherche(&Terrain::default()).await.unwrap().unwrap();
    assert!((a.x - X).abs().max((a.z - Z).abs()) >= DISTANCE_MIN);
}

/// Au fond d'une mine : tout le terrain est plat, mais très au-dessus de l'enfant.
#[tokio::test]
async fn surface_trop_loin_verticalement_refusee() {
    let terrain = Terrain {
        relief: Some(|_, _| Y + ECART_VERTICAL + 1),
        ..Terrain::default()
    };
    assert!(refus(cherche(&terrain).await).contains("sous terre"));
    assert!(
        !terrain
            .recues()
            .iter()
            .any(|c| c.contains("execute if block"))
    );
    assert_eq!(terrain.sondes.get(), 0);
}

#[tokio::test]
async fn ecart_vertical_limite_accepte() {
    let terrain = Terrain {
        relief: Some(|_, _| Y + ECART_VERTICAL),
        ..Terrain::default()
    };
    assert_eq!(
        cherche(&terrain).await.unwrap().unwrap().y,
        Y + ECART_VERTICAL
    );
}

#[tokio::test]
async fn chunk_non_charge_ecarte_le_centre() {
    let terrain = Terrain {
        charge: |x, _| x > PREMIER.0 + 2,
        ..Terrain::default()
    };
    let a = cherche(&terrain).await.unwrap().unwrap();
    assert!(a.x - 2 > PREMIER.0 + 2);
}

fn bossele(x: i32, z: i32) -> i32 {
    Y + (x + z).rem_euclid(2)
}

#[tokio::test]
async fn aucun_emplacement_valide() {
    let terrain = Terrain {
        relief: Some(bossele),
        ..Terrain::default()
    };
    assert_eq!(cherche(&terrain).await.unwrap(), None);
    assert_eq!(terrain.sondes.get(), 0);
}

#[tokio::test]
async fn la_sonde_est_retiree_apres_succes() {
    let terrain = Terrain::default();
    cherche(&terrain).await.unwrap();
    assert_eq!(
        terrain.recues().last().unwrap(),
        "kill @e[type=minecraft:marker,tag=mdr_sonde]"
    );
    assert_eq!(terrain.sondes.get(), 0);
}

#[tokio::test]
async fn la_sonde_est_retiree_si_le_serveur_tombe() {
    let terrain = Terrain {
        coupure: true,
        ..Terrain::default()
    };
    assert!(matches!(
        cherche(&terrain).await,
        Err(ErreurAtelier::Rcon(_))
    ));
    assert_eq!(terrain.sondes.get(), 0);
}

#[tokio::test]
async fn refus_si_la_sonde_ne_peut_pas_etre_posee() {
    let terrain = Terrain {
        summon: "Unable to summon entity",
        ..Terrain::default()
    };
    assert!(refus(cherche(&terrain).await).contains("sonder"));
}

#[tokio::test]
async fn hauteurs_lues_une_seule_fois() {
    let terrain = Terrain {
        sols: HashMap::from([((X - 8, Z - 8), Y + 1)]),
        ..Terrain::default()
    };
    cherche(&terrain).await.unwrap();
    let recues = terrain.recues();
    let hauteurs: Vec<&String> = recues
        .iter()
        .filter(|c| c.contains("positioned over"))
        .collect();
    let uniques: HashSet<&String> = hauteurs.iter().copied().collect();
    assert_eq!(hauteurs.len(), uniques.len());
}

#[test]
fn direction_dominante() {
    for (dx, dz, attendu) in [
        (10, 3, "l'est"),
        (-10, 3, "l'ouest"),
        (2, 9, "le sud"),
        (2, -9, "le nord"),
    ] {
        assert_eq!(direction(dx, dz), attendu);
    }
}

#[tokio::test]
async fn position_arrondie_vers_le_bas_en_negatif() {
    let terrain = Terrain {
        joueur: Some((-27.5, 68.0, -0.25)),
        ..Terrain::default()
    };
    assert_eq!(position_de(&terrain, PSEUDO).await.unwrap(), (-28, 68, -1));
}

#[tokio::test]
async fn position_refuse_un_pseudo_dangereux() {
    let terrain = Terrain::default();
    assert!(matches!(
        position_de(&terrain, "@a").await,
        Err(ErreurAtelier::Rcon(_))
    ));
    assert!(terrain.recues().is_empty());
}

#[tokio::test]
async fn enfant_hors_ligne() {
    let terrain = Terrain {
        joueur: None,
        ..Terrain::default()
    };
    assert_eq!(
        refus(construire_pres_de(&terrain, PSEUDO).await),
        ENFANT_ABSENT
    );
    assert!(!terrain.recues().iter().any(|c| c.starts_with("summon")));
}

#[tokio::test]
async fn enfant_dans_une_autre_dimension() {
    let terrain = Terrain {
        dimension: false,
        ..Terrain::default()
    };
    assert_eq!(
        refus(construire_pres_de(&terrain, PSEUDO).await),
        AUTRE_DIMENSION
    );
}

#[tokio::test]
async fn rien_trouve_rien_construit() {
    let terrain = Terrain {
        relief: Some(bossele),
        ..Terrain::default()
    };
    assert_eq!(
        refus(construire_pres_de(&terrain, PSEUDO).await),
        AUCUN_TERRAIN
    );
    assert!(AUCUN_TERRAIN.contains(&format!("moins de {RAYON} blocs")));
    assert!(!terrain.recues().iter().any(|c| {
        ["fill", "setblock", "forceload"]
            .iter()
            .any(|debut| c.starts_with(debut))
    }));
    assert!(terrain.dits.borrow().is_empty());
}

#[tokio::test]
async fn construit_et_previent_l_enfant() {
    let terrain = Terrain::default();
    let c = construire_pres_de(&terrain, PSEUDO).await.unwrap();
    assert_eq!(c.atelier, Atelier::new(X - 8, Y, Z - 8));
    assert_eq!(c.distance, 11);
    assert!(
        terrain
            .recues()
            .last()
            .unwrap()
            .starts_with("data modify storage")
    );
    assert_eq!(
        *terrain.dits.borrow(),
        [(
            PSEUDO.to_owned(),
            "Ton atelier vient d'apparaître, à 11 blocs vers l'ouest.".to_owned()
        )]
    );
}

/// Sur treize terrains, les mêmes commandes dans le même ordre que la version Python, le
/// même emplacement ou le même refus, et le même message à l'enfant.
#[tokio::test]
async fn oracle_python() {
    for cas in commun::oracle("emplacement") {
        let e = &cas["entree"];
        if e["f"] == "direction" {
            let (dx, dz) = (
                e["dx"].as_i64().unwrap() as i32,
                e["dz"].as_i64().unwrap() as i32,
            );
            assert_eq!(direction(dx, dz), cas["sortie"].as_str().unwrap(), "{e}");
            continue;
        }
        let rejeu = commun::Rejeu::new(&cas);
        let construction = |c: mine_de_rien::games::emplacement::Construction| {
            json!({
                "atelier": [c.atelier.x, c.atelier.y, c.atelier.z],
                "distance": c.distance,
                "direction": c.direction,
            })
        };
        match e["f"].as_str().unwrap() {
            "chercher" => commun::conclure(
                &cas,
                &rejeu,
                chercher(&rejeu, X, Y, Z)
                    .await
                    .map(|a| a.map(|a| [a.x, a.y, a.z])),
            ),
            "position_de" => commun::conclure(
                &cas,
                &rejeu,
                position_de(&rejeu, PSEUDO).await.map(|(x, y, z)| [x, y, z]),
            ),
            "construire_pres_de" => commun::conclure(
                &cas,
                &rejeu,
                construire_pres_de(&rejeu, PSEUDO).await.map(construction),
            ),
            "deplacer_pres_de" => {
                let ancien = Atelier::new(X + 40, Y, Z + 40);
                commun::conclure(
                    &cas,
                    &rejeu,
                    deplacer_pres_de(&rejeu, PSEUDO, &ancien)
                        .await
                        .map(construction),
                )
            }
            autre => panic!("fonction inconnue : {autre}"),
        }
    }
}
