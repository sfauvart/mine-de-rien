"""Contrat d'un adaptateur de jeu."""

from __future__ import annotations

from collections.abc import AsyncIterator
from dataclasses import dataclass
from datetime import datetime
from typing import Protocol


@dataclass(frozen=True)
class MessageChat:
    horodatage: datetime
    joueur: str
    texte: str


class AdaptateurJeu(Protocol):
    async def commande(self, commande: str) -> str:
        """Exécute une commande serveur (sans le / initial) et renvoie sa sortie brute."""

    async def dire(self, joueur: str, texte: str) -> None:
        """Affiche un message à un joueur."""

    def chat(self) -> AsyncIterator[MessageChat]:
        """Flux des messages de chat des joueurs."""
