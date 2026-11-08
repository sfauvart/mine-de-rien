"""Boucle de surveillance : pose les coffres de la file, voit quand l'enfant les ouvre.

Toutes les 3 secondes :
1. l'enfant est-il connecté (`list`) ? À sa connexion, un rappel s'il a un coffre actif ;
2. un coffre est actif : s'il est connecté, clé et niveaux s'il ne les a pas eus, puis
   recherche de la preuve dans son inventaire ; si le coffre a disparu sans être ouvert, il
   est reposé et l'événement noté ;
3. pas de coffre actif : le suivant de la file est posé, 10 secondes au moins après
   l'ouverture du précédent, et seulement quand le coffre est vide.

La boucle ne s'arrête pas sur une erreur du serveur (redémarrage, RCON coupé) : elle la
signale et réessaie au tour suivant.

Les commandes des parents (`/coffre`, `/file`, `/annuler`, `/rapport`) arrivent par un
canal ; `CommandesParents` les traite, indépendamment de Telegram.
"""

from __future__ import annotations

import asyncio
import logging
from collections.abc import Callable
from datetime import datetime, timedelta

from mine_de_rien.adapters.channel.base import Canal, MessageParent
from mine_de_rien.adapters.game.rcon import ErreurRcon
from mine_de_rien.config import Coffre as ConfigCoffre
from mine_de_rien.games.coffre_du_soir import (
    CoffreDuSoir,
    ErreurCoffre,
    Notificateur,
    formater_duree,
)
from mine_de_rien.rapport import rapport
from mine_de_rien.saisie import ErreurSaisie, interpreter
from mine_de_rien.storage.sqlite import Depot, ErreurStockage, maintenant

journal = logging.getLogger(__name__)

INTERVALLE = 3.0
DELAI_SUIVANT = timedelta(seconds=10)


class Orchestrateur:
    def __init__(
        self,
        depot: Depot,
        coffre: CoffreDuSoir,
        notificateur: Notificateur,
        horloge: Callable[[], datetime] = maintenant,
    ) -> None:
        self.depot = depot
        self.coffre = coffre
        self.notificateur = notificateur
        self.horloge = horloge
        self.en_ligne = False
        self._vider_demande = False

    async def tour(self) -> None:
        en_ligne = await self.coffre.joueur_en_ligne()
        vient_de_se_connecter = en_ligne and not self.en_ligne
        self.en_ligne = en_ligne

        actif = self.depot.actif()
        if actif is not None:
            await self._surveiller(actif, vient_de_se_connecter)
        else:
            await self._poser_suivant()

    async def _surveiller(self, actif, vient_de_se_connecter: bool) -> None:
        if self.en_ligne:
            if not actif.niveaux_donnes:
                await self.coffre.equiper()
                self.depot.marquer_niveaux_donnes(actif.id)
            if await self.coffre.preuve_trouvee(actif):
                quand = self.horloge()
                self.depot.marquer_ouvert(actif.id, quand)
                await self.coffre.ranger(actif)
                ouvert = self.depot.lire(actif.id)
                await self.notificateur.notifier(
                    f"Coffre n°{actif.id} ouvert en {formater_duree(ouvert.duree)} "
                    f"({actif.exercice.enonce} → {actif.exercice.reponse})."
                )
                return
        if not await self.coffre.coffre_present():
            await self.coffre.poser(actif)
            self.depot.noter("coffre_disparu", actif.id, self.horloge())
            await self.notificateur.notifier(
                f"Coffre n°{actif.id} disparu sans être ouvert : il a été reposé."
            )
            return
        if vient_de_se_connecter:
            await self.coffre.rappeler(actif)

    async def _poser_suivant(self) -> None:
        suivant = self.depot.suivant()
        if suivant is None:
            return
        dernier = self.depot.dernier_ouvert()
        if dernier is not None and self.horloge() - dernier.ouvert_le < DELAI_SUIVANT:
            return
        if not await self.coffre.coffre_vide():
            if self.en_ligne and not self._vider_demande:
                await self.coffre.demander_de_vider()
                self._vider_demande = True
            return
        self._vider_demande = False
        await self.coffre.poser(suivant)
        self.depot.marquer_pose(suivant.id, self.horloge())
        if self.en_ligne:
            # La clé d'abord : le mode d'emploi de l'annonce en parle.
            await self.coffre.equiper()
            self.depot.marquer_niveaux_donnes(suivant.id)
            await self.coffre.annoncer(suivant)
        await self.notificateur.notifier(
            f"Coffre n°{suivant.id} posé : {suivant.exercice.enonce} "
            f"(réponse {suivant.exercice.reponse})."
        )

    async def tourner(self, intervalle: float = INTERVALLE) -> None:
        while True:
            try:
                await self.tour()
            except (ErreurRcon, ErreurCoffre, ErreurStockage) as e:
                journal.warning("tour de surveillance interrompu : %s", e)
            await asyncio.sleep(intervalle)


