import asyncio
import json
import re
from datetime import UTC, datetime, timedelta

import pytest

from mine_de_rien.exercises.base import Exercice
from mine_de_rien.games.atelier import Atelier
from mine_de_rien.games.coffre_du_soir import (
    ErreurCoffre,
    commandes_pose,
    commandes_rangement,
    commandes_retrait,
    formater_duree,
)
from mine_de_rien.minecraft.texte import lignes_panneau
from mine_de_rien.orchestrateur import Orchestrateur
from mine_de_rien.storage.sqlite import Depot, ExerciceStocke

A = Atelier(0, -60, 8)
T0 = datetime(2026, 11, 7, 18, 0, tzinfo=UTC)


def _stocke(enonce="348 ÷ 12 = ?", reponse="29", ident=12) -> ExerciceStocke:
    return ExerciceStocke(
        id=ident,
        exercice=Exercice("maths/division-posee", enonce, reponse),
        jeu="coffre-du-soir",
        source="expression",
        statut="en_file",
        demande_par="cli",
        recompense="minecraft:diamond",
        recompense_nombre=3,
        cree_le=T0,
        pose_le=None,
        ouvert_le=None,
    )


def test_commandes_pour_un_exercice_fixe():
    assert commandes_pose(A, _stocke()) == [
        ("execute unless block 0 -60 8 minecraft:chest run setblock 0 -60 8 "
        "minecraft:chest[facing=south]"),
        'data modify block 0 -60 8 lock set value {components:{"minecraft:custom_name":"29"}}',
        "item replace block 0 -60 8 container.13 with minecraft:diamond 3",
        ('item replace block 0 -60 8 container.11 with minecraft:paper[minecraft:custom_name='
        '"Preuve n°12",minecraft:custom_data={mdr_coffre:12}]'),
        ('setblock -1 -60 8 minecraft:oak_sign[rotation=0]{front_text:{messages:'
        '["348 ÷ 12 = ?","","",""]},is_waxed:1b}'),
        "setblock 1 -60 8 minecraft:anvil[facing=east]",
    ]


def test_le_verrou_est_pose_avant_la_recompense():
    commandes = commandes_pose(A, _stocke())
    verrou = next(i for i, c in enumerate(commandes) if " lock " in c)
    recompense = next(i for i, c in enumerate(commandes) if "item replace" in c)
    assert verrou < recompense


def test_decoupage_du_panneau():
    assert lignes_panneau("Le fermier a 348 œufs en boîtes de 12. Combien ?") == [
        "Le fermier a",
        "348 œufs en",
        "boîtes de 12.",
        "Combien ?",
    ]
    assert lignes_panneau("(12 + 8) × 3 = ?") == ["(12 + 8) × 3 =", "?", "", ""]
    assert lignes_panneau("un mot-beaucoup-trop-long") is None
    assert lignes_panneau(" ".join(["abcdefgh"] * 5)) is None


def test_enonce_piege_echappe_sur_le_panneau():
    piege = 'l\'élève "a" \\ ]}'
    (panneau,) = [c for c in commandes_pose(A, _stocke(enonce=piege)) if "oak_sign" in c]
    messages = re.search(r"messages:(\[.*\])\}", panneau).group(1)
    assert json.loads(messages) == ["l'élève \"a\" \\", "]}", "", ""]


def test_reponse_decimale_dans_le_verrou():
    commandes = commandes_pose(A, _stocke(enonce="3,5 + 1,25 = ?", reponse="4,75"))
    assert '"minecraft:custom_name":"4,75"' in commandes[1]


def test_enonce_trop_long_refuse():
    with pytest.raises(ErreurCoffre):
        commandes_pose(A, _stocke(enonce=" ".join(["abcdefgh"] * 5)))


def test_rangement_et_retrait():
    assert commandes_rangement(A, "Steve", 12) == [
        "clear Steve minecraft:paper[minecraft:custom_data={mdr_coffre:12}]",
        "setblock -1 -60 8 minecraft:air",
        "data remove block 0 -60 8 lock",
    ]
    assert "data modify block 0 -60 8 Items set value []" in commandes_retrait(A)
    with pytest.raises(ValueError):
        commandes_rangement(A, "@a", 12)


def test_formater_duree():
    assert formater_duree(timedelta(seconds=30)) == "moins d'une minute"
    assert formater_duree(timedelta(minutes=4, seconds=50)) == "4 min"
    assert formater_duree(timedelta(hours=2, minutes=5)) == "2 h 05"
    assert formater_duree(timedelta(days=3)) == "3 jours"


