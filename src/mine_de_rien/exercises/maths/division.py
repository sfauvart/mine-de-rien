"""Division euclidienne, attendu de fin de CM2 : diviseur à un chiffre (niveau 1) puis à deux
chiffres (niveau 2). Le niveau 1 tombe juste pour que la réponse soit une clé simple."""

from __future__ import annotations

import random

from mine_de_rien.exercises.base import Exercice

competence = "maths/division-posee"


def generer(rng: random.Random, niveau: int = 1) -> Exercice:
    if niveau <= 1:
        diviseur = rng.randint(2, 9)
        quotient = rng.randint(12, 99)
        reste = 0
    else:
        diviseur = rng.randint(11, 25)
        quotient = rng.randint(12, 60)
        reste = rng.randint(0, diviseur - 1)
    dividende = diviseur * quotient + reste
    if reste == 0:
        enonce = f"{dividende} ÷ {diviseur} = ?"
        reponse = str(quotient)
    else:
        enonce = f"{dividende} ÷ {diviseur} : quotient et reste ?"
        reponse = f"{quotient} r {reste}"
    return Exercice(
        competence=competence,
        enonce=enonce,
        reponse=reponse,
        donnees={"dividende": dividende, "diviseur": diviseur, "quotient": quotient, "reste": reste},
    )
