"""Contrat d'un fournisseur de LLM.

Le LLM recopie ce qu'il lit sur une photo de fiche, classe et reformule ce que dit Pixel. Il
ne calcule jamais de réponse et ne reçoit jamais le prénom de l'enfant. Une photo ne se
pseudonymise pas, un message de chat non plus : l'une comme l'autre ne partent que vers un
fournisseur local (`distant = False`). Seul le message adressé à Pixel part, jamais le
reste du chat.
"""

from __future__ import annotations

import json
import re
from typing import Protocol


class ErreurLlm(RuntimeError):
    """Le LLM n'a pas répondu, ou sa réponse est inutilisable ; le message s'adresse au parent."""


class Llm(Protocol):
    distant: bool

    async def completer(
        self,
        systeme: str,
        utilisateur: str,
        image: bytes | None = None,
        en_json: bool = False,
    ) -> str:
        """Renvoie la réponse textuelle du modèle. `image` est un JPEG."""


_BALISES = re.compile(r"^\s*```(?:json)?\s*(.*?)\s*```\s*$", re.DOTALL)


def lire_json(texte: str) -> dict:
    """Lit la réponse JSON d'un modèle, entourée ou non de balises de code."""
    m = _BALISES.match(texte)
    if m:
        texte = m.group(1)
    try:
        valeur = json.loads(texte)
    except json.JSONDecodeError as e:
        raise ErreurLlm("Le LLM a répondu autre chose que du JSON.") from e
    if not isinstance(valeur, dict):
        raise ErreurLlm("Le LLM a répondu un JSON qui n'est pas un objet.")
    return valeur
