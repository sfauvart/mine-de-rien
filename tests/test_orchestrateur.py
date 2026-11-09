"""Photo d'une fiche depuis le canal : lecture, tirage, /oui, /non. Sans réseau ni LLM réel."""

from __future__ import annotations

import asyncio
import io
import json
import random
from datetime import UTC, datetime, timedelta

import pytest
from PIL import Image

from mine_de_rien import photo
from mine_de_rien.adapters.channel.base import MessageParent
from mine_de_rien.config import Coffre
from mine_de_rien.games.coffre_du_soir import CoffreDuSoir
from mine_de_rien.games.emplacement import AUCUN_TERRAIN, ENFANT_ABSENT
from mine_de_rien.orchestrateur import (
    LECTURE_EN_COURS,
    RECHERCHE_EN_COURS,
    CommandesParents,
    servir,
)
from mine_de_rien.storage.sqlite import Depot
from tests.test_emplacement import Terrain, Y

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


def _atelier(depot, terrain):
    return CommandesParents(depot, Coffre(), CoffreDuSoir(terrain, "Steve_42"), PARENTS)


def test_atelier_construit_sans_le_pseudo_dans_la_reponse(depot):
    terrain = Terrain()
    reponse = _texte(_atelier(depot, terrain), "/atelier")
    assert reponse == "Atelier construit en 92 64 192, à 11 blocs de l'enfant."
    assert "Steve" not in reponse


def test_atelier_inconnu_ignore(depot):
    terrain = Terrain()
    c = _atelier(depot, terrain)
    assert _texte(c, "/atelier", expediteur=333) is None
    assert c.accuse_reception(MessageParent(333, "/atelier")) is None
    assert terrain.recues == []


def test_atelier_accuse_reception_pour_un_parent(depot):
    c = _atelier(depot, Terrain())
    assert c.accuse_reception(MessageParent(111, "/atelier")) == RECHERCHE_EN_COURS
    assert c.accuse_reception(MessageParent(111, "/rapport")) is None


def test_atelier_enfant_hors_ligne_dit_quoi_faire(depot):
    assert _texte(_atelier(depot, Terrain(joueur=None)), "/atelier") == ENFANT_ABSENT


ANCIEN = [40, 64, 40]
MODIFIANTES = (
    "fill", "setblock", "data modify", "data remove", "forceload",
    "kill @e[type=minecraft:marker,tag=mdr_atelier",
)


def _pose(depot):
    ident = depot.ajouter(photo._calcul("Double de 17"), source="photo", demande_par="1",
                          recompense="minecraft:diamond", recompense_nombre=3)
    depot.marquer_pose(ident, Horloge()())
    return ident


def test_deplacement_dans_l_ordre(depot):
    terrain = Terrain(atelier=ANCIEN)
    reponse = _texte(_atelier(depot, terrain), "/atelier")
    assert reponse == "Atelier déplacé en 92 64 192, à 11 blocs de l'enfant."
    assert terrain.atelier == [92, 64, 192]
    modifs = [c for c in terrain.recues if c.startswith(MODIFIANTES)]
    assert modifs[:4] == [f"setblock {p} minecraft:air" for p in
                          ("40 64 40", "39 64 40", "41 64 40", "42 64 38")]
    assert modifs[4] == "fill 38 63 38 42 63 42 minecraft:dirt"
    assert modifs[5].startswith("kill") and "mdr_atelier" in modifs[5]
    assert modifs[6] == "forceload remove 38 38 42 42"
    assert modifs[7] == "data remove storage mine_de_rien:atelier pos"
    assert modifs[8] == "forceload add 90 190 94 194"
    assert modifs[-1] == "data modify storage mine_de_rien:atelier pos set value [92, 64, 192]"
    assert terrain.dits == [
        ("Steve_42", "Ton atelier vient d'apparaître, à 11 blocs vers l'ouest.")
    ]


def test_deplacement_refuse_avec_un_coffre_actif(depot):
    ident = _pose(depot)
    terrain = Terrain(atelier=ANCIEN)
    reponse = _texte(_atelier(depot, terrain), "/atelier")
    assert f"coffre n°{ident} est en cours" in reponse
    assert not any(c.startswith(MODIFIANTES) for c in terrain.recues)
    assert terrain.atelier == ANCIEN


def test_deplacement_refuse_avec_un_coffre_plein(depot):
    class AvecCoffre(Terrain):
        def bloc(self, x, y, z):
            return "minecraft:chest" if [x, y, z] == ANCIEN else super().bloc(x, y, z)

    terrain = AvecCoffre(atelier=ANCIEN, coffre_plein=True)
    assert "contient encore des objets" in _texte(_atelier(depot, terrain), "/atelier")
    assert not any(c.startswith(MODIFIANTES) for c in terrain.recues)


def test_ancien_atelier_intact_si_rien_ne_convient(depot):
    class Bossele(Terrain):
        def sol(self, x, z):
            return Y + (x + z) % 2

    terrain = Bossele(atelier=ANCIEN)
    assert _texte(_atelier(depot, terrain), "/atelier") == AUCUN_TERRAIN
    assert not any(c.startswith(MODIFIANTES) for c in terrain.recues)
    assert terrain.atelier == ANCIEN


def test_atelier_attend_la_fin_du_tour_de_surveillance(depot):
    """Pas de coffre posé à l'ancien endroit pendant le déplacement."""
    terrain = Terrain(atelier=ANCIEN)
    c = _atelier(depot, terrain)

    async def scenario():
        await c.coffre.verrou.acquire()
        tache = asyncio.create_task(c.traiter(MessageParent(111, "/atelier")))
        await asyncio.sleep(0)
        assert not tache.done() and terrain.recues == []
        c.coffre.verrou.release()
        return await tache

    assert asyncio.run(scenario()).startswith("Atelier déplacé")
