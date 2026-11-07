"""Minecraft Java (Paper) via RCON pour les commandes et le log serveur pour le chat.

Aucun plugin requis. Le chat est lu dans `logs/latest.log`, où chaque message apparaît
sous la forme `[HH:MM:SS INFO]: <Pseudo> texte`.
"""

from __future__ import annotations

from collections.abc import AsyncIterator

from mine_de_rien.adapters.game.base import MessageChat


class JavaRcon:
    def __init__(self, hote: str, port: int, mot_de_passe: str, log: str) -> None:
        self.hote = hote
        self.port = port
        self.mot_de_passe = mot_de_passe
        self.log = log

    async def commande(self, commande: str) -> str:
        raise NotImplementedError("version 0.1 : mcrcon dans un thread")

    async def dire(self, joueur: str, texte: str) -> None:
        raise NotImplementedError("version 0.1 : tellraw avec le texte échappé en JSON")

    async def chat(self) -> AsyncIterator[MessageChat]:
        raise NotImplementedError("version 0.3 : suivi de latest.log")
        yield  # pragma: no cover
