//! Stockage local SQLite : exercices, leur statut et la file d'attente des coffres.
//!
//! Un exercice passe par `en_file` → `pose` → `ouvert`, ou finit `annule`. Un seul exercice est
//! `pose` à la fois ; les autres attendent dans l'ordre de leur demande.
//!
//! Le schéma est daté par `PRAGMA user_version`. Les dates sont en UTC, au format ISO 8601, à
//! la seconde, écrites `2026-11-08T18:45:26+00:00` : c'est le format de la version Python,
//! gardé tel quel pour que les bases existantes se rouvrent et que les comparaisons de dates
//! en SQL, qui se font sur le texte, restent justes.

use std::path::Path;

use chrono::{DateTime, NaiveDate, SecondsFormat, TimeDelta, Utc};
use rusqlite::{Connection, OptionalExtension, Row, params};
use serde_json::{Map, Value};

use crate::exercises::base::Exercice;
use crate::saisie::Source;
use crate::util::json_python;

pub const VERSION_SCHEMA: u32 = 5;

pub const SCHEMA: &str = r#"
CREATE TABLE exercices (
    id                INTEGER PRIMARY KEY,
    competence        TEXT NOT NULL,
    enonce            TEXT NOT NULL,
    reponse           TEXT NOT NULL,
    donnees           TEXT NOT NULL DEFAULT '{}',
    jeu               TEXT NOT NULL,
    source            TEXT NOT NULL CHECK (source IN ('expression', 'competence')),
    statut            TEXT NOT NULL DEFAULT 'en_file'
                      CHECK (statut IN ('en_file', 'pose', 'ouvert', 'annule')),
    demande_par       TEXT NOT NULL,
    recompense        TEXT NOT NULL,
    recompense_nombre INTEGER NOT NULL,
    cree_le           TEXT NOT NULL,
    pose_le           TEXT,
    ouvert_le         TEXT
);
CREATE INDEX exercices_statut ON exercices (statut, id);
CREATE TABLE tentatives (
    id          INTEGER PRIMARY KEY,
    exercice_id INTEGER NOT NULL REFERENCES exercices(id),
    proposition TEXT NOT NULL,
    correcte    INTEGER NOT NULL,
    faite_le    TEXT NOT NULL
);
"#;

