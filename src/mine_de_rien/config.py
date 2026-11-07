"""Lecture de config.toml, avec substitution des ${VARIABLES} d'environnement."""

from __future__ import annotations

import os
import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

_VARIABLE = re.compile(r"\$\{([A-Z0-9_]+)\}")


@dataclass
class Enfant:
    pseudo_minecraft: str
    alias_llm: str = "l'élève"
    classe: str = "CM2"


@dataclass
class Jeu:
    adaptateur: str = "java-rcon"
    hote: str = "localhost"
    port: int = 25575
    mot_de_passe: str = ""
    log: str = ""


@dataclass
class Canal:
    adaptateur: str = "telegram"
    jeton: str = ""
    parents: list[int] = field(default_factory=list)


@dataclass
class Llm:
    adaptateur: str = "anthropic"
    modele: str = ""
    cle: str = ""
    url: str = ""


@dataclass
class Stockage:
    sqlite: str = "mine-de-rien.sqlite"


@dataclass
class Atelier:
    """Position du coffre au centre de l'atelier, relevée avec F3 près de la base."""

    x: int
    y: int
    z: int


@dataclass
class Config:
    enfant: Enfant
    jeu: Jeu
    canal: Canal
    llm: Llm
    stockage: Stockage
    atelier: Atelier | None = None


def _substituer(valeur: Any) -> Any:
    if isinstance(valeur, str):
        return _VARIABLE.sub(lambda m: os.environ.get(m.group(1), ""), valeur)
    if isinstance(valeur, dict):
        return {k: _substituer(v) for k, v in valeur.items()}
    if isinstance(valeur, list):
        return [_substituer(v) for v in valeur]
    return valeur


def charger_config(chemin: Path) -> Config:
    with chemin.open("rb") as f:
        brut = _substituer(tomllib.load(f))
    return Config(
        enfant=Enfant(**brut["enfant"]),
        jeu=Jeu(**brut.get("jeu", {})),
        canal=Canal(**brut.get("canal", {})),
        llm=Llm(**brut.get("llm", {})),
        stockage=Stockage(**brut.get("stockage", {})),
        atelier=Atelier(**brut["atelier"]) if "atelier" in brut else None,
    )
