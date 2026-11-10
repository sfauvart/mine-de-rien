mod commun;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::LazyLock;

use async_trait::async_trait;
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::games::atelier::{
    self, Atelier, ErreurAtelier, Position, commande_forceload, commandes_construction, construire,
    verifications_de_place,
};
use mine_de_rien::minecraft::texte::Style;
use regex::Regex;
use serde_json::json;

static SI_BLOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^execute if block (-?\d+) (-?\d+) (-?\d+) (\S+)$").unwrap());

/// Répond aux commandes de l'atelier comme le serveur 26.3, à partir d'une table de blocs.
#[derive(Default)]
struct FauxMonde {
    blocs: HashMap<Position, &'static str>,
    pos: Option<Position>,
    recues: RefCell<Vec<String>>,
}

impl FauxMonde {
    fn avec(blocs: &[(Position, &'static str)]) -> Self {
        Self {
            blocs: blocs.iter().copied().collect(),
            ..Self::default()
        }
    }

    fn bloc(&self, p: Position) -> &str {
        match self.blocs.get(&p) {
            Some(bloc) => bloc,
            None if p.1 >= -60 => "minecraft:air",
            None => "minecraft:grass_block",
        }
    }

    fn recues(&self) -> Vec<String> {
        self.recues.borrow().clone()
    }
}

#[async_trait(?Send)]
impl AdaptateurJeu for FauxMonde {
    async fn commande(&self, commande: &str) -> Result<String, ErreurRcon> {
        self.recues.borrow_mut().push(commande.to_owned());
        if commande.starts_with("data get storage") {
            return Ok(match self.pos {
                None => "Found no elements matching pos".to_owned(),
                Some((x, y, z)) => format!(
                    "Storage mine_de_rien:atelier has the following contents: [{x}, {y}, {z}]"
                ),
            });
        }
        if commande.starts_with("forceload") {
            return Ok("Marked 2 chunks to be force loaded".to_owned());
        }
        if let Some(m) = SI_BLOC.captures(commande) {
            let p = (
                m[1].parse().unwrap(),
                m[2].parse().unwrap(),
                m[3].parse().unwrap(),
            );
            let (attendu, bloc) = (&m[4], self.bloc(p));
            let dans_le_tag = matches!(
                (attendu, bloc),
                ("#minecraft:dirt", "minecraft:dirt")
                    | ("#minecraft:anvil", "minecraft:anvil")
                    | ("#minecraft:standing_signs", "minecraft:oak_sign")
            );
            return Ok(if bloc == attendu || dans_le_tag {
                "Test passed"
            } else {
                "Test failed"
            }
            .to_owned());
        }
        Ok(String::new())
    }

    async fn dire(&self, _: &str, _: &str, _: &[(&str, Style<'_>)]) -> Result<(), ErreurRcon> {
        Ok(())
    }

    fn chat(&self) -> Result<FluxChat, ErreurRcon> {
        Err(ErreurRcon("pas de chat ici".to_owned()))
    }
}

const A: Atelier = Atelier { x: 0, y: -60, z: 8 };

fn refus(resultat: Result<String, ErreurAtelier>) -> String {
    match resultat {
        Err(ErreurAtelier::Refus(message)) => message,
        autre => panic!("un refus était attendu, pas {autre:?}"),
    }
}

#[test]
fn commandes_pour_des_coordonnees_fixees() {
    assert_eq!(commande_forceload(&A), "forceload add -2 6 2 10");
    assert_eq!(
        commandes_construction(&A),
        [
            "fill -2 -61 6 2 -61 10 minecraft:smooth_stone",
            "execute unless block 0 -60 8 minecraft:chest run setblock 0 -60 8 minecraft:chest[facing=south]",
            "execute unless block 1 -60 8 #minecraft:anvil run setblock 1 -60 8 minecraft:anvil[facing=east]",
            "execute unless block 2 -60 6 minecraft:lantern run setblock 2 -60 6 minecraft:lantern",
            "kill @e[type=minecraft:marker,tag=mdr_atelier]",
            "summon minecraft:marker 0.5 -60 8.5 {Tags:[\"mdr_atelier\"]}",
            "data modify storage mine_de_rien:atelier pos set value [0, -60, 8]",
        ]
    );
}

#[test]
fn marqueur_au_centre_du_bloc_en_negatif() {
    let commandes = commandes_construction(&Atelier::new(-3, 64, -7));
    let summon: Vec<&String> = commandes.iter().filter(|c| c.contains("summon")).collect();
    assert_eq!(summon.len(), 1);
    assert!(summon[0].contains("marker -2.5 64 -6.5 "));
}

#[test]
fn volume_verifie() {
    let positions: Vec<Position> = verifications_de_place(&A, false)
        .into_iter()
        .map(|(p, _)| p)
        .collect();
    assert_eq!(positions.len(), 5 * 5 * 3 + 5 * 5);
    for p in [(-2, -60, 6), (2, -58, 10), (0, -61, 8)] {
        assert!(positions.contains(&p));
    }
}

#[tokio::test]
async fn construit_sur_un_terrain_libre() {
    let monde = FauxMonde::default();
    assert_eq!(
        construire(&monde, &A).await.unwrap(),
        "Atelier construit en 0 -60 8."
    );
    assert!(
        monde
            .recues()
            .last()
            .unwrap()
            .starts_with("data modify storage")
    );
}

#[tokio::test]
async fn refus_si_un_bloc_gene_sans_rien_casser() {
    let monde = FauxMonde::avec(&[((1, -59, 9), "minecraft:oak_planks")]);
    assert!(refus(construire(&monde, &A).await).contains("1 -59 9"));
    let recues = monde.recues();
    assert!(!recues.iter().any(|c| {
        ["fill", "setblock", "summon", "kill"]
            .iter()
            .any(|debut| c.starts_with(debut))
    }));
    assert!(recues.last().unwrap().starts_with("forceload remove"));
}

#[tokio::test]
async fn refus_si_le_plancher_est_une_construction() {
    let monde = FauxMonde::avec(&[((0, -61, 8), "minecraft:oak_planks")]);
    assert!(refus(construire(&monde, &A).await).contains("0 -61 8"));
}

#[tokio::test]
async fn le_coffre_d_autrui_n_est_pas_pris_pour_le_notre() {
    let monde = FauxMonde::avec(&[((0, -60, 8), "minecraft:chest")]);
    assert!(refus(construire(&monde, &A).await).contains("0 -60 8"));
}

#[tokio::test]
async fn relance_reconnait_ses_propres_blocs() {
    let mut blocs: Vec<(Position, &'static str)> = vec![
        ((0, -60, 8), "minecraft:chest"),
        ((1, -60, 8), "minecraft:anvil"),
        ((-1, -60, 8), "minecraft:oak_sign"),
        ((2, -60, 6), "minecraft:lantern"),
    ];
    for x in -2..3 {
        for z in 6..11 {
            blocs.push(((x, -61, z), "minecraft:smooth_stone"));
        }
    }
    let monde = FauxMonde {
        pos: Some((0, -60, 8)),
        ..FauxMonde::avec(&blocs)
    };
    assert_eq!(
        construire(&monde, &A).await.unwrap(),
        "Atelier réparé en 0 -60 8."
    );
    // Un seul marqueur : l'ancien est retiré avant d'en poser un nouveau.
    let recues = monde.recues();
    assert!(
        recues
            .iter()
            .find(|c| c.contains("marker"))
            .unwrap()
            .starts_with("kill")
    );
}

#[tokio::test]
async fn refus_si_l_atelier_est_ailleurs() {
    let monde = FauxMonde {
        pos: Some((100, 64, 100)),
        ..FauxMonde::default()
    };
    assert!(refus(construire(&monde, &A).await).contains("100 64 100"));
}

#[tokio::test]
async fn position_enregistree_illisible_vaut_absence() {
    struct Reponse(&'static str);
    #[async_trait(?Send)]
    impl AdaptateurJeu for Reponse {
        async fn commande(&self, _: &str) -> Result<String, ErreurRcon> {
            Ok(self.0.to_owned())
        }
        async fn dire(&self, _: &str, _: &str, _: &[(&str, Style<'_>)]) -> Result<(), ErreurRcon> {
            Ok(())
        }
        fn chat(&self) -> Result<FluxChat, ErreurRcon> {
            Err(ErreurRcon(String::new()))
        }
    }
    for reponse in [
        "",
        "contents: [1, 2]",
        "contents: [a, b, c]",
        "] à l'envers [",
        "contents: [",
    ] {
        assert_eq!(
            atelier::lire_atelier(&Reponse(reponse)).await.unwrap(),
            None,
            "{reponse}"
        );
    }
    assert_eq!(
        atelier::lire_atelier(&Reponse("contents: [1.0d, -60d, 8]"))
            .await
            .unwrap(),
        None,
        "une coordonnée à virgule n'est pas une position de bloc"
    );
    assert_eq!(
        atelier::lire_atelier(&Reponse("contents: [1d, -60d, 8]"))
            .await
            .unwrap(),
        Some(Atelier::new(1, -60, 8))
    );
}

/// Sur huit mondes, les mêmes commandes dans le même ordre que la version Python, et le
/// même compte rendu ou le même refus.
#[tokio::test]
async fn oracle_python() {
    for cas in commun::oracle("atelier") {
        let e = &cas["entree"];
        let rejeu = commun::Rejeu::new(&cas);
        let atelier = || {
            let c = &e["atelier"];
            let n = |i: usize| c[i].as_i64().unwrap() as i32;
            Atelier::new(n(0), n(1), n(2))
        };
        match e["f"].as_str().unwrap() {
            "construire" => commun::conclure(&cas, &rejeu, construire(&rejeu, &atelier()).await),
            "demonter" => commun::conclure(
                &cas,
                &rejeu,
                atelier::demonter(&rejeu, &atelier())
                    .await
                    .map(|()| json!(null)),
            ),
            "place_libre" => {
                commun::conclure(&cas, &rejeu, atelier::place_libre(&rejeu, &atelier()).await)
            }
            "lire_atelier" => commun::conclure(
                &cas,
                &rejeu,
                atelier::lire_atelier(&rejeu)
                    .await
                    .map(|a| a.map(|a| [a.x, a.y, a.z])),
            ),
            autre => panic!("fonction inconnue : {autre}"),
        }
    }
}
