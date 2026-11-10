"""Mode paisible : tant qu'il reste un coffre à faire, les monstres laissent l'enfant tranquille.

**Le désir.** Aller à l'atelier et chercher son calcul sans se faire interrompre par un zombie.

**Commandes**, vérifiées sur Minecraft Java 26.3 (Paper) :

    difficulty            → « The difficulty is Peaceful »
    difficulty peaceful   → « The difficulty has been set to Peaceful »
                            « The difficulty did not change; it is already set to Peaceful »

La difficulté est relue dans le monde à chaque tour de surveillance et changée seulement si
elle diffère de l'attendue. Rien n'est gardé en base : `/annuler`, un redémarrage du serveur ou
de l'agent et un changement de monde sont couverts sans code de plus. Le serveur conserve la
difficulté d'un démarrage à l'autre.

**Ce que voit l'enfant** : une ligne dans le chat quand la difficulté vient de changer, s'il est
connecté. Une bascule survenue pendant son absence n'est pas annoncée plus tard. Rien ne part
sur Telegram.

**Contournements.**
- *Laisser le coffre non résolu pour rester tranquille* : accepté, les parents voient les
  coffres en attente avec `/file`.
- *Changer la difficulté soi-même* : réservé aux opérateurs, et l'enfant ne doit pas l'être.
  Un `difficulty` tapé à la main par un parent est corrigé au tour suivant ; pour garder la main,
  `[paisible] actif = false`.

**Effets de bord.** La difficulté vaut pour tout le serveur. En paisible, les monstres présents
disparaissent et la faim ne descend plus.
"""

from __future__ import annotations

import re

from mine_de_rien.adapters.game.base import AdaptateurJeu
from mine_de_rien.games.coffre_du_soir import ErreurCoffre
from mine_de_rien.minecraft import texte

PAISIBLE = "peaceful"
MESSAGE_PAISIBLE = "Les monstres s'endorment tant qu'un coffre t'attend."
MESSAGE_RETOUR = "Plus de coffre : les monstres sont de retour !"
_LECTURE = re.compile(r"The difficulty is (\w+)")
# « already set » : quelqu'un d'autre l'a réglée entre la lecture et l'écriture.
_ECRITURE = re.compile(r"has been set to|already set to")


class ModePaisible:
    def __init__(self, jeu: AdaptateurJeu, pseudo: str, difficulte_normale: str = "easy") -> None:
        self.jeu = jeu
        self.pseudo = texte.verifier_pseudo(pseudo)
        self.difficulte_normale = difficulte_normale

    async def _difficulte(self) -> str:
        reponse = await self.jeu.commande("difficulty")
        lecture = _LECTURE.search(reponse)
        if lecture is None:
            raise ErreurCoffre(f"difficulté illisible : {reponse.strip()}")
        return lecture.group(1).lower()

    async def appliquer(self, coffre_a_faire: bool, en_ligne: bool = False) -> str | None:
        """Aligne le serveur sur l'attendu et prévient l'enfant connecté.

        Renvoie la nouvelle difficulté si elle a changé.
        """
        attendue = PAISIBLE if coffre_a_faire else self.difficulte_normale
        if await self._difficulte() == attendue:
            return None
        reponse = await self.jeu.commande(f"difficulty {attendue}")
        if _ECRITURE.search(reponse) is None:
            raise ErreurCoffre(f"le serveur a refusé « difficulty {attendue} » : {reponse.strip()}")
        if en_ligne:
            if coffre_a_faire:
                await self.jeu.dire(self.pseudo, MESSAGE_PAISIBLE, color="gray")
            else:
                await self.jeu.dire(self.pseudo, MESSAGE_RETOUR, color="yellow")
        return attendue