/// Chaque entrée fait passer le schéma de la version `indice + 1` à `indice + 2`.
pub const MIGRATIONS: [&str; 4] = [
    // 2 : niveaux d'expérience donnés pour le coffre, et journal pour le rapport.
    r#"
    ALTER TABLE exercices ADD COLUMN niveaux_donnes INTEGER NOT NULL DEFAULT 0;
    CREATE TABLE evenements (
        id          INTEGER PRIMARY KEY,
        exercice_id INTEGER REFERENCES exercices(id),
        genre       TEXT NOT NULL,
        quand       TEXT NOT NULL
    );
    "#,
    // 3 : source « photo ». SQLite ne modifie pas une contrainte CHECK : la table est recréée
    // et ses lignes recopiées (procédure de la documentation SQLite, clés étrangères coupées
    // le temps de la recopie, puis vérifiées).
    r#"
    PRAGMA foreign_keys = OFF;
    BEGIN;
    CREATE TABLE exercices_v3 (
        id                INTEGER PRIMARY KEY,
        competence        TEXT NOT NULL,
        enonce            TEXT NOT NULL,
        reponse           TEXT NOT NULL,
        donnees           TEXT NOT NULL DEFAULT '{}',
        jeu               TEXT NOT NULL,
        source            TEXT NOT NULL CHECK (source IN ('expression', 'competence', 'photo')),
        statut            TEXT NOT NULL DEFAULT 'en_file'
                          CHECK (statut IN ('en_file', 'pose', 'ouvert', 'annule')),
        demande_par       TEXT NOT NULL,
        recompense        TEXT NOT NULL,
        recompense_nombre INTEGER NOT NULL,
        cree_le           TEXT NOT NULL,
        pose_le           TEXT,
        ouvert_le         TEXT,
        niveaux_donnes    INTEGER NOT NULL DEFAULT 0
    );
    INSERT INTO exercices_v3 (id, competence, enonce, reponse, donnees, jeu, source, statut,
            demande_par, recompense, recompense_nombre, cree_le, pose_le, ouvert_le,
            niveaux_donnes)
        SELECT id, competence, enonce, reponse, donnees, jeu, source, statut, demande_par,
            recompense, recompense_nombre, cree_le, pose_le, ouvert_le, niveaux_donnes
        FROM exercices;
    DROP TABLE exercices;
    ALTER TABLE exercices_v3 RENAME TO exercices;
    CREATE INDEX exercices_statut ON exercices (statut, id);
    COMMIT;
    PRAGMA foreign_keys = ON;
    "#,
    // 4 : source « dictee ». Même recopie que pour la version 3.
    r#"
    PRAGMA foreign_keys = OFF;
    BEGIN;
    CREATE TABLE exercices_v4 (
        id                INTEGER PRIMARY KEY,
        competence        TEXT NOT NULL,
        enonce            TEXT NOT NULL,
        reponse           TEXT NOT NULL,
        donnees           TEXT NOT NULL DEFAULT '{}',
        jeu               TEXT NOT NULL,
        source            TEXT NOT NULL
                          CHECK (source IN ('expression', 'competence', 'photo', 'dictee')),
        statut            TEXT NOT NULL DEFAULT 'en_file'
                          CHECK (statut IN ('en_file', 'pose', 'ouvert', 'annule')),
        demande_par       TEXT NOT NULL,
        recompense        TEXT NOT NULL,
        recompense_nombre INTEGER NOT NULL,
        cree_le           TEXT NOT NULL,
        pose_le           TEXT,
        ouvert_le         TEXT,
        niveaux_donnes    INTEGER NOT NULL DEFAULT 0
    );
    INSERT INTO exercices_v4 (id, competence, enonce, reponse, donnees, jeu, source, statut,
            demande_par, recompense, recompense_nombre, cree_le, pose_le, ouvert_le,
            niveaux_donnes)
        SELECT id, competence, enonce, reponse, donnees, jeu, source, statut, demande_par,
            recompense, recompense_nombre, cree_le, pose_le, ouvert_le, niveaux_donnes
        FROM exercices;
    DROP TABLE exercices;
    ALTER TABLE exercices_v4 RENAME TO exercices;
    CREATE INDEX exercices_statut ON exercices (statut, id);
    COMMIT;
    PRAGMA foreign_keys = ON;
    "#,
    // 5 : les dictées préparées. Une dictée est gardée jusqu'à son jour ; chacune de ses
    // phrases porte ses deux sons (Ogg Vorbis, vitesse normale et lente). Le numéro d'une
    // phrase donne ceux de ses sons (`dictees::numero_son`).
    r#"
    CREATE TABLE dictees (
        id              INTEGER PRIMARY KEY,
        jour            TEXT NOT NULL,
        texte           TEXT NOT NULL,
        imposes         TEXT NOT NULL DEFAULT '[]',
        statut          TEXT NOT NULL DEFAULT 'en_cours'
                        CHECK (statut IN ('en_cours', 'annulee')),
        demande_par     TEXT NOT NULL,
        confirmee_le    TEXT NOT NULL,
        derniere_seance TEXT
    );
    CREATE TABLE dictee_phrases (
        id        INTEGER PRIMARY KEY,
        dictee_id INTEGER NOT NULL REFERENCES dictees(id),
        rang      INTEGER NOT NULL,
        texte     TEXT NOT NULL,
        son       BLOB NOT NULL,
        son_lent  BLOB NOT NULL
    );
    CREATE INDEX dictee_phrases_dictee ON dictee_phrases (dictee_id, rang);
    "#,
];

pub const JEU_PAR_DEFAUT: &str = "coffre-du-soir";

