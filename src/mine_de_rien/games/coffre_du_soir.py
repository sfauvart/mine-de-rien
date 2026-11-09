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
    give <pseudo> minecraft:trial_key[minecraft:item_name="Clé du coffre",
        minecraft:lore=[...],minecraft:custom_data={mdr_cle:1b}] 1
    xp add <pseudo> 3 levels
    execute at <pseudo> run playsound minecraft:block.bell.use master <pseudo> ~ ~ ~ 1 1

L'atelier est relu dans le monde à chaque tour de surveillance (`actualiser_atelier`). S'il
n'y en a pas, tout ce qui a besoin de sa position refuse avec `AtelierAbsent`, sans rien poser.

Le verrou est posé avant d'y mettre la récompense : à aucun moment le coffre n'est ouvert
avec quelque chose dedans.

**La clé** : une clé (`trial_key`) dont la description explique les gestes, donnée avec les
niveaux si l'enfant n'en a plus une sur lui. C'est elle qu'il renomme : il n'a pas à deviner
quel objet poser sur l'enclume. Elle sert d'un coffre à l'autre, renommée à chaque fois
(un simple renommage coûte toujours un niveau, pas plus). N'importe quel autre objet renommé
ouvre aussi le coffre : le verrou ne regarde que le nom. Le mode d'emploi est répété dans le
chat à chaque annonce et à chaque rappel, car l'enclume ne s'explique pas d'elle-même : le
champ du nom reste grisé tant que rien n'est posé à gauche, et il n'y a pas de bouton pour
valider, on clique sur le résultat pour le prendre.

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

import asyncio
import json
import re
from datetime import timedelta
from typing import Protocol

from mine_de_rien.adapters.game.base import AdaptateurJeu
from mine_de_rien.games.atelier import PAS_CONSTRUIT, Atelier, lire_atelier
from mine_de_rien.minecraft import texte
from mine_de_rien.storage.sqlite import ExerciceStocke

_ERREUR_SYNTAXE = re.compile(r"<--\[HERE\]|Unknown|Incorrect argument|Expected|Invalid")


class ErreurCoffre(RuntimeError):
    pass


class AtelierAbsent(ErreurCoffre):
    """Ce monde n'a pas d'atelier ; le message va aux parents."""


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


CLE = "minecraft:trial_key"
NOM_CLE = "Clé du coffre"
MODE_EMPLOI_CLE = (
    "1. Pose-moi à gauche sur l'enclume",
    "2. Écris la réponse, en minuscules",
    "3. Clique sur moi à droite",
    "4. Tiens-moi en main, ouvre le coffre",
)
MODE_EMPLOI = (
    "Pour l'ouvrir : pose ta clé à gauche sur l'enclume, remplace son nom par la réponse "
    "du panneau, écrite en minuscules, clique sur la clé renommée à droite pour la prendre, "
    "puis ouvre le coffre en la tenant en main."
)


def predicat_cle() -> str:
    return f"{CLE}[minecraft:custom_data={{mdr_cle:1b}}]"


def commande_cle(pseudo: str) -> str:
    """`give` d'une clé neuve, avec son mode d'emploi dans sa description."""
    lore = ",".join(
        json.dumps({"text": ligne, "italic": False, "color": "gray"}, ensure_ascii=False)
        for ligne in MODE_EMPLOI_CLE
    )
    return (
        f"give {texte.verifier_pseudo(pseudo)} {CLE}[minecraft:item_name={_snbt(NOM_CLE)},"
        f"minecraft:lore=[{lore}],minecraft:custom_data={{mdr_cle:1b}}] 1"
    )


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
        self,
        jeu: AdaptateurJeu,
        pseudo: str,
        niveaux_offerts: int = 3,
        assistant: str | None = None,
    ) -> None:
        self.jeu = jeu
        self.atelier: Atelier | None = None
        # Tenu pendant un tour de surveillance et pendant /atelier : pas de pose en plein
        # déplacement.
        self.verrou = asyncio.Lock()
        self.pseudo = texte.verifier_pseudo(pseudo)
        self.niveaux_offerts = niveaux_offerts
        # Nom de Pixel s'il écoute le chat : le mode d'emploi dit qu'on peut l'appeler.
        self.assistant = assistant

    async def _mode_emploi(self) -> None:
        await self.jeu.dire(self.pseudo, MODE_EMPLOI, color="gray")
        if self.assistant:
            await self.jeu.dire(
                self.pseudo,
                f"Bloqué ou loin ? Écris « {self.assistant}, aide-moi » ou "
                f"« {self.assistant}, ramène-moi au coffre » dans le chat.",
                color="gray",
            )

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

    async def actualiser_atelier(self) -> None:
        """Relit la position dans le monde : elle peut changer pendant que l'agent tourne."""
        self.atelier = await lire_atelier(self.jeu)

    def verifier_atelier(self) -> Atelier:
        if self.atelier is None:
            raise AtelierAbsent(PAS_CONSTRUIT)
        return self.atelier

    async def poser(self, s: ExerciceStocke) -> None:
        a = self.verifier_atelier()
        await self._executer(commandes_pose(a, s))
        c = _pos(a.coffre)
        if not (
            await self._teste(f"execute if data block {c} lock")
            and await self._teste(
                f"execute if items block {c} container.* {predicat_preuve(s.id)}"
            )
        ):
            raise ErreurCoffre(f"le coffre n°{s.id} n'a pas été posé correctement")

    async def annoncer(self, s: ExerciceStocke) -> None:
        """Message, mode d'emploi et cloche : l'enfant doit être connecté."""
        await self.jeu.dire(
            self.pseudo, "Un nouveau coffre est apparu à l'atelier !", color="gold"
        )
        await self._mode_emploi()
        await self.jeu.commande(
            f"execute at {self.pseudo} run playsound minecraft:block.bell.use master "
            f"{self.pseudo} ~ ~ ~ 1 1"
        )

    async def a_une_cle(self) -> bool:
        reponse = await self.jeu.commande(f"clear {self.pseudo} {predicat_cle()} 0")
        return reponse.strip().startswith("Found")

    async def donner_cle(self) -> None:
        """Une clé seulement s'il n'en a plus : elle sert d'un coffre à l'autre."""
        if not await self.a_une_cle():
            await self._executer([commande_cle(self.pseudo)])

    async def equiper(self) -> None:
        """Pour un nouveau coffre : la clé s'il n'en a plus, et les niveaux offerts."""
        await self.donner_cle()
        if self.niveaux_offerts:
            await self.jeu.commande(f"xp add {self.pseudo} {self.niveaux_offerts} levels")

    async def rappeler(self, s: ExerciceStocke) -> None:
        await self.donner_cle()
        await self.jeu.dire(
            self.pseudo, f"Un coffre t'attend à l'atelier : {s.exercice.enonce}", color="gold"
        )
        await self._mode_emploi()

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
        c = _pos(self.verifier_atelier().coffre)
        return await self._teste(f"execute if block {c} minecraft:chest")

    async def coffre_vide(self) -> bool:
        c = _pos(self.verifier_atelier().coffre)
        if not await self.coffre_present():
            return True
        return not await self._teste(f"execute if data block {c} Items[0]")

    async def ranger(self, s: ExerciceStocke) -> None:
        await self._executer(commandes_rangement(self.verifier_atelier(), self.pseudo, s.id))

    async def retirer(self) -> None:
        await self.actualiser_atelier()  # appelé depuis /annuler, hors de la boucle
        await self._executer(commandes_retrait(self.verifier_atelier()))
