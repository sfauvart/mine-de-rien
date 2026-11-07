"""Stockage local SQLite : compétences, exercices posés, tentatives, rapports."""

from __future__ import annotations

import sqlite3
from pathlib import Path

SCHEMA = """
CREATE TABLE IF NOT EXISTS exercices (
    id          INTEGER PRIMARY KEY,
    competence  TEXT NOT NULL,
    enonce      TEXT NOT NULL,
    reponse     TEXT NOT NULL,
    jeu         TEXT NOT NULL,
    pose_le     TEXT NOT NULL,
    resolu_le   TEXT
);
CREATE TABLE IF NOT EXISTS tentatives (
    id          INTEGER PRIMARY KEY,
    exercice_id INTEGER NOT NULL REFERENCES exercices(id),
    proposition TEXT NOT NULL,
    correcte    INTEGER NOT NULL,
    faite_le    TEXT NOT NULL
);
"""


def ouvrir(chemin: Path | str) -> sqlite3.Connection:
    connexion = sqlite3.connect(chemin)
    connexion.executescript(SCHEMA)
    return connexion
