"""Canal Telegram et commandes des parents, sans réseau."""

import asyncio
from types import SimpleNamespace

import pytest

from mine_de_rien.adapters.channel.base import MessageParent
from mine_de_rien.adapters.channel.telegram import Telegram, message_autorise
from mine_de_rien.config import Coffre
from mine_de_rien.orchestrateur import CommandesParents, servir
from mine_de_rien.storage.sqlite import Depot

PARENTS = [111, 222]


def _update(expediteur: int, texte: str, chat: str = "private"):
    reponses: list[str] = []

    async def reply_text(t):
        reponses.append(t)

    return SimpleNamespace(
        effective_user=SimpleNamespace(id=expediteur),
        effective_chat=SimpleNamespace(type=chat),
        effective_message=SimpleNamespace(text=texte, reply_text=reply_text),
        reponses=reponses,
    )


def test_seuls_les_parents_en_prive_passent():
    assert message_autorise(_update(111, "/coffre 1 + 1"), set(PARENTS)) == MessageParent(
        111, "/coffre 1 + 1"
    )
    assert message_autorise(_update(333, "/coffre 1 + 1"), set(PARENTS)) is None
    assert message_autorise(_update(111, "/coffre 1 + 1", chat="group"), set(PARENTS)) is None


def test_start_repond_a_n_importe_qui_son_identifiant():
    canal = Telegram("jeton", PARENTS)
    update = _update(333, "/start")
    asyncio.run(canal._start(update, None))
    assert update.reponses == ["Ton identifiant Telegram : 333"]


def test_message_d_un_inconnu_n_entre_pas_dans_la_file():
    canal = Telegram("jeton", PARENTS)
    asyncio.run(canal._message(_update(333, "/coffre 1 + 1"), None))
    assert canal._file.empty()
    asyncio.run(canal._message(_update(222, "/file"), None))
    assert canal._file.get_nowait() == MessageParent(222, "/file")


class FauxCoffre:
    def __init__(self):
        self.retraits = 0

    async def retirer(self):
        self.retraits += 1


@pytest.fixture
def commandes(tmp_path):
    depot = Depot.ouvrir(tmp_path / "mdr.sqlite")
    coffre = FauxCoffre()
    yield CommandesParents(depot, Coffre(), coffre, PARENTS), depot, coffre
    depot.fermer()


def _traiter(c, expediteur, texte):
    return asyncio.run(c.traiter(MessageParent(expediteur, texte)))


def test_inconnu_ne_produit_aucun_exercice(commandes):
    c, depot, _ = commandes
    assert _traiter(c, 333, "/coffre 1 + 1") is None
    assert _traiter(c, 333, "/rapport") is None
    assert depot.en_file() == []


def test_coffre_file_annuler(commandes):
    c, depot, coffre = commandes
    assert _traiter(c, 111, "/coffre 348 ÷ 12") == "Coffre n°1 en file : 348 ÷ 12 = ? (réponse 29)"
    assert _traiter(c, 222, "/coffre@MonBot 12 x 7").startswith("Coffre n°2 en file")
    assert depot.lire(1).demande_par == "111"
    assert depot.lire(1).recompense == "minecraft:diamond"

    depot.marquer_pose(1)
    file = _traiter(c, 111, "/file")
    assert "Coffre actif : n°1 348 ÷ 12 = ?" in file and "• n°2 12 × 7 = ?" in file

    assert _traiter(c, 111, "/annuler 2") == "Coffre n°2 annulé."
    assert coffre.retraits == 0
    assert _traiter(c, 111, "/annuler 1") == "Coffre n°1 annulé."
    assert coffre.retraits == 1
    assert "ni actif ni en attente" in _traiter(c, 111, "/annuler 1")
    assert "numéro" in _traiter(c, 111, "/annuler")


def test_saisie_refusee_explique(commandes):
    c, depot, _ = commandes
    assert _traiter(c, 111, "/coffre 10 ÷ 3").startswith("Refusé : ")
    assert depot.en_file() == []


def test_message_inconnu_donne_l_aide(commandes):
    c, _, _ = commandes
    assert "/coffre" in _traiter(c, 111, "bonjour")


class FauxCanal:
    def __init__(self, messages):
        self.messages = messages
        self.reponses: list[tuple[int, str]] = []

    async def recevoir(self):
        for m in self.messages:
            yield m

    async def repondre(self, destinataire, texte):
        self.reponses.append((destinataire, texte))

    async def envoyer(self, texte):
        self.reponses.append((0, texte))


def test_servir_ne_repond_qu_aux_parents(commandes):
    c, _, _ = commandes
    canal = FauxCanal([MessageParent(333, "/coffre 1 + 1"), MessageParent(111, "/coffre 1 + 1")])
    asyncio.run(servir(canal, c))
    assert canal.reponses == [(111, "Coffre n°1 en file : 1 + 1 = ? (réponse 2)")]
