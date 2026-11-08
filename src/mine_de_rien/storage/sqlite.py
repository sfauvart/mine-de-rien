"""Stockage local SQLite : exercices, leur statut et la file d'attente des coffres.

Un exercice passe par `en_file` → `pose` → `ouvert`, ou finit `annule`. Un seul exercice est
`pose` à la fois ; les autres attendent dans l'ordre de leur demande.

Le schéma est daté par `PRAGMA user_version`. Les dates sont en UTC, au format ISO 8601.
"""

from __future__ import annotations

import json
import sqlite3
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path

from mine_de_rien.exercises.base import Exercice

VERSION_SCHEMA = 3

SCHEMA = """
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
"""

# Chaque entrée fait passer le schéma de la version `indice + 1` à `indice + 2`.
MIGRATIONS = (
    # 2 : niveaux d'expérience donnés pour le coffre, et journal pour le rapport.
    """
    ALTER TABLE exercices ADD COLUMN niveaux_donnes INTEGER NOT NULL DEFAULT 0;
    CREATE TABLE evenements (
        id          INTEGER PRIMARY KEY,
        exercice_id INTEGER REFERENCES exercices(id),
        genre       TEXT NOT NULL,
        quand       TEXT NOT NULL
    );
    """,
    # 3 : source « photo ». SQLite ne modifie pas une contrainte CHECK : la table est recréée
    # et ses lignes recopiées (procédure de la documentation SQLite, clés étrangères coupées
    # le temps de la recopie, puis vérifiées).
    """
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
    """,
)

STATUTS = ("en_file", "pose", "ouvert", "annule")


class ErreurStockage(RuntimeError):
    pass


def maintenant() -> datetime:
    return datetime.now(UTC)


def _iso(quand: datetime) -> str:
    return quand.astimezone(UTC).isoformat(timespec="seconds")


def _date(texte: str | None) -> datetime | None:
    return datetime.fromisoformat(texte) if texte else None


@dataclass(frozen=True)
class ExerciceStocke:
    id: int
    exercice: Exercice
    jeu: str
    source: str
    statut: str
    demande_par: str
    recompense: str
    recompense_nombre: int
    cree_le: datetime
    pose_le: datetime | None
    ouvert_le: datetime | None
    niveaux_donnes: bool = False

    @property
    def duree(self):
        """Temps entre la pose et l'ouverture, ou None."""
        if self.pose_le and self.ouvert_le:
            return self.ouvert_le - self.pose_le
        return None


@dataclass(frozen=True)
class Evenement:
    exercice_id: int | None
    genre: str
    quand: datetime


def ouvrir(chemin: Path | str) -> sqlite3.Connection:
    connexion = sqlite3.connect(chemin)
    connexion.row_factory = sqlite3.Row
    connexion.execute("PRAGMA foreign_keys = ON")
    version = connexion.execute("PRAGMA user_version").fetchone()[0]
    if version == 0:
        existantes = connexion.execute(
            "SELECT count(*) FROM sqlite_master WHERE type = 'table'"
        ).fetchone()[0]
        if existantes:
            raise ErreurStockage(
                f"{chemin} a été créée par une version antérieure sans schéma daté : "
                "aucune donnée réelle n'existait alors, supprimer le fichier."
            )
        connexion.executescript(SCHEMA)
        version = 1
    elif version > VERSION_SCHEMA:
        raise ErreurStockage(f"{chemin} vient d'une version plus récente de Mine de rien.")
    for script in MIGRATIONS[version - 1 :]:
        connexion.executescript(script)
    connexion.execute(f"PRAGMA user_version = {VERSION_SCHEMA}")
    connexion.commit()
    return connexion


