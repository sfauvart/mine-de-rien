"""Fournisseur Anthropic, distant.

Il ne reçoit jamais de photo : une photo de fiche montre souvent le prénom de l'enfant et ne
se pseudonymise pas. Le reste n'est pas encore écrit.
"""

from __future__ import annotations

from mine_de_rien.adapters.llm.base import ErreurLlm


class Anthropic:
    distant = True

    def __init__(self, modele: str, cle: str) -> None:
        self.modele = modele
        self.cle = cle

    async def completer(
        self,
        systeme: str,
        utilisateur: str,
        image: bytes | None = None,
        en_json: bool = False,
    ) -> str:
        if image is not None:
            raise ErreurLlm(
                "La lecture de photo demande un LLM local ([llm] adaptateur = "
                "\"openai-compatible\") : une photo ne part jamais vers un LLM distant."
            )
        raise NotImplementedError("aucun usage du LLM distant pour l'instant")