#[derive(Debug, thiserror::Error)]
pub enum ErreurStockage {
    #[error("{0}")]
    Refus(String),
    #[error("base SQLite : {0}")]
    Sqlite(#[from] rusqlite::Error),
}

fn refus<T>(message: impl Into<String>) -> Result<T, ErreurStockage> {
    Err(ErreurStockage::Refus(message.into()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Statut {
    EnFile,
    Pose,
    Ouvert,
    Annule,
}

impl Statut {
    pub const TOUS: [Statut; 4] = [Self::EnFile, Self::Pose, Self::Ouvert, Self::Annule];

    pub fn nom(self) -> &'static str {
        match self {
            Self::EnFile => "en_file",
            Self::Pose => "pose",
            Self::Ouvert => "ouvert",
            Self::Annule => "annule",
        }
    }

    pub fn lire(nom: &str) -> Option<Self> {
        Self::TOUS.into_iter().find(|s| s.nom() == nom)
    }
}

pub fn maintenant() -> DateTime<Utc> {
    Utc::now()
}

fn iso(quand: DateTime<Utc>) -> String {
    quand.to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn date(texte: Option<String>) -> Result<Option<DateTime<Utc>>, ErreurStockage> {
    match texte.as_deref() {
        None | Some("") => Ok(None),
        Some(texte) => match DateTime::parse_from_rfc3339(texte) {
            Ok(quand) => Ok(Some(quand.with_timezone(&Utc))),
            Err(e) => refus(format!("date illisible dans la base ({texte}) : {e}")),
        },
    }
}

fn date_requise(texte: String) -> Result<DateTime<Utc>, ErreurStockage> {
    match date(Some(texte))? {
        Some(quand) => Ok(quand),
        None => refus("date vide dans la base"),
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExerciceStocke {
    pub id: i64,
    pub exercice: Exercice,
    pub jeu: String,
    pub source: Source,
    pub statut: Statut,
    pub demande_par: String,
    pub recompense: String,
    pub recompense_nombre: i64,
    pub cree_le: DateTime<Utc>,
    pub pose_le: Option<DateTime<Utc>>,
    pub ouvert_le: Option<DateTime<Utc>>,
    pub niveaux_donnes: bool,
}

impl ExerciceStocke {
    /// Temps entre la pose et l'ouverture, ou `None`.
    pub fn duree(&self) -> Option<TimeDelta> {
        Some(self.ouvert_le? - self.pose_le?)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Evenement {
    pub exercice_id: Option<i64>,
    pub genre: String,
    pub quand: DateTime<Utc>,
}

/// Ce que l'enfant a écrit sur la clé devant un coffre : un essai, juste ou non.
#[derive(Debug, Clone, PartialEq)]
pub struct Tentative {
    pub proposition: String,
    pub correcte: bool,
    pub faite_le: DateTime<Utc>,
}

/// Une dictée à préparer, telle que le parent l'a confirmée.
#[derive(Debug, Clone, PartialEq)]
pub struct DicteeStockee {
    pub id: i64,
    /// Le jour de la dictée à l'école.
    pub jour: NaiveDate,
    pub texte: String,
    /// Les mots que le parent veut voir sortir en priorité.
    pub imposes: Vec<String>,
    pub annulee: bool,
    pub demande_par: String,
    pub confirmee_le: DateTime<Utc>,
    /// Le jour (de la maison) de la dernière séance créée pour cette dictée.
    pub derniere_seance: Option<NaiveDate>,
}

/// Une phrase d'une dictée. Son `id` donne les numéros de ses sons.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhraseStockee {
    pub id: i64,
    pub rang: usize,
    pub texte: String,
}

/// Une phrase à enregistrer, avec ses deux sons en Ogg Vorbis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhraseSonore {
    pub texte: String,
    pub son: Vec<u8>,
    pub son_lent: Vec<u8>,
}

fn jour(texte: &str) -> Result<NaiveDate, ErreurStockage> {
    NaiveDate::parse_from_str(texte, "%Y-%m-%d")
        .or_else(|e| refus(format!("jour illisible dans la base ({texte}) : {e}")))
}

/// Le numéro d'une phrase et ses deux sons, à vitesse normale puis lente.
pub type SonsDePhrase = (i64, Vec<u8>, Vec<u8>);

const COLONNES_DICTEE: &str =
    "id, jour, texte, imposes, statut, demande_par, confirmee_le, derniere_seance";

type DicteeBrute = (
    i64,
    String,
    String,
    String,
    String,
    String,
    String,
    Option<String>,
);

fn dictee_brute(l: &Row<'_>) -> rusqlite::Result<DicteeBrute> {
    Ok((
        l.get(0)?,
        l.get(1)?,
        l.get(2)?,
        l.get(3)?,
        l.get(4)?,
        l.get(5)?,
        l.get(6)?,
        l.get(7)?,
    ))
}

fn dictee_verifiee(brute: DicteeBrute) -> Result<DicteeStockee, ErreurStockage> {
    let (id, le_jour, texte, imposes, statut, demande_par, confirmee_le, derniere) = brute;
    let imposes: Vec<String> = serde_json::from_str(&imposes)
        .or_else(|_| refus(format!("mots imposés illisibles pour la dictée {id}")))?;
    Ok(DicteeStockee {
        id,
        jour: jour(&le_jour)?,
        texte,
        imposes,
        annulee: statut != "en_cours",
        demande_par,
        confirmee_le: date_requise(confirmee_le)?,
        derniere_seance: derniere.as_deref().map(jour).transpose()?,
    })
}

/// Ce qui accompagne un exercice mis en file : d'où il vient, qui l'a demandé, le butin.
#[derive(Debug, Clone, Copy)]
pub struct Demande<'a> {
    pub source: Source,
    pub demande_par: &'a str,
    pub recompense: &'a str,
    pub recompense_nombre: i64,
}

pub fn ouvrir(chemin: &Path) -> Result<Connection, ErreurStockage> {
    let connexion = Connection::open(chemin)?;
    connexion.pragma_update(None, "foreign_keys", "ON")?;
    // L'image de l'agent ne contient que le binaire : pas de /tmp pour les fichiers
    // temporaires de SQLite.
    connexion.pragma_update(None, "temp_store", "MEMORY")?;
    let mut version: u32 = connexion.query_row("PRAGMA user_version", [], |l| l.get(0))?;
    if version == 0 {
        let existantes: i64 = connexion.query_row(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table'",
            [],
            |l| l.get(0),
        )?;
        if existantes > 0 {
            return refus(format!(
                "{} a été créée par une version antérieure sans schéma daté : \
                 aucune donnée réelle n'existait alors, supprimer le fichier.",
                chemin.display()
            ));
        }
        connexion.execute_batch(SCHEMA)?;
        version = 1;
    } else if version > VERSION_SCHEMA {
        return refus(format!(
            "{} vient d'une version plus récente de Mine de rien.",
            chemin.display()
        ));
    }
    for script in &MIGRATIONS[(version - 1) as usize..] {
        connexion.execute_batch(script)?;
    }
    connexion.pragma_update(None, "user_version", VERSION_SCHEMA)?;
    Ok(connexion)
}

const COLONNES: &str = "id, competence, enonce, reponse, donnees, jeu, source, statut, \
    demande_par, recompense, recompense_nombre, cree_le, pose_le, ouvert_le, niveaux_donnes";

/// Une ligne telle qu'elle sort de SQLite, avant d'être vérifiée.
struct Brut {
    id: i64,
    competence: String,
    enonce: String,
    reponse: String,
    donnees: String,
    jeu: String,
    source: String,
    statut: String,
    demande_par: String,
    recompense: String,
    recompense_nombre: i64,
    cree_le: String,
    pose_le: Option<String>,
    ouvert_le: Option<String>,
    niveaux_donnes: i64,
}

impl Brut {
    fn lire(ligne: &Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            id: ligne.get(0)?,
            competence: ligne.get(1)?,
            enonce: ligne.get(2)?,
            reponse: ligne.get(3)?,
            donnees: ligne.get(4)?,
            jeu: ligne.get(5)?,
            source: ligne.get(6)?,
            statut: ligne.get(7)?,
            demande_par: ligne.get(8)?,
            recompense: ligne.get(9)?,
            recompense_nombre: ligne.get(10)?,
            cree_le: ligne.get(11)?,
            pose_le: ligne.get(12)?,
            ouvert_le: ligne.get(13)?,
            niveaux_donnes: ligne.get(14)?,
        })
    }

    fn verifier(self) -> Result<ExerciceStocke, ErreurStockage> {
        let donnees: Map<String, Value> = match serde_json::from_str(&self.donnees) {
            Ok(Value::Object(objet)) => objet,
            _ => return refus(format!("données illisibles pour l'exercice {}", self.id)),
        };
        let Some(source) = Source::lire(&self.source) else {
            return refus(format!(
                "source inconnue pour l'exercice {} : {}",
                self.id, self.source
            ));
        };
        let Some(statut) = Statut::lire(&self.statut) else {
            return refus(format!(
                "statut inconnu pour l'exercice {} : {}",
                self.id, self.statut
            ));
        };
        Ok(ExerciceStocke {
            id: self.id,
            exercice: Exercice {
                competence: self.competence,
                enonce: self.enonce,
                reponse: self.reponse,
                donnees,
            },
            jeu: self.jeu,
            source,
            statut,
            demande_par: self.demande_par,
            recompense: self.recompense,
            recompense_nombre: self.recompense_nombre,
            cree_le: date_requise(self.cree_le)?,
            pose_le: date(self.pose_le)?,
            ouvert_le: date(self.ouvert_le)?,
            niveaux_donnes: self.niveaux_donnes != 0,
        })
    }
}

/// Accès aux exercices. Toutes les méthodes valident leur transaction.
pub struct Depot {
    cx: Connection,
}

impl Depot {
    pub fn ouvrir(chemin: &Path) -> Result<Self, ErreurStockage> {
        Ok(Self {
            cx: ouvrir(chemin)?,
        })
    }

    /// La connexion, pour les vérifications des tests.
    pub fn connexion(&self) -> &Connection {
        &self.cx
    }

    fn un(
        &self,
        fin_de_requete: &str,
        parametres: impl rusqlite::Params,
    ) -> Result<Option<ExerciceStocke>, ErreurStockage> {
        let requete = format!("SELECT {COLONNES} FROM exercices {fin_de_requete}");
        let brut = self
            .cx
            .query_row(&requete, parametres, Brut::lire)
            .optional()?;
        brut.map(Brut::verifier).transpose()
    }

    fn plusieurs(
        &self,
        fin_de_requete: &str,
        parametres: impl rusqlite::Params,
    ) -> Result<Vec<ExerciceStocke>, ErreurStockage> {
        let requete = format!("SELECT {COLONNES} FROM exercices {fin_de_requete}");
        let mut preparee = self.cx.prepare(&requete)?;
        let bruts = preparee.query_map(parametres, Brut::lire)?;
        let mut sortie = Vec::new();
        for brut in bruts {
            sortie.push(brut?.verifier()?);
        }
        Ok(sortie)
    }

    /// Met un exercice en file et rend son numéro.
    pub fn ajouter(
        &self,
        exercice: &Exercice,
        demande: Demande<'_>,
        quand: DateTime<Utc>,
    ) -> Result<i64, ErreurStockage> {
        self.cx.execute(
            "INSERT INTO exercices (competence, enonce, reponse, donnees, jeu, source,
                 demande_par, recompense, recompense_nombre, cree_le)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                exercice.competence,
                exercice.enonce,
                exercice.reponse,
                json_python(&Value::Object(exercice.donnees.clone())),
                JEU_PAR_DEFAUT,
                demande.source.nom(),
                demande.demande_par,
                demande.recompense,
                demande.recompense_nombre,
                iso(quand),
            ],
        )?;
        Ok(self.cx.last_insert_rowid())
    }

    pub fn lire(&self, ident: i64) -> Result<Option<ExerciceStocke>, ErreurStockage> {
        self.un("WHERE id = ?", [ident])
    }

    /// Exercices en attente, dans l'ordre où ils seront posés.
    pub fn en_file(&self) -> Result<Vec<ExerciceStocke>, ErreurStockage> {
        self.plusieurs("WHERE statut = 'en_file' ORDER BY id", [])
    }

    pub fn suivant(&self) -> Result<Option<ExerciceStocke>, ErreurStockage> {
        self.un("WHERE statut = 'en_file' ORDER BY id LIMIT 1", [])
    }

    pub fn actif(&self) -> Result<Option<ExerciceStocke>, ErreurStockage> {
        self.un("WHERE statut = 'pose' ORDER BY id LIMIT 1", [])
    }

    pub fn marquer_pose(&self, ident: i64, quand: DateTime<Utc>) -> Result<(), ErreurStockage> {
        let transaction = self.cx.unchecked_transaction()?;
        let deja: Option<i64> = transaction
            .query_row("SELECT 1 FROM exercices WHERE statut = 'pose'", [], |l| {
                l.get(0)
            })
            .optional()?;
        if deja.is_some() {
            return refus("un coffre est déjà posé");
        }
        let n = transaction.execute(
            "UPDATE exercices SET statut = 'pose', pose_le = ? WHERE id = ? AND statut = 'en_file'",
            params![iso(quand), ident],
        )?;
        transaction.commit()?;
        if n != 1 {
            return refus(format!("l'exercice {ident} n'est pas en file"));
        }
        Ok(())
    }

    pub fn marquer_ouvert(&self, ident: i64, quand: DateTime<Utc>) -> Result<(), ErreurStockage> {
        let n = self.cx.execute(
            "UPDATE exercices SET statut = 'ouvert', ouvert_le = ? WHERE id = ? AND statut = 'pose'",
            params![iso(quand), ident],
        )?;
        if n != 1 {
            return refus(format!("l'exercice {ident} n'est pas posé"));
        }
        Ok(())
    }

    /// Annule un exercice en file ou posé ; renvoie son état d'avant, ou `None`.
    pub fn annuler(&self, ident: i64) -> Result<Option<ExerciceStocke>, ErreurStockage> {
        let Some(avant) = self.lire(ident)? else {
            return Ok(None);
        };
        if !matches!(avant.statut, Statut::EnFile | Statut::Pose) {
            return Ok(None);
        }
        self.cx.execute(
            "UPDATE exercices SET statut = 'annule' WHERE id = ?",
            [ident],
        )?;
        Ok(Some(avant))
    }

    /// Exercices créés dans [debut, fin[, pour les rapports.
    pub fn entre(
        &self,
        debut: DateTime<Utc>,
        fin: DateTime<Utc>,
    ) -> Result<Vec<ExerciceStocke>, ErreurStockage> {
        self.plusieurs(
            "WHERE cree_le >= ? AND cree_le < ? ORDER BY id",
            [iso(debut), iso(fin)],
        )
    }

    pub fn marquer_niveaux_donnes(&self, ident: i64) -> Result<(), ErreurStockage> {
        self.cx.execute(
            "UPDATE exercices SET niveaux_donnes = 1 WHERE id = ?",
            [ident],
        )?;
        Ok(())
    }

    pub fn dernier_ouvert(&self) -> Result<Option<ExerciceStocke>, ErreurStockage> {
        self.un(
            "WHERE statut = 'ouvert' ORDER BY ouvert_le DESC LIMIT 1",
            [],
        )
    }

    /// Journal des événements notables pour le rapport (coffre disparu, ...).
    pub fn noter(
        &self,
        genre: &str,
        exercice_id: Option<i64>,
        quand: DateTime<Utc>,
    ) -> Result<(), ErreurStockage> {
        self.cx.execute(
            "INSERT INTO evenements (exercice_id, genre, quand) VALUES (?, ?, ?)",
            params![exercice_id, genre, iso(quand)],
        )?;
        Ok(())
    }

    /// Nombre d'événements `genre` notés pour un coffre (indices demandés, ...).
    pub fn compter(&self, genre: &str, exercice_id: i64) -> Result<usize, ErreurStockage> {
        let n: i64 = self.cx.query_row(
            "SELECT COUNT(*) FROM evenements WHERE genre = ? AND exercice_id = ?",
            params![genre, exercice_id],
            |l| l.get(0),
        )?;
        Ok(usize::try_from(n).unwrap_or_default())
    }

    pub fn evenements_entre(
        &self,
        debut: DateTime<Utc>,
        fin: DateTime<Utc>,
    ) -> Result<Vec<Evenement>, ErreurStockage> {
        let mut preparee = self.cx.prepare(
            "SELECT exercice_id, genre, quand FROM evenements \
             WHERE quand >= ? AND quand < ? ORDER BY id",
        )?;
        let lignes = preparee.query_map([iso(debut), iso(fin)], |l| {
            Ok((
                l.get::<_, Option<i64>>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, String>(2)?,
            ))
        })?;
        let mut sortie = Vec::new();
        for ligne in lignes {
            let (exercice_id, genre, quand) = ligne?;
            sortie.push(Evenement {
                exercice_id,
                genre,
                quand: date_requise(quand)?,
            });
        }
        Ok(sortie)
    }

    /// Tous les coffres tirés d'une dictée, du plus ancien au plus récent, quel que soit leur
    /// statut : l'histoire de ses mots.
    pub fn coffres_de_dictee(&self, dictee: i64) -> Result<Vec<ExerciceStocke>, ErreurStockage> {
        self.plusieurs(
            "WHERE source = 'dictee' AND json_extract(donnees, '$.dictee') = ? ORDER BY id",
            [dictee],
        )
    }

    /// Note un essai de l'enfant sur le coffre `exercice_id`.
    pub fn noter_tentative(
        &self,
        exercice_id: i64,
        proposition: &str,
        correcte: bool,
        quand: DateTime<Utc>,
    ) -> Result<(), ErreurStockage> {
        self.cx.execute(
            "INSERT INTO tentatives (exercice_id, proposition, correcte, faite_le)
             VALUES (?, ?, ?, ?)",
            params![exercice_id, proposition, correcte, iso(quand)],
        )?;
        Ok(())
    }

    /// Les essais de l'enfant sur un coffre, du premier au dernier.
    pub fn tentatives(&self, exercice_id: i64) -> Result<Vec<Tentative>, ErreurStockage> {
        let mut preparee = self.cx.prepare(
            "SELECT proposition, correcte, faite_le FROM tentatives \
             WHERE exercice_id = ? ORDER BY id",
        )?;
        let lignes = preparee.query_map([exercice_id], |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, i64>(1)?,
                l.get::<_, String>(2)?,
            ))
        })?;
        let mut sortie = Vec::new();
        for ligne in lignes {
            let (proposition, correcte, faite_le) = ligne?;
            sortie.push(Tentative {
                proposition,
                correcte: correcte != 0,
                faite_le: date_requise(faite_le)?,
            });
        }
        Ok(sortie)
    }

    /// Enregistre une dictée confirmée et ses phrases, d'un seul tenant. Rend son numéro.
    pub fn ajouter_dictee(
        &self,
        le_jour: NaiveDate,
        texte: &str,
        phrases: &[PhraseSonore],
        demande_par: &str,
        quand: DateTime<Utc>,
    ) -> Result<i64, ErreurStockage> {
        let transaction = self.cx.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO dictees (jour, texte, demande_par, confirmee_le) VALUES (?, ?, ?, ?)",
            params![le_jour.to_string(), texte, demande_par, iso(quand)],
        )?;
        let ident = transaction.last_insert_rowid();
        for (rang, phrase) in phrases.iter().enumerate() {
            transaction.execute(
                "INSERT INTO dictee_phrases (dictee_id, rang, texte, son, son_lent)
                 VALUES (?, ?, ?, ?, ?)",
                params![
                    ident,
                    i64::try_from(rang).unwrap_or_default(),
                    phrase.texte,
                    phrase.son,
                    phrase.son_lent
                ],
            )?;
        }
        transaction.commit()?;
        Ok(ident)
    }

    pub fn lire_dictee(&self, ident: i64) -> Result<Option<DicteeStockee>, ErreurStockage> {
        let brute = self
            .cx
            .query_row(
                &format!("SELECT {COLONNES_DICTEE} FROM dictees WHERE id = ?"),
                [ident],
                dictee_brute,
            )
            .optional()?;
        brute.map(dictee_verifiee).transpose()
    }

    /// Les dictées non annulées dont le jour est `depuis` ou après, la plus proche d'abord.
    pub fn dictees_depuis(&self, depuis: NaiveDate) -> Result<Vec<DicteeStockee>, ErreurStockage> {
        let mut preparee = self.cx.prepare(&format!(
            "SELECT {COLONNES_DICTEE} FROM dictees \
             WHERE statut = 'en_cours' AND jour >= ? ORDER BY jour, id"
        ))?;
        let brutes = preparee.query_map([depuis.to_string()], dictee_brute)?;
        let mut sortie = Vec::new();
        for brute in brutes {
            sortie.push(dictee_verifiee(brute?)?);
        }
        Ok(sortie)
    }

    /// Annule une dictée en cours. Faux si elle n'existe pas ou l'était déjà.
    pub fn annuler_dictee(&self, ident: i64) -> Result<bool, ErreurStockage> {
        let touchees = self.cx.execute(
            "UPDATE dictees SET statut = 'annulee' WHERE id = ? AND statut = 'en_cours'",
            [ident],
        )?;
        Ok(touchees > 0)
    }

    pub fn imposer_mots(&self, ident: i64, mots: &[String]) -> Result<(), ErreurStockage> {
        let liste = serde_json::to_string(mots)
            .or_else(|e| refus(format!("mots imposés impossibles à écrire : {e}")))?;
        self.cx.execute(
            "UPDATE dictees SET imposes = ? WHERE id = ?",
            params![liste, ident],
        )?;
        Ok(())
    }

    /// Note le jour de la dernière séance créée pour cette dictée.
    pub fn marquer_seance(&self, ident: i64, le_jour: NaiveDate) -> Result<(), ErreurStockage> {
        self.cx.execute(
            "UPDATE dictees SET derniere_seance = ? WHERE id = ?",
            params![le_jour.to_string(), ident],
        )?;
        Ok(())
    }

    /// Les phrases d'une dictée, dans l'ordre du texte, sans leurs sons.
    pub fn phrases_de(&self, dictee: i64) -> Result<Vec<PhraseStockee>, ErreurStockage> {
        let mut preparee = self.cx.prepare(
            "SELECT id, rang, texte FROM dictee_phrases WHERE dictee_id = ? ORDER BY rang",
        )?;
        let lignes = preparee.query_map([dictee], |l| {
            Ok((
                l.get::<_, i64>(0)?,
                l.get::<_, i64>(1)?,
                l.get::<_, String>(2)?,
            ))
        })?;
        let mut sortie = Vec::new();
        for ligne in lignes {
            let (id, rang, texte) = ligne?;
            sortie.push(PhraseStockee {
                id,
                rang: usize::try_from(rang).unwrap_or_default(),
                texte,
            });
        }
        Ok(sortie)
    }

    /// Les sons gardés en base : ceux des dictées non annulées dont le jour est
    /// `depuis` ou après. Chaque élément : numéro de la phrase, son normal, son lent.
    pub fn sons_depuis(&self, depuis: NaiveDate) -> Result<Vec<SonsDePhrase>, ErreurStockage> {
        let mut preparee = self.cx.prepare(
            "SELECT p.id, p.son, p.son_lent FROM dictee_phrases p \
             JOIN dictees d ON d.id = p.dictee_id \
             WHERE d.statut = 'en_cours' AND d.jour >= ? ORDER BY p.id",
        )?;
        let lignes = preparee.query_map([depuis.to_string()], |l| {
            Ok((l.get(0)?, l.get(1)?, l.get(2)?))
        })?;
        let mut sortie = Vec::new();
        for ligne in lignes {
            sortie.push(ligne?);
        }
        Ok(sortie)
    }

    /// Exercices posés dans [debut, fin[, quel que soit leur statut actuel.
    pub fn poses_entre(
        &self,
        debut: DateTime<Utc>,
        fin: DateTime<Utc>,
    ) -> Result<Vec<ExerciceStocke>, ErreurStockage> {
        self.plusieurs(
            "WHERE pose_le >= ? AND pose_le < ? ORDER BY id",
            [iso(debut), iso(fin)],
        )
    }
}
