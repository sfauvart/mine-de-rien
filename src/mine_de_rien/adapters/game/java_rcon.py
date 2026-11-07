"""Minecraft Java (Paper) via RCON pour les commandes et le log serveur pour le chat.

Aucun plugin requis. Le chat est lu dans `logs/latest.log`, où chaque message apparaît
sous la forme `[HH:MM:SS INFO]: <Pseudo> texte`.
"""

from __future__ import annotations

import asyncio
from collections.abc import AsyncIterator

from mine_de_rien.adapters.game.base import MessageChat
from mine_de_rien.adapters.game.rcon import ClientRcon, ErreurRcon
from mine_de_rien.minecraft import texte


class JavaRcon:
    def __init__(
        self, hote: str, port: int, mot_de_passe: str, log: str, delai: float = 5.0
    ) -> None:
        self.hote = hote
        self.port = port
        self.mot_de_passe = mot_de_passe
        self.log = log
        self._client = ClientRcon(hote, port, mot_de_passe, delai)
        # Une seule commande à la fois sur la connexion : les réponses ne se mélangent pas.
        self._verrou = asyncio.Lock()

    async def commande(self, commande: str) -> str:
        async with self._verrou:
            return await asyncio.to_thread(self._executer, commande)

    def _executer(self, commande: str) -> str:
        # Reconnexion une fois, et seulement avant d'envoyer : une connexion qui tombe
        # pendant la réponse remonte en erreur, pour ne pas rejouer une commande déjà
        # exécutée (un `xp add` compté deux fois).
        if self._client.connexion_tombee():
            self._client.connecter()
        try:
            return self._client.commande(commande)
        except (BrokenPipeError, ConnectionResetError):
            self._client.connecter()
            return self._client.commande(commande)

    async def dire(self, joueur: str, message: str) -> None:
        reponse = await self.commande(texte.tellraw(joueur, message))
        # `tellraw` ne répond rien quand il réussit.
        if reponse.strip():
            raise ErreurRcon(f"le serveur a refusé le message : {reponse.strip()}")

    async def fermer(self) -> None:
        async with self._verrou:
            self._client.fermer()

    async def chat(self) -> AsyncIterator[MessageChat]:
        raise NotImplementedError("version 0.3 : suivi de latest.log")
        yield  # pragma: no cover
