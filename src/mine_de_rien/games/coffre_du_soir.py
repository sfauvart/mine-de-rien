"""Coffre du soir : un coffre verrouillé par la réponse, à renommer sur une enclume.

**Le désir.** Des diamants dans un coffre, à l'atelier près de sa base. Le coffre ne s'ouvre
qu'en tenant un objet dont le nom est la réponse : le calcul est la clé, pas une barrière
posée devant.

**Commandes**, vérifiées sur Minecraft Java 26.3 (Paper), `C` coffre, `P` panneau, `E`
enclume (voir `atelier.py`) :

    data modify block C lock set value {components:{"minecraft:custom_name":"29"}}
    item replace block C container.13 with minecraft:diamond 3
    item replace block C container.11 with minecraft:paper[minecraft:custom_name="Preuve n°12",
        minecraft:custom_data={mdr_coffre:12}]
    setblock P minecraft:oak_sign[rotation=0]{front_text:{messages:[...]},is_waxed:1b}
    setblock E minecraft:anvil[facing=east]      # enclume neuve, elle s'abîme à l'usage
    xp add <pseudo> 3 levels
    execute at <pseudo> run playsound minecraft:block.bell.use master <pseudo> ~ ~ ~ 1 1

Le verrou est posé avant d'y mettre la récompense : à aucun moment le coffre n'est ouvert
avec quelque chose dedans.

**Détection**, toutes les 3 secondes quand l'enfant est connecté :

    clear <pseudo> minecraft:paper[minecraft:custom_data={mdr_coffre:12}] 0

compte sans retirer (« Found 1 matching item(s) on player ... »). Le papier n'existe que dans
le coffre : l'avoir sur soi prouve qu'on l'a ouvert.

**Récompense** : la valeur de `[coffre]` dans `config.toml`, identique pour tous les coffres.

**Rangement** après ouverture : la preuve est retirée de l'inventaire, le panneau disparaît,
le verrou saute. Ce qui reste dans le coffre n'est pas effacé : le coffre suivant attend
qu'il soit vide, pour ne jamais détruire une récompense oubliée.

**Rapport aux parents** : coffre posé, ouvert avec la durée depuis la pose, disparu et
reposé. Jamais le pseudo de l'enfant.

**Contournements.**
- *Casser le coffre* : zone en mode aventure autour de l'atelier (datapack). Un coffre qui
  disparaît quand même (creeper, créatif) est reposé et l'événement noté. Limite connue :
  une explosion fait tomber le contenu, preuve comprise, et compte donc comme une ouverture.
- *Opérateur, créatif* : l'enfant ne doit pas être opérateur sur le serveur réel (voir
  `docs/installation.md`). En créatif, la zone ne s'applique pas.
- *Répondre au hasard* : chaque renommage coûte un niveau ; 3 niveaux sont offerts par coffre,
  au-delà c'est son expérience à lui.
- *Lire la réponse* : elle n'est que dans les données du bloc, invisibles sans `/data`
  (réservé aux opérateurs). Le chat de l'agent ne la donne jamais.
- *Faire résoudre par un autre* : possible, le rapport dit « ouvert », pas « résolu seul ».
"""

from __future__ import annotations

import json
import re
from datetime import timedelta
from typing import Protocol

from mine_de_rien.adapters.game.base import AdaptateurJeu
from mine_de_rien.games.atelier import Atelier
from mine_de_rien.minecraft import texte
from mine_de_rien.storage.sqlite import ExerciceStocke

_ERREUR_SYNTAXE = re.compile(r"<--\[HERE\]|Unknown|Incorrect argument|Expected|Invalid")


class ErreurCoffre(RuntimeError):
    pass


class Notificateur(Protocol):
    async def notifier(self, message: str) -> None:
        """Prévient les parents. Le message ne contient jamais le pseudo de l'enfant."""


class NotificateurConsole:
    """En attendant le canal Telegram : les notifications s'affichent dans la console."""

    async def notifier(self, message: str) -> None:
        print(message, flush=True)


def _pos(p: tuple[int, int, int]) -> str:
    return f"{p[0]} {p[1]} {p[2]}"


def _snbt(chaine: str) -> str:
    """Chaîne SNBT entre guillemets doubles (sous-ensemble JSON), échappée."""
    return json.dumps(chaine, ensure_ascii=False)


def predicat_preuve(numero: int) -> str:
    return f"minecraft:paper[minecraft:custom_data={{mdr_coffre:{numero}}}]"


