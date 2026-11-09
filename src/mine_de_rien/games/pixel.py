"""Pixel : l'assistant que l'enfant appelle dans le chat, pour revenir au coffre ou être aidé.

**Le désir.** L'enfant joue loin de l'atelier, ou il bloque devant le coffre. Il tape
« Pixel, ramène-moi au coffre » ou « Pixel, aide-moi » ; il se retrouve devant le coffre, ou
reçoit un indice, de plus en plus précis à chaque demande, jamais la réponse.

**Déclencheur.** Seuls les messages du pseudo de l'enfant qui commencent par le nom du
personnage (casse et accents ignorés), suivi d'une virgule, de deux-points ou d'une espace.
Le reste du chat n'est ni lu plus loin, ni envoyé nulle part. Pixel ne parle jamais le
premier : seule une ligne de l'annonce du coffre dit qu'il existe.

**Classement.** Des mots-clés d'abord (« aide », « ramène »…) : à l'essai du 2026-11-08, le
LLM rangeait « aide-moi » dans le bavardage. **Le LLM, s'il est local**, ne classe que ce que
les mots-clés ne reconnaissent pas, dans une liste fermée (`teleporter`, `indice`,
`bavardage`), et reformule l'indice calculé par `exercises/indices.py`. Il reçoit le message
(200 caractères au plus, sans le nom), l'énoncé et l'indice, jamais la réponse ni le pseudo.
Il n'écrit aucune commande : c'est ce module qui agit. Sans LLM, avec un LLM distant ou en
erreur, les mots-clés suffisent et l'indice part tel quel.

**Garde-fous sur ce que dit le LLM**, appliqués en Python :
- une reformulation qui contient la réponse (`contient_la_reponse`), un nombre absent de
  l'indice et de l'énoncé (calcul inventé), ou plus de 200 caractères est remplacée par
  l'indice brut ;
- un bavardage qui contient un chiffre est remplacé par une phrase fixe : Pixel n'est pas une
  calculatrice pour les vrais devoirs ;
- les émojis sont retirés : le chat de Minecraft ne les affiche pas.

**Commande**, vérifiée sur Minecraft Java 26.3 (Paper), `C` = coffre de l'atelier :

    execute in minecraft:overworld run tp <pseudo> Cx Cy Cz+2 180 0

depuis n'importe quelle dimension : l'enfant arrive au centre du bloc, sur le plancher au sud
du coffre, face à lui (« Teleported … », ou « No entity was found » s'il est déconnecté).
Seulement si l'atelier est construit dans ce monde (`atelier.probleme_atelier`) : ailleurs,
rien ne garantit que ce bloc soit de l'air.

**Rapport.** Chaque indice et chaque téléportation est noté dans `evenements` avec le numéro
du coffre ; aucun texte du chat n'est gardé.

**Contournements.**
- *Faire agir Pixel* (objets, heure, mode de jeu) : liste fermée, aucune commande libre.
- *Lui faire dire la réponse* : le LLM ne la connaît pas, et le filtre la retire quand même.
- *S'en servir pour les vrais devoirs* : il ne répond à aucun calcul (chiffres interdits).
- *Se téléporter pour fuir un danger* : seulement quand un coffre attend, une fois toutes les
  5 minutes, et toujours au même endroit.
- *Enchaîner les indices* : permis (choix des parents), le dernier palier se répète, et les
  parents voient le compte.
- *Un autre joueur qui appelle Pixel* : ignoré, seul le pseudo de l'enfant compte.
"""

from __future__ import annotations

import asyncio
import logging
import re
import unicodedata
from collections.abc import Callable
from datetime import datetime, timedelta

from mine_de_rien.adapters.game.base import AdaptateurJeu, MessageChat
from mine_de_rien.adapters.game.rcon import ErreurRcon
from mine_de_rien.adapters.llm.base import ErreurLlm, Llm, lire_json
from mine_de_rien.exercises.base import normaliser
from mine_de_rien.exercises.indices import contient_la_reponse, indices
from mine_de_rien.games.atelier import Atelier, probleme_atelier
from mine_de_rien.minecraft import texte
from mine_de_rien.storage.sqlite import Depot, ErreurStockage, maintenant