def mettre_en_file(
    depot: Depot, coffre: ConfigCoffre, texte: str, demande_par: str
) -> list[str]:
    """Ajoute à la file les exercices saisis ; une ligne par coffre. Lève `ErreurSaisie`."""
    saisie = interpreter(texte)
    lignes = []
    for ex in saisie.exercices:
        numero = depot.ajouter(
            ex,
            source=saisie.source,
            demande_par=demande_par,
            recompense=coffre.recompense,
            recompense_nombre=coffre.recompense_nombre,
        )
        lignes.append(f"Coffre n°{numero} en file : {ex.enonce} (réponse {ex.reponse})")
    return lignes


AIDE = (
    "Commandes :\n"
    "/coffre 348 ÷ 12 : un calcul recopié du cahier\n"
    "/coffre division 3 : trois exercices d'une compétence\n"
    "/file : le coffre actif et ceux qui attendent\n"
    "/annuler 12 : retire le coffre n°12\n"
    "/rapport : les 7 derniers jours"
)


class CommandesParents:
    """Traite les messages des parents. Ne répond rien à un expéditeur non autorisé."""

    def __init__(
        self,
        depot: Depot,
        config_coffre: ConfigCoffre,
        coffre: CoffreDuSoir | None,
        parents: list[int],
        horloge: Callable[[], datetime] = maintenant,
    ) -> None:
        self.depot = depot
        self.config_coffre = config_coffre
        self.coffre = coffre
        self.parents = set(parents)
        self.horloge = horloge

    async def traiter(self, message: MessageParent) -> str | None:
        if message.expediteur not in self.parents:
            return None
        commande, _, argument = message.texte.strip().partition(" ")
        # « /coffre@MonBot 12 x 7 » dans certains clients Telegram.
        commande = commande.split("@", 1)[0].lower()
        argument = argument.strip()
        if commande == "/coffre":
            try:
                lignes = mettre_en_file(
                    self.depot, self.config_coffre, argument, str(message.expediteur)
                )
            except ErreurSaisie as e:
                return f"Refusé : {e}"
            return "\n".join(lignes)
        if commande == "/file":
            return self._file()
        if commande == "/annuler":
            return await self._annuler(argument)
        if commande == "/rapport":
            return rapport(self.depot, self.horloge())
        return AIDE

    def _file(self) -> str:
        actif = self.depot.actif()
        lignes = []
        if actif is not None:
            depuis = formater_duree(self.horloge() - actif.pose_le) if actif.pose_le else "?"
            lignes.append(
                f"Coffre actif : n°{actif.id} {actif.exercice.enonce} "
                f"(réponse {actif.exercice.reponse}), posé depuis {depuis}"
            )
        else:
            lignes.append("Aucun coffre actif.")
        attente = self.depot.en_file()
        if attente:
            lignes.append("En attente :")
            lignes += [f"• n°{s.id} {s.exercice.enonce}" for s in attente]
        return "\n".join(lignes)

    async def _annuler(self, argument: str) -> str:
        if not argument.lstrip("n°").isdigit():
            return "Préciser le numéro du coffre : /annuler 12"
        numero = int(argument.lstrip("n°"))
        s = self.depot.lire(numero)
        if s is None or s.statut not in ("en_file", "pose"):
            return f"Le coffre n°{numero} n'est ni actif ni en attente."
        if s.statut == "pose" and self.coffre is not None:
            try:
                await self.coffre.retirer()
            except (ErreurRcon, ErreurCoffre) as e:
                return f"Le coffre n°{numero} n'a pas pu être retiré du jeu, réessayer : {e}"
        self.depot.annuler(numero)
        return f"Coffre n°{numero} annulé."


class NotificateurCanal:
    """Les notifications de l'orchestrateur partent vers tous les parents du canal."""

    def __init__(self, canal: Canal) -> None:
        self.canal = canal

    async def notifier(self, message: str) -> None:
        await self.canal.envoyer(message)


async def servir(canal: Canal, commandes: CommandesParents) -> None:
    """Répond aux messages des parents, un par un, aussi longtemps que le canal est ouvert."""
    async for message in canal.recevoir():
        try:
            reponse = await commandes.traiter(message)
        except (ErreurRcon, ErreurCoffre, ErreurStockage) as e:
            reponse = f"Erreur : {e}"
        if reponse:
            await canal.repondre(message.expediteur, reponse)