def formater_duree(duree: timedelta | None) -> str:
    if duree is None:
        return "?"
    minutes = int(duree.total_seconds() // 60)
    if minutes < 1:
        return "moins d'une minute"
    if minutes < 60:
        return f"{minutes} min"
    heures, minutes = divmod(minutes, 60)
    if heures < 48:
        return f"{heures} h {minutes:02d}"
    return f"{heures // 24} jours"


def commandes_pose(a: Atelier, s: ExerciceStocke) -> list[str]:
    """Commandes qui posent le coffre `s` dans l'atelier `a`. Aucune réponse calculée ici."""
    lignes = texte.lignes_panneau(s.exercice.enonce)
    if lignes is None:
        raise ErreurCoffre(f"énoncé trop long pour le panneau : {s.exercice.enonce!r}")
    c, p, e = _pos(a.coffre), _pos(a.panneau), _pos(a.enclume)
    messages = ",".join(_snbt(texte.nettoyer(ligne)) for ligne in lignes)
    nom_preuve = _snbt(texte.nettoyer(f"Preuve n°{s.id}"))
    cle = _snbt(s.exercice.reponse)
    preuve = (
        f"minecraft:paper[minecraft:custom_name={nom_preuve},"
        f"minecraft:custom_data={{mdr_coffre:{s.id}}}]"
    )
    panneau = f"minecraft:oak_sign[rotation=0]{{front_text:{{messages:[{messages}]}},is_waxed:1b}}"
    return [
        f"execute unless block {c} minecraft:chest run setblock {c} minecraft:chest[facing=south]",
        f'data modify block {c} lock set value {{components:{{"minecraft:custom_name":{cle}}}}}',
        f"item replace block {c} container.13 with {s.recompense} {s.recompense_nombre}",
        f"item replace block {c} container.11 with {preuve}",
        f"setblock {p} {panneau}",
        f"setblock {e} minecraft:anvil[facing=east]",
    ]


def commandes_rangement(a: Atelier, pseudo: str, numero: int) -> list[str]:
    """Après ouverture : preuve retirée, panneau enlevé, verrou ôté. Le contenu reste."""
    return [
        f"clear {texte.verifier_pseudo(pseudo)} {predicat_preuve(numero)}",
        f"setblock {_pos(a.panneau)} minecraft:air",
        f"data remove block {_pos(a.coffre)} lock",
    ]


def commandes_retrait(a: Atelier) -> list[str]:
    """Coffre annulé : il n'a pas été gagné, on le vide entièrement."""
    return [
        f"setblock {_pos(a.panneau)} minecraft:air",
        f"data remove block {_pos(a.coffre)} lock",
        f"data modify block {_pos(a.coffre)} Items set value []",
    ]


class CoffreDuSoir:
    def __init__(
        self, jeu: AdaptateurJeu, atelier: Atelier, pseudo: str, niveaux_offerts: int = 3
    ) -> None:
        self.jeu = jeu
        self.atelier = atelier
        self.pseudo = texte.verifier_pseudo(pseudo)
        self.niveaux_offerts = niveaux_offerts

    async def _executer(self, commandes: list[str]) -> None:
        for commande in commandes:
            reponse = await self.jeu.commande(commande)
            if _ERREUR_SYNTAXE.search(reponse) or "not loaded" in reponse:
                raise ErreurCoffre(f"le serveur a refusé « {commande} » : {reponse.strip()}")

    async def _teste(self, commande: str) -> bool:
        return (await self.jeu.commande(commande)).strip().startswith("Test passed")

    async def joueur_en_ligne(self) -> bool:
        reponse = await self.jeu.commande("list")
        if ":" not in reponse:
            return False
        noms = {nom.strip() for nom in reponse.rsplit(":", 1)[1].split(",")}
        return self.pseudo in noms

    async def poser(self, s: ExerciceStocke) -> None:
        await self._executer(commandes_pose(self.atelier, s))
        c = _pos(self.atelier.coffre)
        if not (
            await self._teste(f"execute if data block {c} lock")
            and await self._teste(
                f"execute if items block {c} container.* {predicat_preuve(s.id)}"
            )
        ):
            raise ErreurCoffre(f"le coffre n°{s.id} n'a pas été posé correctement")

    async def annoncer(self, s: ExerciceStocke) -> None:
        """Message, cloche et niveaux offerts : l'enfant doit être connecté."""
        await self.jeu.dire(
            self.pseudo, "Un nouveau coffre est apparu à l'atelier !", color="gold"
        )
        await self.jeu.commande(
            f"execute at {self.pseudo} run playsound minecraft:block.bell.use master "
            f"{self.pseudo} ~ ~ ~ 1 1"
        )

    async def donner_niveaux(self) -> None:
        if self.niveaux_offerts:
            await self.jeu.commande(f"xp add {self.pseudo} {self.niveaux_offerts} levels")

    async def rappeler(self, s: ExerciceStocke) -> None:
        await self.jeu.dire(
            self.pseudo, f"Un coffre t'attend à l'atelier : {s.exercice.enonce}", color="gold"
        )

    async def demander_de_vider(self) -> None:
        await self.jeu.dire(
            self.pseudo,
            "Vide le coffre de l'atelier : le suivant attend qu'il soit vide.",
            color="gray",
        )

    async def preuve_trouvee(self, s: ExerciceStocke) -> bool:
        reponse = await self.jeu.commande(f"clear {self.pseudo} {predicat_preuve(s.id)} 0")
        return reponse.strip().startswith("Found")

    async def coffre_present(self) -> bool:
        return await self._teste(f"execute if block {_pos(self.atelier.coffre)} minecraft:chest")

    async def coffre_vide(self) -> bool:
        c = _pos(self.atelier.coffre)
        if not await self.coffre_present():
            return True
        return not await self._teste(f"execute if data block {c} Items[0]")

    async def ranger(self, s: ExerciceStocke) -> None:
        await self._executer(commandes_rangement(self.atelier, self.pseudo, s.id))

    async def retirer(self) -> None:
        await self._executer(commandes_retrait(self.atelier))
