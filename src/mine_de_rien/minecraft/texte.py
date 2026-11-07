"""Composants texte Minecraft construits à partir de texte venu de l'extérieur.

Les énoncés sont écrits par les parents : ils peuvent contenir des guillemets, des
antislashs, des accolades ou des crochets. Le texte n'est donc jamais concaténé dans une
commande. On construit un objet Python que l'on sérialise.

Depuis la 1.21.5, les composants texte des commandes s'écrivent en SNBT. Le JSON produit par
`json.dumps` en est un sous-ensemble valide : clés entre guillemets doubles, chaînes entre
guillemets doubles où seuls `"` et `\\` sont échappés (les caractères de contrôle, que JSON
échapperait en `\\uXXXX`, sont retirés avant). Les accents restent tels quels, RCON
transporte de l'UTF-8.
"""

from __future__ import annotations

import json
import re
import unicodedata

LONGUEUR_MAX = 256

# Pseudo Minecraft : ce qui est accepté par la liste blanche. Interdit `@a`, les espaces, etc.
_PSEUDO = re.compile(r"[A-Za-z0-9_]{1,16}")


def nettoyer(texte: str, longueur_max: int = LONGUEUR_MAX) -> str:
    """Retire les caractères de contrôle et le `§` (codes de mise en forme hérités), puis borne.

    Un saut de ligne devient une espace : le chat et les panneaux ne s'en servent pas.
    """
    propre = "".join(
        " " if c in "\n\r\t" else c
        for c in texte
        if c != "§" and (c in "\n\r\t" or unicodedata.category(c) != "Cc")
    )
    return propre[:longueur_max]


def composant(texte: str, longueur_max: int = LONGUEUR_MAX, **style: str | bool) -> str:
    """Composant texte sérialisé, prêt à être placé dans une commande.

    `style` accepte les clés de style des composants (`color="gold"`, `bold=True`, ...).
    """
    objet: dict[str, str | bool] = {"text": nettoyer(texte, longueur_max)}
    objet.update(style)
    return json.dumps(objet, ensure_ascii=False)


def verifier_pseudo(joueur: str) -> str:
    """Renvoie le pseudo s'il est sûr à placer dans une commande, sinon lève `ValueError`."""
    if not _PSEUDO.fullmatch(joueur):
        raise ValueError(f"pseudo Minecraft invalide : {joueur!r}")
    return joueur


def tellraw(joueur: str, texte: str, **style: str | bool) -> str:
    """Commande `tellraw` (sans `/`) qui affiche `texte` à `joueur`."""
    return f"tellraw {verifier_pseudo(joueur)} {composant(texte, **style)}"
