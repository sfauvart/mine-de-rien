"""Fournisseur Anthropic."""

from __future__ import annotations


class Anthropic:
    def __init__(self, modele: str, cle: str) -> None:
        self.modele = modele
        self.cle = cle

    async def completer(self, systeme: str, utilisateur: str, image: bytes | None = None) -> str:
        raise NotImplementedError("version 0.1")
