"""Noms de compétences que les parents peuvent taper : `/coffre division 3`.

Chaque nom désigne un générateur et le niveau utilisé dans le coffre. Seuls les niveaux dont
la réponse a une écriture unique tapable sur une enclume figurent ici (la division avec
reste, niveau 2, en est exclue).
"""

from __future__ import annotations

import random
from collections.abc import Callable
from dataclasses import dataclass

from mine_de_rien.exercises.base import Exercice, normaliser
from mine_de_rien.exercises.maths import division


@dataclass(frozen=True)
class Entree:
    nom: str
    competence: str
    niveau: int
    generer: Callable[[random.Random, int], Exercice]


ENTREES: tuple[Entree, ...] = (
    Entree("division", division.competence, 1, division.generer),
)

_PAR_NOM = {normaliser(e.nom): e for e in ENTREES}


def chercher(nom: str) -> Entree | None:
    return _PAR_NOM.get(normaliser(nom))


def noms() -> list[str]:
    return [e.nom for e in ENTREES]
