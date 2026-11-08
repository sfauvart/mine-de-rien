"""Calcul d'une expression recopiée du cahier par un parent : `348 ÷ 12`, `3,5 + 1,25`.

Attendu de fin de CM2 : les quatre opérations sur les entiers et les décimaux, avec
parenthèses. Mine de rien recalcule toujours la réponse : on ne fait confiance ni au parent
ni à un LLM pour le résultat.

Analyseur par descente récursive sur une liste de jetons. Jamais `eval` : tout caractère
hors des chiffres, de la virgule, du point, des quatre opérations et des parenthèses est
refusé. Le calcul est exact, en `Fraction`.

Écriture unique de la réponse (le coffre compare caractère pour caractère) : entier, ou
décimal à deux chiffres au plus après la virgule, écrit avec une virgule et sans zéro
inutile (`3,5`, jamais `3,50` ni `3.5`). Un résultat qui ne s'écrit pas ainsi (`10 ÷ 3`), ou
négatif, est refusé : ce n'est pas un exercice de coffre.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from fractions import Fraction

from mine_de_rien.exercises.base import Exercice
from mine_de_rien.minecraft.texte import lignes_panneau

# Garde-fou avant analyse ; l'énoncé normalisé doit en plus tenir sur le panneau.
LONGUEUR_MAX = 60
CHIFFRES_MAX = 9

_JETON = re.compile(r"\s*(?:(\d+(?:[.,]\d+)?)|([-+×xX*÷:/()]))")
_OPERATEURS = {
    "+": "+",
    "-": "-",
    "×": "×",
    "x": "×",
    "X": "×",
    "*": "×",
    "÷": "÷",
    ":": "÷",
    "/": "÷",
}
_COMPETENCES = {
    "+": "maths/addition-posee",
    "-": "maths/soustraction-posee",
    "×": "maths/multiplication-posee",
    "÷": "maths/division-posee",
}
COMPETENCE_MELANGEE = "maths/calcul"


class ErreurExpression(ValueError):
    """Expression refusée ; le message s'adresse au parent."""


@dataclass(frozen=True)
class _Jeton:
    genre: str  # "nombre", "op", "(", ")"
    texte: str
    valeur: Fraction | None = None


def _decouper(texte: str) -> list[_Jeton]:
    # Tolère « 348 ÷ 12 = » et « 348 ÷ 12 = ? », tels qu'on les recopie du cahier.
    texte = re.sub(r"\s*=\s*\??\s*$", "", texte.strip())
    if len(texte) > LONGUEUR_MAX:
        raise ErreurExpression(
            f"Expression trop longue ({LONGUEUR_MAX} caractères au plus)."
        )
    jetons: list[_Jeton] = []
    position = 0
    while position < len(texte):
        m = _JETON.match(texte, position)
        if not m:
            reste = texte[position:].strip()
            raise ErreurExpression(f"Caractère non reconnu : « {reste[:1]} ».")
        position = m.end()
        nombre, symbole = m.groups()
        if nombre is not None:
            entier = nombre.replace(",", ".").split(".")[0]
            if len(entier) > CHIFFRES_MAX:
                raise ErreurExpression("Nombre trop grand pour un exercice de CM2.")
            jetons.append(_Jeton("nombre", nombre.replace(".", ","), Fraction(nombre.replace(",", "."))))
        elif symbole in "()":
            jetons.append(_Jeton(symbole, symbole))
        else:
            jetons.append(_Jeton("op", _OPERATEURS[symbole]))
        if position < len(texte) and texte[position:].strip() == "":
            break
    if not jetons:
        raise ErreurExpression("Expression vide.")
    return jetons


class _Analyseur:
    """expression := terme (('+' | '-') terme)*
    terme      := facteur (('×' | '÷') facteur)*
    facteur    := nombre | '(' expression ')'
    """

    def __init__(self, jetons: list[_Jeton]) -> None:
        self.jetons = jetons
        self.i = 0

    def _courant(self) -> _Jeton | None:
        return self.jetons[self.i] if self.i < len(self.jetons) else None

    def analyser(self) -> Fraction:
        valeur = self._expression()
        if self._courant() is not None:
            raise ErreurExpression(f"Expression mal formée près de « {self._courant().texte} ».")
        return valeur

    def _expression(self) -> Fraction:
        valeur = self._terme()
        while (j := self._courant()) is not None and j.genre == "op" and j.texte in "+-":
            self.i += 1
            droite = self._terme()
            valeur = valeur + droite if j.texte == "+" else valeur - droite
        return valeur

    def _terme(self) -> Fraction:
        valeur = self._facteur()
        while (j := self._courant()) is not None and j.genre == "op" and j.texte in "×÷":
            self.i += 1
            droite = self._facteur()
            if j.texte == "×":
                valeur *= droite
            elif droite == 0:
                raise ErreurExpression("Division par zéro.")
            else:
                valeur /= droite
        return valeur

    def _facteur(self) -> Fraction:
        j = self._courant()
        if j is None:
            raise ErreurExpression("Expression incomplète.")
        if j.genre == "nombre":
            self.i += 1
            assert j.valeur is not None
            return j.valeur
        if j.genre == "(":
            self.i += 1
            valeur = self._expression()
            fermante = self._courant()
            if fermante is None or fermante.genre != ")":
                raise ErreurExpression("Parenthèse non fermée.")
            self.i += 1
            return valeur
        raise ErreurExpression(f"Expression mal formée près de « {j.texte} ».")


def ecrire_reponse(valeur: Fraction) -> str:
    """Écriture unique : entier, ou décimal à deux chiffres au plus, avec une virgule.

    Lève `ErreurExpression` si `valeur` ne s'écrit pas ainsi, ou si elle est négative.
    """
    if valeur < 0:
        raise ErreurExpression("Le résultat est négatif : ce n'est pas un exercice de CM2.")
    centiemes = valeur * 100
    if centiemes.denominator != 1:
        raise ErreurExpression(
            "Le résultat ne tombe pas juste (plus de deux chiffres après la virgule) : "
            "le coffre ne saurait pas quelle réponse attendre."
        )
    entier, decimales = divmod(centiemes.numerator, 100)
    if decimales == 0:
        return str(entier)
    return f"{entier},{decimales:02d}".rstrip("0")


def _enonce(jetons: list[_Jeton]) -> str:
    morceaux: list[str] = []
    for j in jetons:
        if j.genre == ")" or (morceaux and morceaux[-1] == "("):
            morceaux.append(j.texte)
        elif morceaux:
            morceaux.append(" " + j.texte)
        else:
            morceaux.append(j.texte)
    return "".join(morceaux) + " = ?"


def calculer(texte: str) -> Exercice:
    """Transforme une expression en `Exercice`, réponse recalculée, ou lève `ErreurExpression`."""
    jetons = _decouper(texte)
    operateurs = {j.texte for j in jetons if j.genre == "op"}
    if not operateurs:
        raise ErreurExpression("Il manque une opération (+, -, ×, ÷).")
    valeur = _Analyseur(jetons).analyser()
    reponse = ecrire_reponse(valeur)
    enonce = _enonce(jetons)
    if lignes_panneau(enonce) is None:
        raise ErreurExpression("Expression trop longue pour tenir sur le panneau (4 lignes de 15).")
    competence = _COMPETENCES[operateurs.pop()] if len(operateurs) == 1 else COMPETENCE_MELANGEE
    return Exercice(
        competence=competence,
        enonce=enonce,
        reponse=reponse,
        donnees={"expression": enonce.removesuffix(" = ?")},
    )