journal = logging.getLogger(__name__)

NOM_DEFAUT = "Pixel"
_NOM = re.compile(r"[A-Za-zÀ-ÖØ-öø-ÿ]{2,16}")
LONGUEUR_MESSAGE = 200
LONGUEUR_REPONSE = 200
DELAI_TELEPORTATION = timedelta(minutes=5)
COULEUR = "aqua"

TELEPORTER = "teleporter"
INDICE = "indice"
BAVARDAGE = "bavardage"
INTENTIONS = (TELEPORTER, INDICE, BAVARDAGE)

EVENEMENT_INDICE = "indice"
EVENEMENT_TELEPORTATION = "teleportation"

_MOTS_TELEPORTER = ("ramene", "teleporte", "teleport", "tp", "amene", "emmene", "retour")
_MOTS_INDICE = (
    "aide", "indice", "bloque", "help", "comment", "sais pas", "trouve pas", "comprends pas",
    "comprend pas", "combien", "reponse", "veut dire", "signifie",
)
_CHIFFRE = re.compile(r"\d")
_NOMBRE = re.compile(r"\d+(?:[.,]\d+)?")

SYSTEME_CLASSEMENT = (
    "Tu ranges le message d'un enfant qui joue à Minecraft dans une seule catégorie : "
    "\"teleporter\" s'il veut aller ou revenir au coffre, \"indice\" s'il demande de l'aide "
    "pour trouver la réponse du coffre, \"bavardage\" pour tout le reste."
)
CONSIGNE_CLASSEMENT = 'Réponds en JSON : {{"intention": "..."}}. Message : « {message} »'
SYSTEME_REFORMULATION = (
    "Tu es Pixel, un assistant gentil dans Minecraft, qui parle à un enfant de 10 ans. Tu "
    "reformules l'indice qu'on te donne en une ou deux phrases courtes, chaleureuses, en "
    "français, sans émoji. Tu n'ajoutes aucune information, aucun calcul, aucun résultat, et "
    "tu ne dis pas que l'enfant a déjà trouvé quelque chose."
)
SYSTEME_BAVARDAGE = (
    "Tu es Pixel, un assistant gentil dans Minecraft, qui parle à un enfant de 10 ans. Tu "
    "réponds en une ou deux phrases courtes, en français, sans aucun chiffre ni calcul, sans "
    "émoji. Tu ne peux rien faire dans le jeu, à part donner un indice pour le coffre ou "
    "ramener l'enfant au coffre : ne prétends jamais avoir fait autre chose."
)

ATTENTE = "…"
OCCUPE = "Je réfléchis encore, une question à la fois !"
PAS_DE_COFFRE = "Pas de coffre en ce moment. Je te préviendrai par l'annonce."
PATIENCE = "Encore un peu de patience : une téléportation toutes les 5 minutes."
TELEPORTE = "Te voilà devant le coffre !"
HORS_LIGNE = "Je n'arrive pas à te déplacer, réessaie dans un instant."
ATELIER_ABSENT = (
    "L'atelier n'est pas encore construit, je ne peux pas t'y emmener. Préviens tes parents."
)
PHRASE_CALCUL = (
    "Je ne fais pas les calculs, mais je peux t'aider pour le coffre : dis « {nom}, aide-moi »."
)
INCOMPRIS = "Dis-moi « {nom}, aide-moi » ou « {nom}, ramène-moi au coffre »."


def verifier_nom(nom: str) -> str:
    if not _NOM.fullmatch(nom):
        raise ValueError(f"[assistant] nom : 2 à 16 lettres, sans espace ({nom!r})")
    return nom


