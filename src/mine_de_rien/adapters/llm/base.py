"""Contrat d'un fournisseur de LLM.

Le LLM habille des énoncés et lit des photos de cahier. Il ne calcule jamais de réponse,
et ne reçoit jamais le prénom ni le chat brut de l'enfant.
"""

from __future__ import annotations

from typing import Protocol


class Llm(Protocol):
    async def completer(self, systeme: str, utilisateur: str, image: bytes | None = None) -> str:
        """Renvoie la réponse textuelle du modèle."""
