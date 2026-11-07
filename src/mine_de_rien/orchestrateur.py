"""Boucle de surveillance : pose les coffres de la file, voit quand l'enfant les ouvre.

Toutes les 3 secondes :
1. l'enfant est-il connecté (`list`) ? À sa connexion, un rappel s'il a un coffre actif ;
2. un coffre est actif : s'il est connecté, niveaux offerts s'il ne les a pas eus, puis
   recherche de la preuve dans son inventaire ; si le coffre a disparu sans être ouvert, il
   est reposé et l'événement noté ;
3. pas de coffre actif : le suivant de la file est posé, 10 secondes au moins après
   l'ouverture du précédent, et seulement quand le coffre est vide.

La boucle ne s'arrête pas sur une erreur du serveur (redémarrage, RCON coupé) : elle la
signale et réessaie au tour suivant.
"""

from __future__ import annotations

import asyncio
import logging
from collections.abc import Callable
from datetime import datetime, timedelta

from mine_de_rien.adapters.game.rcon import ErreurRcon
from mine_de_rien.games.coffre_du_soir import (
    CoffreDuSoir,
    ErreurCoffre,
    Notificateur,
    formater_duree,
)
from mine_de_rien.storage.sqlite import Depot, maintenant

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
                await self.coffre.donner_niveaux()
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
            await self.coffre.annoncer(suivant)
            await self.coffre.donner_niveaux()
            self.depot.marquer_niveaux_donnes(suivant.id)
        await self.notificateur.notifier(
            f"Coffre n°{suivant.id} posé : {suivant.exercice.enonce} "
            f"(réponse {suivant.exercice.reponse})."
        )

    async def tourner(self, intervalle: float = INTERVALLE) -> None:
        while True:
            try:
                await self.tour()
            except (ErreurRcon, ErreurCoffre) as e:
                journal.warning("tour de surveillance interrompu : %s", e)
            await asyncio.sleep(intervalle)
