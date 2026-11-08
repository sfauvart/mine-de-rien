"""Canal Telegram, premier canal implémenté (python-telegram-bot, interrogation longue).

Seuls les identifiants listés dans `canal.parents` sont écoutés, et seulement en
conversation privée avec le bot : un message de groupe ou d'un inconnu est ignoré sans
réponse. Seule exception, `/start` répond à n'importe qui son propre identifiant Telegram,
pour que les parents puissent le recopier dans `config.toml`, et rien d'autre.

Ce qui part vers Telegram est décidé ailleurs (orchestrateur, rapport) : énoncés, réponses,
numéros de coffre et durées, jamais le pseudo de l'enfant. Alternatives prévues sans compte
externe : Matrix auto-hébergé, page web sur le réseau local.
"""

from __future__ import annotations

import asyncio
import logging
from collections.abc import AsyncIterator

from telegram import Update
from telegram.error import TelegramError
from telegram.ext import Application, CommandHandler, ContextTypes, MessageHandler, filters

from mine_de_rien.adapters.channel.base import MessageParent

journal = logging.getLogger(__name__)


def message_autorise(update: Update, parents: set[int]) -> MessageParent | None:
    """Le message, s'il vient d'un parent autorisé en conversation privée ; sinon None."""
    message, utilisateur, chat = update.effective_message, update.effective_user, update.effective_chat
    if message is None or utilisateur is None or chat is None or not message.text:
        return None
    if chat.type != "private" or utilisateur.id not in parents:
        return None
    return MessageParent(expediteur=utilisateur.id, texte=message.text)


class Telegram:
    def __init__(self, jeton: str, parents: list[int]) -> None:
        self.jeton = jeton
        self.parents = set(parents)
        self._file: asyncio.Queue[MessageParent] = asyncio.Queue()
        self._app: Application | None = None

    async def _start(self, update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
        utilisateur, message, chat = (
            update.effective_user, update.effective_message, update.effective_chat
        )
        if utilisateur and message and chat and chat.type == "private":
            await message.reply_text(f"Ton identifiant Telegram : {utilisateur.id}")

    async def _message(self, update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
        recu = message_autorise(update, self.parents)
        if recu is not None:
            await self._file.put(recu)

    async def demarrer(self) -> None:
        self._app = Application.builder().token(self.jeton).build()
        self._app.add_handler(CommandHandler("start", self._start))
        self._app.add_handler(MessageHandler(filters.TEXT, self._message))
        await self._app.initialize()
        await self._app.start()
        await self._app.updater.start_polling(drop_pending_updates=False)

    async def arreter(self) -> None:
        if self._app is not None:
            await self._app.updater.stop()
            await self._app.stop()
            await self._app.shutdown()
            self._app = None

    async def repondre(self, destinataire: int, texte: str) -> None:
        if destinataire not in self.parents or self._app is None:
            return
        try:
            await self._app.bot.send_message(chat_id=destinataire, text=texte)
        except TelegramError as e:  # réseau coupé, bot bloqué : on le signale, on continue
            journal.warning("message Telegram non remis : %s", e)

    async def envoyer(self, texte: str) -> None:
        for parent in sorted(self.parents):
            await self.repondre(parent, texte)

    async def recevoir(self) -> AsyncIterator[MessageParent]:
        while True:
            yield await self._file.get()
