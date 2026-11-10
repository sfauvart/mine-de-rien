"""La difficulté du serveur suit les coffres. Réponses relevées sur Minecraft Java 26.3."""

from __future__ import annotations

import asyncio
from datetime import UTC, datetime

import pytest

from mine_de_rien.exercises.base import Exercice
from mine_de_rien.games.coffre_du_soir import CoffreDuSoir, ErreurCoffre
from mine_de_rien.games.paisible import MESSAGE_PAISIBLE, MESSAGE_RETOUR, ModePaisible
from mine_de_rien.orchestrateur import Orchestrateur
from mine_de_rien.storage.sqlite import Depot

PSEUDO = "Zorglub42"
T0 = datetime(2026, 11, 9, 18, 0, tzinfo=UTC)


class FauxServeur:
    def __init__(self, difficulte="Easy", atelier=True, coffre_plein=False):
        self.difficulte = difficulte
        self.atelier = atelier
        self.coffre_plein = coffre_plein
        self.commandes: list[str] = []
        self.messages: list[str] = []

    @property
    def ecritures(self) -> list[str]:
        return [c for c in self.commandes if c.startswith("difficulty ")]

    async def commande(self, c: str) -> str:
        self.commandes.append(c)
        if c == "difficulty":
            return f"The difficulty is {self.difficulte}"
        if c.startswith("difficulty "):
            demandee = c.split()[1].capitalize()
            if demandee == self.difficulte:
                return f"The difficulty did not change; it is already set to {demandee}"
            self.difficulte = demandee
            return f"The difficulty has been set to {demandee}"
        if c == "list":
            return f"There are 1 of a max of 20 players online: {PSEUDO}"
        if c.startswith("data get storage"):
            if not self.atelier:
                return "Found no elements matching pos"
            return "Storage mine_de_rien:atelier has the following contents: [0, -60, 8]"
        if c.startswith("clear "):
            return f"No items were found on player {PSEUDO}"
        if "Items[0]" in c:
            return "Test passed" if self.coffre_plein else "Test failed"
        if c.startswith("execute if"):
            return "Test passed"
        return ""

    async def dire(self, joueur, texte, **style):
        self.messages.append(texte)


class Notif:
    async def notifier(self, message):
        pass


@pytest.fixture
def depot(tmp_path):
    d = Depot.ouvrir(tmp_path / "mdr.sqlite")
    yield d
    d.fermer()


def _ajouter(depot) -> int:
    return depot.ajouter(
        Exercice("maths/division-posee", "348 ÷ 12 = ?", "29"),
        source="expression",
        demande_par="1",
        recompense="minecraft:diamond",
        recompense_nombre=3,
    )


def _tour(depot, serveur, paisible=True):
    mode = ModePaisible(serveur, PSEUDO, "easy") if paisible else None
    orch = Orchestrateur(
        depot, CoffreDuSoir(serveur, PSEUDO), Notif(), lambda: T0, paisible=mode
    )
    asyncio.run(orch.tour())


def test_appliquer_renvoie_la_difficulte_quand_elle_change():
    serveur = FauxServeur("Easy")
    mode = ModePaisible(serveur, PSEUDO, "hard")
    assert asyncio.run(mode.appliquer(True)) == "peaceful"
    assert serveur.difficulte == "Peaceful"
    assert asyncio.run(mode.appliquer(False)) == "hard"
    assert serveur.difficulte == "Hard"


def test_difficulte_deja_correcte_aucune_ecriture():
    serveur = FauxServeur("Peaceful")
    assert asyncio.run(ModePaisible(serveur, PSEUDO).appliquer(True, en_ligne=True)) is None
    assert serveur.ecritures == []
    assert serveur.messages == []


def test_ecriture_refusee():
    class Refus(FauxServeur):
        async def commande(self, c):
            if c.startswith("difficulty "):
                return "Unknown or incomplete command"
            return await super().commande(c)

    with pytest.raises(ErreurCoffre, match="refusé"):
        asyncio.run(ModePaisible(Refus("Easy"), PSEUDO).appliquer(True))


def test_lecture_illisible():
    class Muet(FauxServeur):
        async def commande(self, c):
            return ""

    with pytest.raises(ErreurCoffre, match="illisible"):
        asyncio.run(ModePaisible(Muet(), PSEUDO).appliquer(True))


def test_un_message_a_chaque_bascule_enfant_connecte():
    serveur = FauxServeur("Easy")
    mode = ModePaisible(serveur, PSEUDO)
    asyncio.run(mode.appliquer(True, en_ligne=True))
    asyncio.run(mode.appliquer(True, en_ligne=True))
    assert serveur.messages == [MESSAGE_PAISIBLE]
    asyncio.run(mode.appliquer(False, en_ligne=True))
    assert serveur.messages == [MESSAGE_PAISIBLE, MESSAGE_RETOUR]


def test_aucun_message_enfant_deconnecte():
    serveur = FauxServeur("Easy")
    mode = ModePaisible(serveur, PSEUDO)
    assert asyncio.run(mode.appliquer(True, en_ligne=False)) == "peaceful"
    assert asyncio.run(mode.appliquer(False, en_ligne=False)) == "easy"
    assert serveur.messages == []


def test_les_messages_ne_portent_aucun_chiffre():
    assert not any(c.isdigit() for c in MESSAGE_PAISIBLE + MESSAGE_RETOUR)


def test_le_message_vient_apres_l_annonce_du_coffre(depot):
    _ajouter(depot)
    serveur = FauxServeur("Easy")
    _tour(depot, serveur)
    annonce = next(i for i, m in enumerate(serveur.messages) if "nouveau coffre" in m)
    assert serveur.messages.index(MESSAGE_PAISIBLE) > annonce


def test_coffre_actif_paisible(depot):
    depot.marquer_pose(_ajouter(depot), T0)
    serveur = FauxServeur("Easy")
    _tour(depot, serveur)
    assert serveur.difficulte == "Peaceful"


def test_coffre_pose_dans_ce_tour_paisible(depot):
    _ajouter(depot)
    serveur = FauxServeur("Easy")
    _tour(depot, serveur)
    assert depot.actif() is not None
    assert serveur.difficulte == "Peaceful"


def test_file_non_vide_sans_coffre_actif_paisible(depot):
    _ajouter(depot)
    serveur = FauxServeur("Easy", coffre_plein=True)
    _tour(depot, serveur)
    assert depot.actif() is None and depot.suivant() is not None
    assert serveur.difficulte == "Peaceful"


def test_plus_rien_a_faire_retour_a_la_normale(depot):
    serveur = FauxServeur("Peaceful")
    _tour(depot, serveur)
    assert serveur.difficulte == "Easy"


def test_atelier_absent_retour_a_la_normale(depot):
    _ajouter(depot)
    serveur = FauxServeur("Peaceful", atelier=False)
    _tour(depot, serveur)
    assert depot.suivant() is not None
    assert serveur.difficulte == "Easy"


def test_desactive_aucune_commande_difficulty(depot):
    depot.marquer_pose(_ajouter(depot), T0)
    serveur = FauxServeur("Easy")
    _tour(depot, serveur, paisible=False)
    assert serveur.difficulte == "Easy"
    assert not [c for c in serveur.commandes if c.startswith("difficulty")]
