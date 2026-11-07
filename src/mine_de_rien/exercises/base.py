from __future__ import annotations

import random
import re
import unicodedata
from dataclasses import dataclass, field
from typing import Protocol


def normaliser(texte: str) -> str:
    """Compare les réponses sans tenir compte de la casse, des accents ni des espaces."""
    sans_accents = unicodedata.normalize("NFKD", texte).encode("ascii", "ignore").decode()
    return re.sub(r"\s+", "", sans_accents).lower()


@dataclass(frozen=True)
class Exercice:
    competence: str      # ex. "maths/division-posee"
    enonce: str          # texte neutre, sans habillage
    reponse: str         # la clé exacte attendue (c'est elle qui verrouille le coffre)
    donnees: dict = field(default_factory=dict)   # matière pour l'habillage par le LLM

    def accepte(self, proposition: str) -> bool:
        return normaliser(proposition) == normaliser(self.reponse)


class Generateur(Protocol):
    competence: str

    def generer(self, rng: random.Random, niveau: int = 1) -> Exercice: ...
