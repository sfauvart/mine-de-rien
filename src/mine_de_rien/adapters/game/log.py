"""Chat des joueurs lu dans `logs/latest.log`, sans plugin.

Format observé (Paper, Minecraft 26.3) :

    [18:45:26] [Async Chat Thread - #0/INFO]: [Not Secure] <seb> salut

`[Not Secure]` apparaît quand le chat n'est pas signé (comptes hors ligne) ; le nom du fil
varie. Les autres lignes sont ignorées : `/say` s'écrit `[Pseudo] texte`, et un joueur qui
tape « <autre> bonjour » produit `<lui> <autre> bonjour`, son propre pseudo en premier.

Le fichier est suivi comme `tail -F` : à l'ouverture, on part de la fin (les messages d'avant
le lancement ne sont jamais rejoués). Le serveur renomme `latest.log` à chaque démarrage et au
changement de jour : si le fichier change (inode) ou rétrécit, il est relu depuis le début.

Un message est daté de sa **réception** : l'heure du log est celle du conteneur, en UTC dans
l'image Docker (une heure d'écart observée le 2026-11-08), et elle ne porte pas la date.

Le texte du chat n'est ni journalisé ni enregistré ici.
"""

from __future__ import annotations

import asyncio
import os
import re
from collections.abc import AsyncIterator, Callable
from datetime import datetime
from pathlib import Path
from typing import BinaryIO

from mine_de_rien.adapters.game.base import MessageChat

LIGNE_CHAT = re.compile(
    r"^\[\d\d:\d\d:\d\d\] \[[^\]]+/INFO\]: (?:\[Not Secure\] )?"
    r"<([A-Za-z0-9_]{3,16})> (.*)$"
)
INTERVALLE = 0.5


def analyser(ligne: str, quand: datetime) -> MessageChat | None:
    """Une ligne du log, lue à `quand` → message de chat, ou None si ce n'en est pas un."""
    m = LIGNE_CHAT.match(ligne.rstrip("\r\n"))
    if not m:
        return None
    return MessageChat(quand, m.group(1), m.group(2))


class SuiviLog:
    """Lit les lignes ajoutées à un fichier, rotation comprise. Une lecture par appel."""

    def __init__(self, chemin: Path | str) -> None:
        self.chemin = Path(chemin)
        self._fichier: BinaryIO | None = None
        self._inode: int | None = None
        self._position = 0
        self._reste = b""
        # Au premier passage seulement, on saute ce qui est déjà écrit. Un fichier qui
        # apparaît ou qui remplace l'ancien est lu en entier : il est neuf.
        self._premier = True

    def _fermer(self) -> None:
        if self._fichier is not None:
            self._fichier.close()
        self._fichier = None
        self._reste = b""

    def lire(self) -> list[str]:
        try:
            etat = os.stat(self.chemin)
        except FileNotFoundError:
            self._fermer()
            self._premier = False
            return []
        if self._fichier is None or etat.st_ino != self._inode or etat.st_size < self._position:
            self._fermer()
            self._fichier = open(self.chemin, "rb")  # noqa: SIM115 (gardé ouvert entre deux appels)
            self._inode = etat.st_ino
            self._position = etat.st_size if self._premier else 0
        self._premier = False
        self._fichier.seek(self._position)
        donnees = self._fichier.read()
        self._position += len(donnees)
        *lignes, self._reste = (self._reste + donnees).split(b"\n")
        return [ligne.decode("utf-8", errors="replace") for ligne in lignes]

    def fermer(self) -> None:
        self._fermer()


async def suivre_chat(
    chemin: Path | str,
    intervalle: float = INTERVALLE,
    horloge: Callable[[], datetime] = datetime.now,
) -> AsyncIterator[MessageChat]:
    """Flux des messages de chat écrits dans le log à partir de maintenant."""
    suivi = SuiviLog(chemin)
    try:
        while True:
            for ligne in await asyncio.to_thread(suivi.lire):
                message = analyser(ligne, horloge())
                if message is not None:
                    yield message
            await asyncio.sleep(intervalle)
    finally:
        suivi.fermer()
