//! Recherche d'un emplacement, sur un terrain simulé qui répond comme le serveur 26.3.

mod commun;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use async_trait::async_trait;
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::games::atelier::{Atelier, ErreurAtelier};
use mine_de_rien::games::emplacement::{
    AUCUN_TERRAIN, AUTRE_DIMENSION, DISTANCE_MIN, ECART_VERTICAL, ENFANT_ABSENT, RAYON, centres,
    chercher, commande_hauteur, construire_pres_de, deplacer_pres_de, direction, position_de,
};
use mine_de_rien::minecraft::texte::Style;
use regex::Regex;
use serde_json::json;

const X: i32 = 100;
const Y: i32 = 64;
const Z: i32 = 200;
const PSEUDO: &str = "Steve_42";

static HAUTEUR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^execute positioned (-?\d+) 0 (-?\d+) positioned over").unwrap());
static SI_BLOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^execute if block (-?\d+) (-?\d+) (-?\d+) (\S+)$").unwrap());

/// Sol à `Y` partout, sauf les colonnes de `sols` ou ce que dit `relief` ; `blocs` pour le reste.
struct Terrain {
    sols: HashMap<(i32, i32), i32>,
    relief: Option<fn(i32, i32) -> i32>,
    blocs: HashMap<(i32, i32, i32), &'static str>,
    charge: fn(i32, i32) -> bool,
    summon: &'static str,
    joueur: Option<(f64, f64, f64)>,
    dimension: bool,
    /// Le serveur tombe à la première lecture de hauteur.
    coupure: bool,
    sondes: Cell<i32>,
    recues: RefCell<Vec<String>>,
    dits: RefCell<Vec<(String, String)>>,
}

impl Default for Terrain {
    fn default() -> Self {
        Self {
            sols: HashMap::new(),
            relief: None,
            blocs: HashMap::new(),
            charge: |_, _| true,
            summon: "Summoned new Marker",
            joueur: Some((f64::from(X) + 0.5, f64::from(Y), f64::from(Z) + 0.5)),
            dimension: true,
            coupure: false,
            sondes: Cell::new(0),
            recues: RefCell::new(Vec::new()),
            dits: RefCell::new(Vec::new()),
        }
    }
}

impl Terrain {
    fn sol(&self, x: i32, z: i32) -> i32 {
        match self.relief {
            Some(relief) => relief(x, z),
            None => self.sols.get(&(x, z)).copied().unwrap_or(Y),
        }
    }

    fn bloc(&self, x: i32, y: i32, z: i32) -> &str {
        match self.blocs.get(&(x, y, z)) {
            Some(bloc) => bloc,
            None if y >= self.sol(x, z) => "minecraft:air",
            None => "minecraft:grass_block",
        }
    }

    fn recues(&self) -> Vec<String> {
        self.recues.borrow().clone()
    }
}

#[async_trait(?Send)]
impl AdaptateurJeu for Terrain {
    async fn dire(
        &self,
        joueur: &str,
        texte: &str,
        _: &[(&str, Style<'_>)],
    ) -> Result<(), ErreurRcon> {
        self.dits
            .borrow_mut()
            .push((joueur.to_owned(), texte.to_owned()));
        Ok(())
    }

    async fn commande(&self, c: &str) -> Result<String, ErreurRcon> {
        self.recues.borrow_mut().push(c.to_owned());
        let ok = |texte: &str| Ok(texte.to_owned());
        if c.starts_with(&format!("data get entity {PSEUDO} Pos")) {
            return Ok(match self.joueur {
                None => "No entity was found".to_owned(),
                Some((x, y, z)) => {
                    format!("{PSEUDO} has the following entity data: [{x:?}d, {y:?}d, {z:?}d]")
                }
            });
        }
        if c.starts_with("execute in minecraft:overworld if entity") {
            return ok(if self.dimension {
                "Test passed. Count: 1"
            } else {
                "Test failed"
            });
        }
        if c.starts_with("data get storage") {
            return ok("Found no elements matching pos");
        }
        if c.starts_with("data") {
            return ok("Modified storage mine_de_rien:atelier");
        }
        if c.starts_with("forceload") {
            return ok("Marked 2 chunks to be force loaded");
        }
        if c.starts_with("kill") {
            self.sondes.set(0);
            return ok("Killed Marker");
        }
        if c.starts_with("summon") {
            if self.summon.starts_with("Summoned") {
                self.sondes.set(self.sondes.get() + 1);
            }
            return ok(self.summon);
        }
        if let Some(m) = HAUTEUR.captures(c) {
            if self.coupure {
                return Err(ErreurRcon("RCON coupé".to_owned()));
            }
            let (x, z): (i32, i32) = (m[1].parse().unwrap(), m[2].parse().unwrap());
            if !(self.charge)(x, z) {
                return ok("That position is not loaded");
            }
            return Ok(format!(
                "Teleported Marker to {x}.500000, {}.000000, {z}.500000",
                self.sol(x, z)
            ));
        }
        if let Some(m) = SI_BLOC.captures(c) {
            let (x, y, z): (i32, i32, i32) = (
                m[1].parse().unwrap(),
                m[2].parse().unwrap(),
                m[3].parse().unwrap(),
            );
            if !(self.charge)(x, z) {
                return ok("That position is not loaded");
            }
            return ok(if self.bloc(x, y, z) == &m[4] {
                "Test passed"
            } else {
                "Test failed"
            });
        }
        ok("")
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat ici".to_owned()))
    }
}

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
