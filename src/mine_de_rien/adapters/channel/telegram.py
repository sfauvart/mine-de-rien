"""Canal Telegram, premier canal implémenté.

Seuls les identifiants listés dans `canal.parents` sont écoutés. Alternatives prévues sans
compte externe : Matrix auto-hébergé, page web sur le réseau local.
"""

from __future__ import annotations

from collections.abc import AsyncIterator

from mine_de_rien.adapters.channel.base import MessageParent


class Telegram:
    def __init__(self, jeton: str, parents: list[int]) -> None:
        self.jeton = jeton
        self.parents = parents

    async def envoyer(self, texte: str) -> None:
        raise NotImplementedError("version 0.1")

    async def recevoir(self) -> AsyncIterator[MessageParent]:
        raise NotImplementedError("version 0.1")
        yield  # pragma: no cover
