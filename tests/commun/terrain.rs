//! Un terrain simulé qui répond comme le serveur 26.3, partagé par les tests de l'emplacement
//! et ceux de la commande `/atelier`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::LazyLock;

use async_trait::async_trait;
use mine_de_rien::adapters::game::base::AdaptateurJeu;
use mine_de_rien::adapters::game::log::FluxChat;
use mine_de_rien::adapters::game::rcon::ErreurRcon;
use mine_de_rien::minecraft::texte::Style;
use regex::Regex;

pub const X: i32 = 100;
pub const Y: i32 = 64;
pub const Z: i32 = 200;
pub const PSEUDO: &str = "Steve_42";

static HAUTEUR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^execute positioned (-?\d+) 0 (-?\d+) positioned over").unwrap());
static SI_BLOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^execute if block (-?\d+) (-?\d+) (-?\d+) (\S+)$").unwrap());

/// Sol à `Y` partout, sauf les colonnes de `sols` ou ce que dit `relief` ; `blocs` pour le reste.
pub struct Terrain {
    pub sols: HashMap<(i32, i32), i32>,
    pub relief: Option<fn(i32, i32) -> i32>,
    pub blocs: HashMap<(i32, i32, i32), &'static str>,
    pub charge: fn(i32, i32) -> bool,
    pub summon: &'static str,
    pub joueur: Option<(f64, f64, f64)>,
    pub dimension: bool,
    /// Le serveur tombe à la première lecture de hauteur.
    pub coupure: bool,
    /// La position de l'atelier enregistrée dans le monde, s'il y en a un.
    pub atelier: RefCell<Option<[i32; 3]>>,
    /// Le coffre de l'atelier contient encore quelque chose.
    pub coffre_plein: bool,
    pub sondes: Cell<i32>,
    pub recues: RefCell<Vec<String>>,
    pub dits: RefCell<Vec<(String, String)>>,
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
            atelier: RefCell::new(None),
            coffre_plein: false,
            sondes: Cell::new(0),
            recues: RefCell::new(Vec::new()),
            dits: RefCell::new(Vec::new()),
        }
    }
}

impl Terrain {
    pub fn sol(&self, x: i32, z: i32) -> i32 {
        match self.relief {
            Some(relief) => relief(x, z),
            None => self.sols.get(&(x, z)).copied().unwrap_or(Y),
        }
    }

    pub fn bloc(&self, x: i32, y: i32, z: i32) -> &str {
        match self.blocs.get(&(x, y, z)) {
            Some(bloc) => bloc,
            None if y >= self.sol(x, z) => "minecraft:air",
            None => "minecraft:grass_block",
        }
    }

    pub fn recues(&self) -> Vec<String> {
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
            return Ok(match *self.atelier.borrow() {
                None => "Found no elements matching pos".to_owned(),
                Some([x, y, z]) => format!(
                    "Storage mine_de_rien:atelier has the following contents: [{x}, {y}, {z}]"
                ),
            });
        }
        if c.starts_with("data remove storage") {
            *self.atelier.borrow_mut() = None;
            return ok("Modified storage mine_de_rien:atelier");
        }
        if c.starts_with("data modify storage") {
            let liste = &c[c.find('[').unwrap() + 1..c.rfind(']').unwrap()];
            let v: Vec<i32> = liste
                .split(',')
                .map(|n| n.trim().parse().unwrap())
                .collect();
            *self.atelier.borrow_mut() = Some([v[0], v[1], v[2]]);
            return ok("Modified storage mine_de_rien:atelier");
        }
        if c.contains("Items[0]") {
            return ok(if self.coffre_plein {
                "Test passed. Count: 1"
            } else {
                "Test failed"
            });
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
