import asyncio
import json
import re
from datetime import UTC, datetime, timedelta

import pytest

from mine_de_rien.exercises.base import Exercice
from mine_de_rien.games.atelier import Atelier
from mine_de_rien.games.coffre_du_soir import (
    AtelierAbsent,
    CoffreDuSoir,
    ErreurCoffre,
    commande_cle,
    commandes_pose,
    commandes_rangement,
    commandes_retrait,
    formater_duree,
    predicat_cle,
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


def test_cle_avec_mode_d_emploi():
    commande = commande_cle("Steve")
    assert commande.startswith('give Steve minecraft:trial_key[minecraft:item_name="Clé du coffre",')
    assert commande.endswith(",minecraft:custom_data={mdr_cle:1b}] 1")
    lore = json.loads(re.search(r"minecraft:lore=(\[.*?\]),", commande).group(1))
    assert len(lore) == 4
    assert lore[0]["text"] == "1. Pose-moi à gauche sur l'enclume"
    assert all(ligne["italic"] is False for ligne in lore)
    assert predicat_cle() == "minecraft:trial_key[minecraft:custom_data={mdr_cle:1b}]"
    with pytest.raises(ValueError):
        commande_cle("@a")


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
        self.probleme = None
        self.actualisations = 0
        self.actions: list[str] = []

    async def joueur_en_ligne(self):
        return self.en_ligne

    async def actualiser_atelier(self):
        self.actualisations += 1

    def verifier_atelier(self):
        if self.probleme:
            raise AtelierAbsent(self.probleme)

    async def poser(self, s):
        self.verifier_atelier()
        self.actions.append(f"poser {s.id}")
        self.present, self.vide, self.preuve = True, False, False

    async def annoncer(self, s):
        self.actions.append(f"annoncer {s.id}")

    async def equiper(self):
        self.actions.append("équiper")

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

    # Il se connecte : clé et niveaux une seule fois, puis rappel.
    coffre.en_ligne = True
    horloge.t += timedelta(minutes=1)
    _tour(orch)
    _tour(orch)
    assert coffre.actions[1:] == ["équiper", "rappel 1"]

    # Il ouvre : coffre rangé, parents prévenus avec la durée, pas encore de suivant.
    coffre.preuve = True
    horloge.t += timedelta(minutes=4)
    _tour(orch)
    assert coffre.actions[-1] == "ranger 1"
    assert notifs.messages[-1] == "Coffre n°1 ouvert en 5 min, sans indice (12 ÷ 12 = ? → 1)."
    assert depot.lire(1).statut == "ouvert"

    # Coffre pas vidé : on attend, et on le lui demande une fois.
    horloge.t += timedelta(seconds=11)
    _tour(orch)
    _tour(orch)
    assert coffre.actions[-1] == "vider ?"
    assert coffre.actions.count("vider ?") == 1
    assert depot.actif() is None

    # Vidé : le second coffre apparaît, clé et niveaux donnés avant l'annonce.
    coffre.vide = True
    _tour(orch)
    assert coffre.actions[-3:] == ["poser 2", "équiper", "annoncer 2"]
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


PAS_CONSTRUIT = "l'atelier n'est pas construit en 0 -60 8 dans ce monde."
AILLEURS = "l'atelier de ce monde est en 120 64 -35, mais l'agent attend 0 -60 8."


class MondeSansAtelier:
    """Serveur neuf : `mine-de-rien atelier` n'a jamais été lancé, rien n'est enregistré."""

    def __init__(self, pos=None):
        self.pos = pos
        self.recues: list[str] = []

    async def commande(self, c):
        self.recues.append(c)
        if c == "data get storage mine_de_rien:atelier pos":
            if self.pos is None:
                return "Found no elements matching pos"
            return f"Storage mine_de_rien:atelier has the following contents: {self.pos}"
        return ""


def test_pose_refusee_sans_atelier_dans_le_monde():
    monde = MondeSansAtelier()
    coffre = CoffreDuSoir(monde, "Steve")
    asyncio.run(coffre.actualiser_atelier())
    with pytest.raises(AtelierAbsent, match="pas construit"):
        asyncio.run(coffre.poser(_stocke()))
    assert monde.recues == ["data get storage mine_de_rien:atelier pos"]


def test_la_position_suit_le_monde_sans_recreer_le_coffre():
    """Atelier construit ou déplacé pendant que l'agent tourne : pas de redémarrage."""
    monde = MondeSansAtelier([120, 64, -35])
    coffre = CoffreDuSoir(monde, "Steve")
    asyncio.run(coffre.actualiser_atelier())
    assert coffre.verifier_atelier() == Atelier(120, 64, -35)
    monde.pos = [-27, 68, -16]
    asyncio.run(coffre.actualiser_atelier())
    assert coffre.verifier_atelier() == Atelier(-27, 68, -16)
    assert any("-27 68 -16" in c for c in commandes_pose(coffre.atelier, _stocke()))
    monde.pos = None
    asyncio.run(coffre.actualiser_atelier())
    with pytest.raises(AtelierAbsent):
        coffre.verifier_atelier()


def test_sans_atelier_parents_prevenus_une_fois_puis_pose_quand_il_existe(monde):
    depot, coffre, notifs, _horloge, orch = monde
    coffre.probleme = PAS_CONSTRUIT
    _tour(orch)
    _tour(orch)
    assert coffre.actions == [] and depot.actif() is None
    assert notifs.messages == [f"Aucun coffre ne peut être posé : {PAS_CONSTRUIT}"]
    coffre.probleme = None
    _tour(orch)
    assert coffre.actions == ["poser 1"] and depot.actif().id == 1


def test_coffre_actif_d_un_autre_monde_jamais_repose_dans_la_roche(monde):
    """Le cas de l'essai du 2026-11-09 : nouveau serveur, coffre actif, atelier jamais construit."""
    _depot, coffre, notifs, _horloge, orch = monde
    _tour(orch)
    coffre.present, coffre.probleme = False, PAS_CONSTRUIT
    _tour(orch)
    _tour(orch)
    assert coffre.actions == ["poser 1"]
    assert notifs.messages[-1].startswith("Aucun coffre ne peut être posé")
    assert sum(m.startswith("Aucun coffre") for m in notifs.messages) == 1


def test_coffre_plein_d_un_autre_monde_ne_fait_pas_attendre(monde):
    """L'essai du 2026-11-09 : un coffre emmuré, plein, ne doit pas retarder le message."""
    _depot, coffre, notifs, _horloge, orch = monde
    coffre.en_ligne, coffre.vide, coffre.probleme = True, False, PAS_CONSTRUIT
    _tour(orch)
    assert coffre.actions == []  # pas de « vide le coffre » pour un coffre inaccessible
    assert notifs.messages == [f"Aucun coffre ne peut être posé : {PAS_CONSTRUIT}"]


def test_coffre_actif_present_mais_atelier_absent_parents_prevenus_sans_rappel(monde):
    _depot, coffre, notifs, _horloge, orch = monde
    _tour(orch)
    coffre.en_ligne, coffre.probleme = True, PAS_CONSTRUIT
    _tour(orch)
    assert "rappel 1" not in coffre.actions
    assert notifs.messages[-1] == f"Aucun coffre ne peut être posé : {PAS_CONSTRUIT}"


def test_chaque_tour_relit_l_atelier(monde):
    _depot, coffre, _notifs, _horloge, orch = monde
    _tour(orch)
    _tour(orch)
    assert coffre.actualisations == 2


def test_un_nouveau_probleme_d_atelier_est_signale_meme_apres_le_premier(monde):
    """Le premier message ne doit pas rendre l'agent muet sur un problème différent."""
    _depot, coffre, notifs, _horloge, orch = monde
    coffre.probleme = PAS_CONSTRUIT
    _tour(orch)
    coffre.probleme = AILLEURS
    _tour(orch)
    _tour(orch)
    assert notifs.messages == [
        f"Aucun coffre ne peut être posé : {PAS_CONSTRUIT}",
        f"Aucun coffre ne peut être posé : {AILLEURS}",
    ]