def adresse(message: str, nom: str) -> str | None:
    """Le reste du message s'il commence par `nom` (« Pixel, aide-moi » → « aide-moi »)."""
    debut = message.lstrip()
    tete = debut[: len(nom)]
    if normaliser(tete) != normaliser(nom):
        return None
    reste = debut[len(nom) :]
    if reste and reste[0] not in ",: \t":
        return None
    return reste.lstrip(",: \t").strip()


def _sans_accents(message: str) -> str:
    return " " + re.sub(r"[^a-z]+", " ", normaliser_espaces(message)) + " "


def normaliser_espaces(message: str) -> str:
    """Minuscules sans accents, espaces gardées (contrairement à `normaliser`)."""
    return " ".join(normaliser(mot) for mot in message.split())


def classer_par_mots_cles(message: str) -> str:
    mots = _sans_accents(message)
    if any(f" {m}" in mots for m in _MOTS_TELEPORTER):
        return TELEPORTER
    if any(f" {m}" in mots for m in _MOTS_INDICE):
        return INDICE
    return BAVARDAGE


def commande_teleportation(atelier: Atelier, pseudo: str) -> str:
    """Devant le coffre, sur le plancher au sud, regard vers le nord (vers le coffre)."""
    x, y, z = atelier.coffre
    return (
        f"execute in minecraft:overworld run tp {texte.verifier_pseudo(pseudo)} "
        f"{x} {y} {z + 2} 180 0"
    )


def propre(brut: str) -> str:
    """Réponse du LLM affichable : sans émojis ni symboles graphiques, espaces simples."""
    garde = "".join(
        c for c in texte.nettoyer(brut)
        if unicodedata.category(c) not in ("So", "Cs", "Co") and not 0xFE00 <= ord(c) <= 0xFE0F
    )
    return " ".join(garde.split())


def _nombres(chaine: str) -> set[str]:
    return {n.replace(".", ",") for n in _NOMBRE.findall(chaine.replace(" ", ""))}


