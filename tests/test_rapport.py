import asyncio
from datetime import UTC, datetime, timedelta

import pytest

from mine_de_rien.adapters.channel.base import MessageParent
from mine_de_rien.config import Coffre
from mine_de_rien.exercises.base import Exercice
from mine_de_rien.games.coffre_du_soir import CoffreDuSoir
from mine_de_rien.orchestrateur import CommandesParents, Orchestrateur
from mine_de_rien.rapport import a_retravailler, rapport
from mine_de_rien.storage.sqlite import Depot

MAINTENANT = datetime(2026, 11, 14, 18, 0, tzinfo=UTC)


@pytest.fixture
def depot(tmp_path):
    d = Depot.ouvrir(tmp_path / "mdr.sqlite")
    yield d
    d.fermer()


def _ajouter(depot, competence, enonce, pose_il_y_a, ouvert_apres=None, annule=False):
    ident = depot.ajouter(
        Exercice(competence, enonce, "1"), source="competence", demande_par="111",
        recompense="minecraft:diamond", recompense_nombre=3,
        quand=MAINTENANT - pose_il_y_a - timedelta(minutes=1),
    )
    if pose_il_y_a is None:
        return ident
    pose = MAINTENANT - pose_il_y_a
    depot.marquer_pose(ident, pose)
    if ouvert_apres is not None:
        depot.marquer_ouvert(ident, pose + ouvert_apres)
    if annule:
        depot.annuler(ident)
    return ident


def test_rapport_classe_les_exercices_a_retravailler(depot):
    # Un seul coffre posé à la fois : on les crée dans l'ordre de leur vie.
    _ajouter(depot, "maths/division-posee", "ancien", timedelta(days=9), timedelta(minutes=90))
    _ajouter(depot, "maths/division-posee", "72 ÷ 6 = ?", timedelta(days=4), timedelta(minutes=10))
    lent = _ajouter(
        depot, "maths/division-posee", "84 ÷ 7 = ?", timedelta(days=3), timedelta(minutes=45)
    )
    _ajouter(depot, "maths/division-posee", "96 ÷ 3 = ?", timedelta(days=2), timedelta(minutes=4))
    _ajouter(depot, "maths/addition-posee", "annulé", timedelta(days=2), annule=True)
    ferme = _ajouter(depot, "maths/addition-posee", "3,5 + 1,25 = ?", timedelta(hours=50))
    depot.noter("coffre_disparu", ferme, MAINTENANT - timedelta(days=1))

    texte = rapport(depot, MAINTENANT)
    assert "maths/division-posee : 3 posé(s), 3 ouvert(s), durée médiane 10 min" in texte
    assert "maths/addition-posee : 1 posé(s), 0 ouvert(s)" in texte
    lignes = texte.split("À retravailler :\n")[1].split("\n\n")[0].splitlines()
    assert lignes == [
        f"• n°{lent} 84 ÷ 7 = ? : ouvert en 45 min",
        f"• n°{ferme} 3,5 + 1,25 = ? : toujours fermé après 2 jours",
    ]
    assert "ancien" not in texte and "annulé" not in texte
    assert "Coffres disparus puis reposés : 1" in texte


def test_pas_encore_a_retravailler_avant_48_heures(depot):
    ident = _ajouter(depot, "maths/addition-posee", "1 + 1 = ?", timedelta(hours=47))
    assert a_retravailler(depot.lire(ident), MAINTENANT) is None
    assert "À retravailler" not in rapport(depot, MAINTENANT)


def test_rapport_vide(depot):
    assert "Aucun coffre posé." in rapport(depot, MAINTENANT)


PSEUDO = "Zorglub42"


class FauxServeur:
    """Le monde côté RCON : l'enfant est connecté et finit par ouvrir le coffre."""

    def __init__(self):
        self.ouvert = False
        self.coffre_plein = False

    async def commande(self, c: str) -> str:
        if c == "list":
            return f"There are 1 of a max of 20 players online: {PSEUDO}"
        if c == "data get storage mine_de_rien:atelier pos":
            return "Storage mine_de_rien:atelier has the following contents: [0, -60, 8]"
        if c.startswith(f"clear {PSEUDO}") and c.endswith(" 0"):
            return f"Found 1 matching item(s) on player {PSEUDO}" if self.ouvert else (
                f"No items were found on player {PSEUDO}"
            )
        if "Items[0]" in c:
            return "Test failed"
        if c.startswith("execute if"):
            return "Test passed"
        return ""

    async def dire(self, joueur, texte, **style):
        pass


