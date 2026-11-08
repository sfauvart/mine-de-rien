"""Vocabulaire anglais, attendu de fin de CM2 : comprendre et restituer les mots familiers
(jours, mois, nombres jusqu'à 20, couleurs, salutations), dans les deux sens.

Les paires viennent de `donnees/lexique.toml`, relu par un parent ; aucune n'est produite par
un LLM. Chaque exercice tire une paire et un sens :

- « mercredi en anglais ? » → `wednesday` ;
- « Wednesday en français ? » → `mercredi`.

**Écriture unique.** Le verrou compare lettre à lettre : la clé est toujours en minuscules,
avec l'orthographe du lexique (accents, tirets, espaces). Le mode d'emploi de la clé le dit.
Un sens n'est proposé que si sa réponse est unique dans tout le lexique (« bonjour » →
`hello` ou `good morning` : jamais demandé vers l'anglais), et jamais quand les deux mots
sont identiques (« six »). Pour les nombres, l'énoncé précise « en lettres ».

**Accents.** `ACCENTS_SUR_ENCLUME` dit si l'enfant sait taper un accent sur l'enclume. S'il
vaut `False`, une réponse française accentuée n'est jamais demandée ; le mot reste demandé
dans l'autre sens.
"""

from __future__ import annotations

import random
import tomllib
import unicodedata
from dataclasses import dataclass
from functools import cache
from importlib import resources

from mine_de_rien.exercises.base import Exercice, normaliser

competence = "anglais/vocabulaire"

VERS_ANGLAIS = "vers-anglais"
VERS_FRANCAIS = "vers-francais"

# Vérifié en jeu le 2026-11-08 : « août » se tape sur l'enclume, et l'accent est exigé.
ACCENTS_SUR_ENCLUME = True

_AVEC_MAJUSCULE = {"jours", "mois"}


@dataclass(frozen=True)
class Paire:
    anglais: str
    francais: str
    famille: str
    sens: tuple[str, ...]

    @property
    def accent(self) -> bool:
        return unicodedata.normalize("NFKD", self.francais) != self.francais


def _brut() -> list[tuple[str, str, str, bool]]:
    texte = resources.files(__package__).joinpath("donnees/lexique.toml").read_text("utf-8")
    entrees = []
    for famille, table in tomllib.loads(texte).items():
        for anglais, valeur in table.items():
            if isinstance(valeur, dict):
                entrees.append((famille, anglais, valeur["francais"], valeur["vers_anglais"]))
            else:
                entrees.append((famille, anglais, valeur, True))
    return entrees


def construire(entrees: list[tuple[str, str, str, bool]], accents: bool) -> tuple[Paire, ...]:
    """Calcule les sens autorisés de chaque paire. Une paire sans sens autorisé est gardée,
    pour être reconnue à la lecture d'une photo, mais n'est jamais demandée."""
    par_anglais: dict[str, int] = {}
    par_francais: dict[str, int] = {}
    for _, anglais, francais, _ in entrees:
        par_anglais[normaliser(anglais)] = par_anglais.get(normaliser(anglais), 0) + 1
        par_francais[normaliser(francais)] = par_francais.get(normaliser(francais), 0) + 1
    paires = []
    for famille, anglais, francais, vers_anglais in entrees:
        if anglais != anglais.lower() or francais != francais.lower():
            raise ValueError(f"lexique : majuscule dans {anglais!r} = {francais!r}")
        sens: list[str] = []
        if normaliser(anglais) != normaliser(francais):
            if vers_anglais and par_francais[normaliser(francais)] == 1:
                sens.append(VERS_ANGLAIS)
            accent = unicodedata.normalize("NFKD", francais) != francais
            if par_anglais[normaliser(anglais)] == 1 and (accents or not accent):
                sens.append(VERS_FRANCAIS)
        paires.append(Paire(anglais, francais, famille, tuple(sens)))
    return tuple(paires)


@cache
def lexique() -> tuple[Paire, ...]:
    return construire(_brut(), ACCENTS_SUR_ENCLUME)


def _affiche_anglais(p: Paire) -> str:
    return p.anglais.capitalize() if p.famille in _AVEC_MAJUSCULE else p.anglais


def exercice(p: Paire, sens: str) -> Exercice:
    if sens not in p.sens:
        raise ValueError(f"sens {sens} non autorisé pour {p.anglais!r}")
    precision = ", en lettres" if p.famille == "nombres" else ""
    if sens == VERS_ANGLAIS:
        enonce, reponse = f"{p.francais} en anglais{precision} ?", p.anglais
    else:
        enonce, reponse = f"{_affiche_anglais(p)} en français{precision} ?", p.francais
    return Exercice(
        competence=competence,
        enonce=enonce,
        reponse=reponse,
        donnees={"anglais": p.anglais, "francais": p.francais, "famille": p.famille,
                 "sens": sens},
    )


def generer(rng: random.Random, niveau: int = 1) -> Exercice:
    p = rng.choice([p for p in lexique() if p.sens])
    return exercice(p, rng.choice(p.sens))


def chercher(mot: str) -> Paire | None:
    """La paire d'un mot lu, en anglais d'abord, puis en français s'il n'est pas ambigu."""
    cle = normaliser(mot)
    for p in lexique():
        if normaliser(p.anglais) == cle:
            return p
    trouvees = [p for p in lexique() if normaliser(p.francais) == cle]
    return trouvees[0] if len(trouvees) == 1 else None


def exercice_pour(mot: str, rng: random.Random) -> Exercice | None:
    """L'exercice d'un mot lu sur une photo, dans un sens autorisé tiré au hasard ;
    None si le mot est hors lexique ou n'a aucun sens autorisé."""
    p = chercher(mot)
    if p is None or not p.sens:
        return None
    return exercice(p, rng.choice(p.sens))