class Pixel:
    def __init__(
        self,
        jeu: AdaptateurJeu,
        depot: Depot,
        atelier: Atelier,
        pseudo: str,
        nom: str = NOM_DEFAUT,
        llm: Llm | None = None,
        horloge: Callable[[], datetime] = maintenant,
    ) -> None:
        self.jeu = jeu
        self.depot = depot
        self.atelier = atelier
        self.pseudo = texte.verifier_pseudo(pseudo)
        self.nom = verifier_nom(nom)
        # Le message de l'enfant ne part jamais vers un LLM distant.
        self.llm = llm if llm is not None and not llm.distant else None
        self.horloge = horloge
        self._tache: asyncio.Task | None = None

    async def dire(self, phrase: str, **style: str | bool) -> None:
        await self.jeu.dire(self.pseudo, f"<{self.nom}> {phrase}", **({"color": COULEUR} | style))

    async def ecouter(self) -> None:
        """Suit le chat aussi longtemps que l'agent tourne."""
        async for message in self.jeu.chat():
            try:
                await self.recevoir(message)
            except (ErreurRcon, ErreurStockage) as e:
                journal.warning("Pixel n'a pas pu répondre : %s", e)

    async def recevoir(self, message: MessageChat) -> None:
        """Filtre, puis traite en tâche de fond : le chat continue d'être lu pendant ce temps."""
        demande = self._demande(message)
        if demande is None:
            return
        if self._tache is not None and not self._tache.done():
            await self.dire(OCCUPE, color="gray")
            return
        await self.dire(ATTENTE, color="gray")
        self._tache = asyncio.create_task(self._repondre_sans_echec(demande))

    def _demande(self, message: MessageChat) -> str | None:
        if message.joueur != self.pseudo:
            return None
        reste = adresse(message.texte, self.nom)
        return None if reste is None else reste[:LONGUEUR_MESSAGE]

    async def _repondre_sans_echec(self, demande: str) -> None:
        try:
            await self.repondre(demande)
        except (ErreurRcon, ErreurStockage) as e:
            journal.warning("Pixel n'a pas pu répondre : %s", e)

    async def repondre(self, demande: str) -> None:
        if not demande:
            await self.dire(INCOMPRIS.format(nom=self.nom))
            return
        intention = await self.classer(demande)
        if intention == TELEPORTER:
            await self._teleporter()
        elif intention == INDICE:
            await self._indice(demande)
        else:
            await self._bavarder(demande)

    async def classer(self, demande: str) -> str:
        par_mots = classer_par_mots_cles(demande)
        if par_mots != BAVARDAGE or self.llm is None:
            return par_mots
        try:
            brut = await self.llm.completer(
                SYSTEME_CLASSEMENT, CONSIGNE_CLASSEMENT.format(message=demande), en_json=True
            )
            intention = lire_json(brut).get("intention")
        except ErreurLlm as e:
            journal.info("classement par mots-clés, LLM indisponible : %s", e)
            return BAVARDAGE
        return intention if intention in INTENTIONS else BAVARDAGE

    async def _teleporter(self) -> None:
        actif = self.depot.actif()
        if actif is None:
            await self.dire(PAS_DE_COFFRE)
            return
        maintenant_ = self.horloge()
        recents = self.depot.evenements_entre(maintenant_ - DELAI_TELEPORTATION, maintenant_)
        if any(e.genre == EVENEMENT_TELEPORTATION for e in recents):
            await self.dire(PATIENCE)
            return
        probleme = await probleme_atelier(self.jeu, self.atelier)
        if probleme:
            journal.warning("téléportation refusée : %s", probleme)
            await self.dire(ATELIER_ABSENT)
            return
        reponse = await self.jeu.commande(commande_teleportation(self.atelier, self.pseudo))
        if not reponse.startswith("Teleported"):
            await self.dire(HORS_LIGNE)
            return
        self.depot.noter(EVENEMENT_TELEPORTATION, actif.id, maintenant_)
        await self.dire(TELEPORTE)

    async def _indice(self, demande: str) -> None:
        actif = self.depot.actif()
        if actif is None:
            await self.dire(PAS_DE_COFFRE)
            return
        ex = actif.exercice
        paliers = indices(ex)
        deja = self.depot.compter(EVENEMENT_INDICE, actif.id)
        palier = paliers[min(deja, len(paliers) - 1)]
        phrase = palier
        if self.llm is not None:
            try:
                brut = await self.llm.completer(
                    SYSTEME_REFORMULATION,
                    f"Énoncé du coffre : {ex.enonce}\nIndice à reformuler : {palier}\n"
                    f"Message de l'enfant : {demande}",
                )
                phrase = self._reformulation_sure(brut, palier, actif.exercice) or palier
            except ErreurLlm as e:
                journal.info("indice brut, LLM indisponible : %s", e)
        self.depot.noter(EVENEMENT_INDICE, actif.id, self.horloge())
        await self.dire(phrase)

    @staticmethod
    def _reformulation_sure(brut: str, palier: str, ex) -> str | None:
        phrase = propre(brut)
        if not phrase or len(phrase) > LONGUEUR_REPONSE or contient_la_reponse(phrase, ex):
            return None
        if not _nombres(phrase) <= _nombres(palier) | _nombres(ex.enonce):
            return None
        return phrase

    async def _bavarder(self, demande: str) -> None:
        if self.llm is None:
            await self.dire(INCOMPRIS.format(nom=self.nom))
            return
        try:
            brut = await self.llm.completer(SYSTEME_BAVARDAGE, demande)
        except ErreurLlm as e:
            journal.info("bavardage sans LLM : %s", e)
            await self.dire(INCOMPRIS.format(nom=self.nom))
            return
        phrase = propre(brut)
        actif = self.depot.actif()
        if (
            not phrase
            or _CHIFFRE.search(phrase)
            or len(phrase) > LONGUEUR_REPONSE
            or (actif is not None and contient_la_reponse(phrase, actif.exercice))
        ):
            phrase = PHRASE_CALCUL.format(nom=self.nom)
        await self.dire(phrase)
