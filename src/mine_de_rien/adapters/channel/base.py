"""Contrat d'un canal de communication avec les parents."""

from __future__ import annotations

from collections.abc import AsyncIterator
from dataclasses import dataclass
from typing import Protocol


@dataclass(frozen=True)
class MessageParent:
    expediteur: int
    texte: str
    image: bytes | None = None


class Canal(Protocol):
    async def envoyer(self, texte: str) -> None:
        """Envoie un message à tous les parents autorisés."""

    def recevoir(self) -> AsyncIterator[MessageParent]:
        """Flux des messages reçus des parents autorisés."""
