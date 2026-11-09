"""Lecture de config.toml, avec substitution des ${VARIABLES} d'environnement."""

from __future__ import annotations

import os
import re
import tomllib
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any

_VARIABLE = re.compile(r"\$\{([A-Z0-9_]+)\}")
_OBJET = re.compile(r"(minecraft:)?[a-z0-9_]+")
_NOM_ASSISTANT = re.compile(r"[A-Za-zÀ-ÖØ-öø-ÿ]{2,16}")


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
    """Facultatif : où `mine-de-rien atelier` construit. Ensuite, le monde fait foi."""

    x: int
    y: int
    z: int


@dataclass
class Coffre:
    """Récompense identique pour tous les coffres, et niveaux offerts pour les essais."""

    recompense: str = "minecraft:diamond"
    recompense_nombre: int = 3
    niveaux_offerts: int = 3

    def __post_init__(self) -> None:
        if not _OBJET.fullmatch(self.recompense):
            raise ValueError(f"[coffre] recompense invalide : {self.recompense!r}")
        if not 1 <= self.recompense_nombre <= 64:
            raise ValueError("[coffre] recompense_nombre doit être entre 1 et 64")
        if not 0 <= self.niveaux_offerts <= 30:
            raise ValueError("[coffre] niveaux_offerts doit être entre 0 et 30")


@dataclass
class Assistant:
    """Pixel, que l'enfant appelle dans le chat par son nom."""

    nom: str = "Pixel"
    actif: bool = True

    def __post_init__(self) -> None:
        if not _NOM_ASSISTANT.fullmatch(self.nom):
            raise ValueError(f"[assistant] nom : 2 à 16 lettres, sans espace ({self.nom!r})")


@dataclass
class Config:
    enfant: Enfant
    jeu: Jeu
    canal: Canal
    llm: Llm
    stockage: Stockage
    atelier: Atelier | None = None
    coffre: Coffre = field(default_factory=Coffre)
    assistant: Assistant = field(default_factory=Assistant)


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
        coffre=Coffre(**brut.get("coffre", {})),
        assistant=Assistant(**brut.get("assistant", {})),
    )