class Depot:
    """Accès aux exercices. Toutes les méthodes valident leur transaction."""

    def __init__(self, connexion: sqlite3.Connection) -> None:
        self.cx = connexion

    @classmethod
    def ouvrir(cls, chemin: Path | str) -> Depot:
        return cls(ouvrir(chemin))

    def fermer(self) -> None:
        self.cx.close()

    def _lire(self, ligne: sqlite3.Row) -> ExerciceStocke:
        return ExerciceStocke(
            id=ligne["id"],
            exercice=Exercice(
                competence=ligne["competence"],
                enonce=ligne["enonce"],
                reponse=ligne["reponse"],
                donnees=json.loads(ligne["donnees"]),
            ),
            jeu=ligne["jeu"],
            source=ligne["source"],
            statut=ligne["statut"],
            demande_par=ligne["demande_par"],
            recompense=ligne["recompense"],
            recompense_nombre=ligne["recompense_nombre"],
            cree_le=_date(ligne["cree_le"]),
            pose_le=_date(ligne["pose_le"]),
            ouvert_le=_date(ligne["ouvert_le"]),
            niveaux_donnes=bool(ligne["niveaux_donnes"]),
        )

    def ajouter(
        self,
        exercice: Exercice,
        *,
        source: str,
        demande_par: str,
        recompense: str,
        recompense_nombre: int,
        jeu: str = "coffre-du-soir",
        quand: datetime | None = None,
    ) -> int:
        with self.cx:
            curseur = self.cx.execute(
                """INSERT INTO exercices (competence, enonce, reponse, donnees, jeu, source,
                       demande_par, recompense, recompense_nombre, cree_le)
                   VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)""",
                (
                    exercice.competence,
                    exercice.enonce,
                    exercice.reponse,
                    json.dumps(exercice.donnees, ensure_ascii=False),
                    jeu,
                    source,
                    demande_par,
                    recompense,
                    recompense_nombre,
                    _iso(quand or maintenant()),
                ),
            )
        return curseur.lastrowid

    def lire(self, ident: int) -> ExerciceStocke | None:
        ligne = self.cx.execute("SELECT * FROM exercices WHERE id = ?", (ident,)).fetchone()
        return self._lire(ligne) if ligne else None

    def en_file(self) -> list[ExerciceStocke]:
        """Exercices en attente, dans l'ordre où ils seront posés."""
        lignes = self.cx.execute(
            "SELECT * FROM exercices WHERE statut = 'en_file' ORDER BY id"
        ).fetchall()
        return [self._lire(ligne) for ligne in lignes]

    def suivant(self) -> ExerciceStocke | None:
        ligne = self.cx.execute(
            "SELECT * FROM exercices WHERE statut = 'en_file' ORDER BY id LIMIT 1"
        ).fetchone()
        return self._lire(ligne) if ligne else None

    def actif(self) -> ExerciceStocke | None:
        ligne = self.cx.execute(
            "SELECT * FROM exercices WHERE statut = 'pose' ORDER BY id LIMIT 1"
        ).fetchone()
        return self._lire(ligne) if ligne else None

    def marquer_pose(self, ident: int, quand: datetime | None = None) -> None:
        with self.cx:
            if self.cx.execute("SELECT 1 FROM exercices WHERE statut = 'pose'").fetchone():
                raise ErreurStockage("un coffre est déjà posé")
            n = self.cx.execute(
                "UPDATE exercices SET statut = 'pose', pose_le = ? "
                "WHERE id = ? AND statut = 'en_file'",
                (_iso(quand or maintenant()), ident),
            ).rowcount
        if n != 1:
            raise ErreurStockage(f"l'exercice {ident} n'est pas en file")

    def marquer_ouvert(self, ident: int, quand: datetime | None = None) -> None:
        with self.cx:
            n = self.cx.execute(
                "UPDATE exercices SET statut = 'ouvert', ouvert_le = ? "
                "WHERE id = ? AND statut = 'pose'",
                (_iso(quand or maintenant()), ident),
            ).rowcount
        if n != 1:
            raise ErreurStockage(f"l'exercice {ident} n'est pas posé")

    def annuler(self, ident: int) -> ExerciceStocke | None:
        """Annule un exercice en file ou posé ; renvoie son état d'avant, ou None."""
        avant = self.lire(ident)
        if avant is None or avant.statut not in ("en_file", "pose"):
            return None
        with self.cx:
            self.cx.execute("UPDATE exercices SET statut = 'annule' WHERE id = ?", (ident,))
        return avant

    def entre(self, debut: datetime, fin: datetime) -> list[ExerciceStocke]:
        """Exercices créés dans [debut, fin[, pour les rapports."""
        lignes = self.cx.execute(
            "SELECT * FROM exercices WHERE cree_le >= ? AND cree_le < ? ORDER BY id",
            (_iso(debut), _iso(fin)),
        ).fetchall()
        return [self._lire(ligne) for ligne in lignes]

    def marquer_niveaux_donnes(self, ident: int) -> None:
        with self.cx:
            self.cx.execute("UPDATE exercices SET niveaux_donnes = 1 WHERE id = ?", (ident,))

    def dernier_ouvert(self) -> ExerciceStocke | None:
        ligne = self.cx.execute(
            "SELECT * FROM exercices WHERE statut = 'ouvert' ORDER BY ouvert_le DESC LIMIT 1"
        ).fetchone()
        return self._lire(ligne) if ligne else None

    def noter(self, genre: str, exercice_id: int | None = None, quand: datetime | None = None):
        """Journal des événements notables pour le rapport (coffre disparu, ...)."""
        with self.cx:
            self.cx.execute(
                "INSERT INTO evenements (exercice_id, genre, quand) VALUES (?, ?, ?)",
                (exercice_id, genre, _iso(quand or maintenant())),
            )

    def compter(self, genre: str, exercice_id: int) -> int:
        """Nombre d'événements `genre` notés pour un coffre (indices demandés, ...)."""
        (n,) = self.cx.execute(
            "SELECT COUNT(*) FROM evenements WHERE genre = ? AND exercice_id = ?",
            (genre, exercice_id),
        ).fetchone()
        return n

    def evenements_entre(self, debut: datetime, fin: datetime) -> list[Evenement]:
        lignes = self.cx.execute(
            "SELECT * FROM evenements WHERE quand >= ? AND quand < ? ORDER BY id",
            (_iso(debut), _iso(fin)),
        ).fetchall()
        return [Evenement(lg["exercice_id"], lg["genre"], _date(lg["quand"])) for lg in lignes]

    def poses_entre(self, debut: datetime, fin: datetime) -> list[ExerciceStocke]:
        """Exercices posés dans [debut, fin[, quel que soit leur statut actuel."""
        lignes = self.cx.execute(
            "SELECT * FROM exercices WHERE pose_le >= ? AND pose_le < ? ORDER BY id",
            (_iso(debut), _iso(fin)),
        ).fetchall()
        return [self._lire(ligne) for ligne in lignes]
