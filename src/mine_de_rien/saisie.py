"""Ce que tape un parent après `/coffre` devient une liste d'exercices, ou un message d'erreur.

Deux formes :
- une expression recopiée du cahier (`348 ÷ 12`), recalculée par Mine de rien ;
- un nom de compétence suivi d'un nombre d'exercices (`division 3`).
"""

from __future__ import annotations

import random
import re
from dataclasses import dataclass
from fractions import Fraction

from mine_de_rien.exercises import registre
from mine_de_rien.exercises.base import Exercice
from mine_de_rien.exercises.expression import ErreurExpression, calculer, ecrire_reponse
from mine_de_rien.minecraft.texte import lignes_panneau

NOMBRE_MAX = 10
_ESSAIS_PAR_EXERCICE = 50


class ErreurSaisie(ValueError):
    """Saisie refusée ; le message s'adresse au parent, tel quel."""


@dataclass(frozen=True)
class Saisie:
    exercices: list[Exercice]
    source: str  # "expression" ou "competence"


# Clé en mots : minuscules (accents compris), espaces et tirets simples, 50 caractères au plus
# comme le champ de l'enclume. Jamais de majuscule : le verrou compare lettre à lettre.
_CLE_MOTS = re.compile(r"[a-zà-öø-ÿœæ]+(?:[ -][a-zà-öø-ÿœæ]+)*")
LONGUEUR_CLE = 50


def cle_valide(reponse: str) -> bool:
    """Une clé de coffre a une écriture unique : nombre écrit par `ecrire_reponse`, ou mots."""
    if len(reponse) > LONGUEUR_CLE:
        return False
    if _CLE_MOTS.fullmatch(reponse):
        return True
    try:
        return ecrire_reponse(Fraction(reponse.replace(",", "."))) == reponse
    except ValueError:  # y compris ErreurExpression
        return False


def _verifier_cle(ex: Exercice) -> None:
    """Garde-fou : la réponse d'un générateur doit avoir l'écriture unique du coffre."""
    if not cle_valide(ex.reponse):
        raise RuntimeError(f"réponse de générateur inutilisable dans un coffre : {ex!r}")
    if lignes_panneau(ex.enonce) is None:
        raise RuntimeError(f"énoncé de générateur trop long pour un panneau : {ex!r}")


def _competence(texte: str, rng: random.Random) -> Saisie:
    m = re.fullmatch(r"\s*(\S+)(?:\s+(\d+))?\s*", texte)
    entree = registre.chercher(m.group(1)) if m else None
    if entree is None:
        raise ErreurSaisie(
            "Je ne reconnais ni un calcul ni une compétence. "
            f"Compétences disponibles : {', '.join(registre.noms())}. "
            "Exemples : « 348 ÷ 12 », « division 3 » ou « anglais 3 »."
        )
    nombre = int(m.group(2)) if m.group(2) else 1
    if not 1 <= nombre <= NOMBRE_MAX:
        raise ErreurSaisie(f"Entre 1 et {NOMBRE_MAX} exercices à la fois.")
    exercices: list[Exercice] = []
    vus: set[str] = set()
    for _ in range(nombre * _ESSAIS_PAR_EXERCICE):
        if len(exercices) == nombre:
            break
        ex = entree.generer(rng, entree.niveau)
        if ex.enonce in vus:
            continue
        _verifier_cle(ex)
        vus.add(ex.enonce)
        exercices.append(ex)
    return Saisie(exercices, "competence")


def interpreter(texte: str, rng: random.Random | None = None) -> Saisie:
    texte = texte.strip()
    if not texte:
        raise ErreurSaisie("Que faut-il mettre dans le coffre ? Exemple : « /coffre 348 ÷ 12 ».")
    # Une saisie qui commence par une lettre est un nom de compétence, sinon un calcul.
    if texte[0].isalpha():
        return _competence(texte, rng or random.Random())
    try:
        return Saisie([calculer(texte)], "expression")
    except ErreurExpression as e:
        raise ErreurSaisie(str(e)) from e