def test_aucun_message_aux_parents_ne_contient_le_pseudo(depot):
    serveur = FauxServeur()
    coffre = CoffreDuSoir(serveur, PSEUDO)
    envoyes: list[str] = []

    class Canal:
        async def notifier(self, m):
            envoyes.append(m)

    horloge = [MAINTENANT]
    orch = Orchestrateur(depot, coffre, Canal(), lambda: horloge[0])
    commandes = CommandesParents(depot, Coffre(), coffre, [111], lambda: horloge[0])

    async def scenario():
        for texte in ("/coffre 348 ÷ 12", "/coffre division 2", "/file"):
            envoyes.append(await commandes.traiter(MessageParent(111, texte)))
        await orch.tour()                       # posé
        horloge[0] += timedelta(minutes=3)
        serveur.ouvert = True
        await orch.tour()                       # ouvert
        envoyes.append(await commandes.traiter(MessageParent(111, "/file")))
        envoyes.append(await commandes.traiter(MessageParent(111, "/rapport")))
        envoyes.append(await commandes.traiter(MessageParent(111, "/annuler 3")))

    asyncio.run(scenario())
    assert any("Coffre n°1 ouvert en 3 min" in m for m in envoyes)
    assert all(PSEUDO not in m for m in envoyes), envoyes


def _indices(depot, ident, n):
    for _ in range(n):
        depot.noter("indice", ident, MAINTENANT - timedelta(days=1))


@pytest.mark.parametrize("n, texte", [(0, "sans indice"), (1, "1 indice"), (3, "3 indices")])
def test_notification_d_ouverture_compte_les_indices(depot, n, texte):
    serveur = FauxServeur()
    coffre = CoffreDuSoir(serveur, PSEUDO)
    envoyes: list[str] = []

    class Notif:
        async def notifier(self, m):
            envoyes.append(m)

    horloge = [MAINTENANT]
    orch = Orchestrateur(depot, coffre, Notif(), lambda: horloge[0])
    depot.ajouter(Exercice("maths/doubles-moities", "Double de 17 = ?", "34"), source="photo",
                  demande_par="111", recompense="minecraft:diamond", recompense_nombre=3)

    async def scenario():
        await orch.tour()
        _indices(depot, 1, n)
        horloge[0] += timedelta(minutes=12)
        serveur.ouvert = True
        await orch.tour()

    asyncio.run(scenario())
    assert envoyes[-1] == f"Coffre n°1 ouvert en 12 min, {texte} (Double de 17 = ? → 34)."


def test_rapport_compte_les_indices_et_retravaille_a_trois(depot):
    deux = _ajouter(depot, "maths/doubles-moities", "Double de 17 = ?", timedelta(days=4),
                    timedelta(minutes=5))
    _indices(depot, deux, 2)
    trois = _ajouter(depot, "maths/doubles-moities", "Moitié de 75 = ?", timedelta(days=3),
                     timedelta(minutes=5))
    _indices(depot, trois, 3)
    lent = _ajouter(depot, "anglais/vocabulaire", "August en français ?", timedelta(days=2),
                    timedelta(minutes=40))
    _indices(depot, lent, 4)
    annule = _ajouter(depot, "anglais/vocabulaire", "annulé", timedelta(days=1), annule=True)
    _indices(depot, annule, 5)

    texte = rapport(depot, MAINTENANT)
    assert "maths/doubles-moities : 2 posé(s), 2 ouvert(s), durée médiane 5 min, 5 indices" in texte
    assert "anglais/vocabulaire : 1 posé(s), 1 ouvert(s), durée médiane 40 min, 4 indices" in texte
    lignes = texte.split("À retravailler :\n")[1].split("\n\n")[0].splitlines()
    assert lignes == [
        f"• n°{trois} Moitié de 75 = ? : ouvert avec 3 indices",
        f"• n°{lent} August en français ? : ouvert en 40 min avec 4 indices",
    ]
