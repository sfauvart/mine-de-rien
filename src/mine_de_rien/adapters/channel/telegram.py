"""Canal Telegram, premier canal implémenté (python-telegram-bot, interrogation longue).

Seuls les identifiants listés dans `canal.parents` sont écoutés, et seulement en
conversation privée avec le bot : un message de groupe ou d'un inconnu est ignoré sans
réponse. Seule exception, `/start` répond à n'importe qui son propre identifiant Telegram,
pour que les parents puissent le recopier dans `config.toml`, et rien d'autre.

Ce qui part vers Telegram est décidé ailleurs (orchestrateur, rapport) : énoncés, réponses,
numéros de coffre et durées, jamais le pseudo de l'enfant.

Une photo de fiche envoyée par un parent est téléchargée en mémoire (la plus grande taille
proposée), jamais écrite sur le disque, et transmise avec sa légende. Elle a déjà traversé
les serveurs de Telegram : c'est le parent qui choisit ce qu'il photographie. Alternatives prévues sans compte
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


TAILLE_MESSAGE = 4000  # Telegram refuse au-delà de 4096 caractères


def expediteur_autorise(update: Update, parents: set[int]) -> int | None:
    """L'identifiant de l'expéditeur, s'il est parent et en conversation privée ; sinon None."""
    message, utilisateur, chat = update.effective_message, update.effective_user, update.effective_chat
    if message is None or utilisateur is None or chat is None:
        return None
    if chat.type != "private" or utilisateur.id not in parents:
        return None
    return utilisateur.id


def message_autorise(update: Update, parents: set[int]) -> MessageParent | None:
    """Le message texte, s'il vient d'un parent autorisé en conversation privée ; sinon None."""
    expediteur = expediteur_autorise(update, parents)
    if expediteur is None or not update.effective_message.text:
        return None
    return MessageParent(expediteur=expediteur, texte=update.effective_message.text)


def decouper(texte: str, taille: int = TAILLE_MESSAGE) -> list[str]:
    """Coupe un long message sur les fins de ligne, en morceaux acceptés par Telegram."""
    morceaux: list[str] = []
    courant = ""
    for ligne in texte.split("\n"):
        while len(ligne) > taille:
            if courant:
                morceaux.append(courant)
                courant = ""
            morceaux.append(ligne[:taille])
            ligne = ligne[taille:]
        candidat = f"{courant}\n{ligne}" if courant else ligne
        if len(candidat) > taille:
            morceaux.append(courant)
            courant = ligne
        else:
            courant = candidat
    if courant:
        morceaux.append(courant)
    return morceaux


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

    async def _photo(self, update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
        expediteur = expediteur_autorise(update, self.parents)
        message = update.effective_message
        if expediteur is None or not message.photo:
            return
        try:
            fichier = await message.photo[-1].get_file()
            octets = bytes(await fichier.download_as_bytearray())
        except TelegramError as e:
            journal.warning("photo Telegram non téléchargée : %s", e)
            await self.repondre(expediteur, "Je n'ai pas pu récupérer la photo, réessayer.")
            return
        await self._file.put(
            MessageParent(expediteur=expediteur, texte=message.caption or "", image=octets)
        )

    async def demarrer(self) -> None:
        self._app = Application.builder().token(self.jeton).build()
        self._app.add_handler(CommandHandler("start", self._start))
        self._app.add_handler(MessageHandler(filters.TEXT, self._message))
        self._app.add_handler(MessageHandler(filters.PHOTO, self._photo))
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
            for morceau in decouper(texte):
                await self._app.bot.send_message(chat_id=destinataire, text=morceau)
        except TelegramError as e:  # réseau coupé, bot bloqué : on le signale, on continue
            journal.warning("message Telegram non remis : %s", e)

    async def envoyer(self, texte: str) -> None:
        for parent in sorted(self.parents):
            await self.repondre(parent, texte)

    async def recevoir(self) -> AsyncIterator[MessageParent]:
        while True:
            yield await self._file.get()
