"""Photo d'une fiche depuis le canal : lecture, tirage, /oui, /non. Sans réseau ni LLM réel."""

from __future__ import annotations

import asyncio
import io
import json
import random
from datetime import UTC, datetime, timedelta

import pytest
from PIL import Image

from mine_de_rien.adapters.channel.base import MessageParent
from mine_de_rien.config import Coffre
from mine_de_rien.orchestrateur import LECTURE_EN_COURS, CommandesParents, servir
from mine_de_rien.storage.sqlite import Depot

PARENTS = [111, 222]
DOUBLES = {"lignes": [{"genre": "calcul", "texte": f"Double de {n}"} for n in range(1, 21)]}


def _jpeg(largeur: int = 600, hauteur: int = 800) -> bytes:
    tampon = io.BytesIO()
    Image.new("RGB", (largeur, hauteur), "white").save(tampon, format="JPEG")
    return tampon.getvalue()


class FauxLlm:
    def __init__(self, reponse: dict, distant: bool = False) -> None:
        self.reponse = reponse
        self.distant = distant
        self.consignes: list[str] = []

    async def completer(self, systeme, utilisateur, image=None, en_json=False):
        self.consignes.append(utilisateur)
        return json.dumps(self.reponse)


class Horloge:
    def __init__(self) -> None:
        self.t = datetime(2026, 11, 8, 18, 0, tzinfo=UTC)

    def __call__(self) -> datetime:
        return self.t


@pytest.fixture
def depot(tmp_path):
    d = Depot.ouvrir(tmp_path / "mdr.sqlite")
    yield d
    d.fermer()


def _commandes(depot, llm, horloge=None):
    return CommandesParents(
        depot, Coffre(), None, PARENTS, horloge=horloge or Horloge(), llm=llm,
        rng=random.Random(0),
    )


def _photo(c, expediteur=111, legende=""):
    return asyncio.run(c.traiter(MessageParent(expediteur, legende, image=_jpeg())))


def _texte(c, texte, expediteur=111):
    return asyncio.run(c.traiter(MessageParent(expediteur, texte)))


def test_photo_d_un_inconnu_ignoree(depot):
    llm = FauxLlm(DOUBLES)
    c = _commandes(depot, llm)
    assert _photo(c, expediteur=333) is None
    assert c.accuse_reception(MessageParent(333, "", image=b"x")) is None
    assert llm.consignes == []


def test_llm_distant_refuse_sans_appel(depot):
    llm = FauxLlm(DOUBLES, distant=True)
    c = _commandes(depot, llm)
    assert c.accuse_reception(MessageParent(111, "", image=b"x")) is None
    assert "LLM local" in _photo(c)
    assert llm.consignes == []


def test_sans_llm_configure(depot):
    assert "pas configurée" in _photo(_commandes(depot, None))


def test_legende_nombre_et_consigne(depot):
    llm = FauxLlm(DOUBLES)
    c = _commandes(depot, llm)
    reponse = _photo(c, legende="5 exercice 7")
    assert "exercice 7" in llm.consignes[0]
    assert "Doubles de (20)" in reponse
    assert "Tirés au hasard (5)" in reponse
    assert "Compare cette liste à la fiche" in reponse
    assert len(c.en_attente[111].exercices) == 5
    assert depot.en_file() == []


def test_legende_par_defaut_et_bornes(depot):
    c = _commandes(depot, FauxLlm(DOUBLES))
    assert "Tirés au hasard (3)" in _photo(c)
    assert "Entre 1 et 10" in _photo(c, legende="11")
    assert "Entre 1 et 10" in _photo(c, legende="0")


def test_oui_met_en_file_avec_la_source_photo(depot):
    c = _commandes(depot, FauxLlm(DOUBLES))
    _photo(c, legende="2")
    reponse = _texte(c, "/oui")
    assert reponse.count("en file") == 2
    file = depot.en_file()
    assert [s.source for s in file] == ["photo", "photo"]
    assert all(s.demande_par == "111" for s in file)
    assert file[0].exercice.enonce.startswith("Double de ")
    assert "Rien à confirmer" in _texte(c, "/oui")


def test_non_abandonne(depot):
    c = _commandes(depot, FauxLlm(DOUBLES))
    _photo(c)
    assert "Abandonné" in _texte(c, "/non")
    assert depot.en_file() == []
    assert "Aucune lecture" in _texte(c, "/non")


def test_lecture_expiree_apres_30_minutes(depot):
    horloge = Horloge()
    c = _commandes(depot, FauxLlm(DOUBLES), horloge)
    _photo(c)
    horloge.t += timedelta(minutes=31)
    assert "Rien à confirmer" in _texte(c, "/oui")
    assert depot.en_file() == []


def test_une_lecture_par_parent(depot):
    c = _commandes(depot, FauxLlm(DOUBLES))
    _photo(c, expediteur=111, legende="1")
    _photo(c, expediteur=222, legende="2")
    assert "Rien à confirmer" not in _texte(c, "/oui", expediteur=222)
    assert len(depot.en_file()) == 2
    assert len(c.en_attente[111].exercices) == 1


def test_rien_reconnu(depot):
    c = _commandes(depot, FauxLlm({"lignes": [{"genre": "mot", "texte": "Greetings"}]}))
    assert _photo(c) == "Je n'ai rien reconnu. La photo est-elle à l'endroit ?"
    assert c.en_attente == {}


def test_reponse_illisible_du_llm(depot):
    class Bavard(FauxLlm):
        async def completer(self, *a, **k):
            return "Voici ce que je vois"

    c = _commandes(depot, Bavard(DOUBLES))
    assert _photo(c).startswith("Lecture impossible")


class FauxCanal:
    def __init__(self, messages):
        self.messages = messages
        self.reponses: list[tuple[int, str]] = []

    async def recevoir(self):
        for m in self.messages:
            yield m

    async def repondre(self, destinataire, texte):
        self.reponses.append((destinataire, texte))


def test_servir_previent_avant_de_lire(depot):
    c = _commandes(depot, FauxLlm(DOUBLES))
    canal = FauxCanal([MessageParent(111, "2", image=_jpeg())])
    asyncio.run(servir(canal, c))
    assert canal.reponses[0] == (111, LECTURE_EN_COURS)
    assert "Tirés au hasard (2)" in canal.reponses[1][1]