class FauxCoffre:
    """Le monde vu par l'orchestrateur : joueur en ligne, coffre présent, preuve sur soi."""

    def __init__(self):
        self.en_ligne = False
        self.present = True
        self.vide = True
        self.preuve = False
        self.actions: list[str] = []

    async def joueur_en_ligne(self):
        return self.en_ligne

    async def poser(self, s):
        self.actions.append(f"poser {s.id}")
        self.present, self.vide, self.preuve = True, False, False

    async def annoncer(self, s):
        self.actions.append(f"annoncer {s.id}")

    async def donner_niveaux(self):
        self.actions.append("niveaux")

    async def rappeler(self, s):
        self.actions.append(f"rappel {s.id}")

    async def demander_de_vider(self):
        self.actions.append("vider ?")

    async def preuve_trouvee(self, s):
        return self.preuve

    async def coffre_present(self):
        return self.present

    async def coffre_vide(self):
        return self.vide

    async def ranger(self, s):
        self.actions.append(f"ranger {s.id}")


class Notifs:
    def __init__(self):
        self.messages: list[str] = []

    async def notifier(self, message):
        self.messages.append(message)


class Horloge:
    def __init__(self):
        self.t = T0

    def __call__(self):
        return self.t


@pytest.fixture
def monde(tmp_path):
    depot = Depot.ouvrir(tmp_path / "mdr.sqlite")
    coffre, notifs, horloge = FauxCoffre(), Notifs(), Horloge()
    orch = Orchestrateur(depot, coffre, notifs, horloge)
    for n in (1, 2):
        depot.ajouter(
            Exercice("maths/division-posee", f"{n * 12} ÷ 12 = ?", str(n)),
            source="expression", demande_par="cli", recompense="minecraft:diamond",
            recompense_nombre=3, quand=T0,
        )
    yield depot, coffre, notifs, horloge, orch
    depot.fermer()


def _tour(orch):
    asyncio.run(orch.tour())


def test_scenario_complet(monde):
    depot, coffre, notifs, horloge, orch = monde

    # Enfant absent : le coffre est posé quand même, sans annonce ni niveaux.
    _tour(orch)
    assert coffre.actions == ["poser 1"]
    assert notifs.messages == ["Coffre n°1 posé : 12 ÷ 12 = ? (réponse 1)."]
    assert depot.actif().id == 1

    # Il se connecte : rappel et niveaux offerts une seule fois.
    coffre.en_ligne = True
    horloge.t += timedelta(minutes=1)
    _tour(orch)
    _tour(orch)
    assert coffre.actions[1:] == ["niveaux", "rappel 1"]

    # Il ouvre : coffre rangé, parents prévenus avec la durée, pas encore de suivant.
    coffre.preuve = True
    horloge.t += timedelta(minutes=4)
    _tour(orch)
    assert coffre.actions[-1] == "ranger 1"
    assert notifs.messages[-1] == "Coffre n°1 ouvert en 5 min (12 ÷ 12 = ? → 1)."
    assert depot.lire(1).statut == "ouvert"

    # Coffre pas vidé : on attend, et on le lui demande une fois.
    horloge.t += timedelta(seconds=11)
    _tour(orch)
    _tour(orch)
    assert coffre.actions[-1] == "vider ?"
    assert coffre.actions.count("vider ?") == 1
    assert depot.actif() is None

    # Vidé : le second coffre apparaît, annoncé, avec ses niveaux.
    coffre.vide = True
    _tour(orch)
    assert coffre.actions[-3:] == ["poser 2", "annoncer 2", "niveaux"]
    assert depot.actif().id == 2
    assert depot.actif().niveaux_donnes


def test_suivant_attend_dix_secondes(monde):
    depot, coffre, _notifs, horloge, orch = monde
    coffre.en_ligne = True
    _tour(orch)
    coffre.preuve = True
    _tour(orch)
    coffre.vide = True
    horloge.t += timedelta(seconds=5)
    _tour(orch)
    assert depot.actif() is None
    horloge.t += timedelta(seconds=6)
    _tour(orch)
    assert depot.actif().id == 2


def test_coffre_disparu_repose_et_note(monde):
    depot, coffre, notifs, _horloge, orch = monde
    _tour(orch)
    coffre.present = False
    _tour(orch)
    assert coffre.actions[-1] == "poser 1"
    assert notifs.messages[-1] == "Coffre n°1 disparu sans être ouvert : il a été reposé."
    (evenement,) = depot.evenements_entre(T0 - timedelta(days=1), T0 + timedelta(days=1))
    assert (evenement.exercice_id, evenement.genre) == (1, "coffre_disparu")
    assert depot.actif().id == 1


def test_preuve_ignoree_hors_ligne(monde):
    depot, coffre, _notifs, _horloge, orch = monde
    _tour(orch)
    coffre.preuve = True  # impossible en vrai, mais l'orchestrateur ne doit pas interroger
    _tour(orch)
    assert depot.actif().id == 1
