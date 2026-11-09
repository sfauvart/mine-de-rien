"""Pixel dans le chat, avec un faux jeu, un faux LLM et une vraie base jetable."""

from __future__ import annotations

import asyncio
import json
from datetime import datetime, timedelta

import pytest

from mine_de_rien import photo
from mine_de_rien.adapters.game.base import MessageChat
from mine_de_rien.adapters.llm.base import ErreurLlm
from mine_de_rien.exercises.indices import indices
from mine_de_rien.games.atelier import Atelier
from mine_de_rien.games.coffre_du_soir import CoffreDuSoir
from mine_de_rien.games.pixel import (
    BAVARDAGE,
    EVENEMENT_INDICE,
    INDICE,
    TELEPORTER,
    Pixel,
    adresse,
    classer_par_mots_cles,
    commande_teleportation,
)
from mine_de_rien.storage.sqlite import Depot

PSEUDO = "Steve_42"
ATELIER = Atelier(10, 64, -5)
DEBUT = datetime(2026, 11, 8, 18, 0)


class FauxJeu:
    def __init__(
        self,
        reponse_tp: str = "Teleported Steve_42 to 10.5, 64.0, -2.5",
        atelier: tuple[int, int, int] | None = (10, 64, -5),
    ) -> None:
        self.commandes: list[str] = []
        self.dits: list[str] = []
        self.reponse_tp = reponse_tp
        self.atelier = atelier

    async def commande(self, commande: str) -> str:
        if commande == "data get storage mine_de_rien:atelier pos":
            if self.atelier is None:
                return "Found no elements matching pos"
            return f"Storage mine_de_rien:atelier has the following contents: {list(self.atelier)}"
        self.commandes.append(commande)
        return self.reponse_tp if " tp " in commande else ""

    async def dire(self, joueur: str, texte: str, **style) -> None:
        assert joueur == PSEUDO
        self.dits.append(texte)


class FauxLlm:
    """Répond selon le prompt système ; garde chaque appel pour vérifier ce qui part."""

    def __init__(self, intention: str = INDICE, reformulation: str = "Courage !",
                 bavardage: str = "Je suis Pixel !", distant: bool = False,
                 erreur: bool = False) -> None:
        self.intention = intention
        self.reformulation = reformulation
        self.bavardage = bavardage
        self.distant = distant
        self.erreur = erreur
        self.appels: list[tuple[str, str]] = []

    async def completer(self, systeme, utilisateur, image=None, en_json=False):
        self.appels.append((systeme, utilisateur))
        if self.erreur:
            raise ErreurLlm("injoignable")
        if "catégorie" in systeme:
            return json.dumps({"intention": self.intention})
        if "reformules" in systeme:
            return self.reformulation
        return self.bavardage


@pytest.fixture
def depot(tmp_path):
    d = Depot.ouvrir(tmp_path / "mdr.sqlite")
    yield d
    d.fermer()


def _poser(depot: Depot, texte: str = "Double de 17") -> int:
    ident = depot.ajouter(photo._calcul(texte), source="photo", demande_par="1",
                          recompense="minecraft:diamond", recompense_nombre=3)
    depot.marquer_pose(ident, DEBUT)
    return ident


def _pixel(depot, llm=None, jeu=None, horloge=None):
    jeu = jeu or FauxJeu()
    temps = horloge or [DEBUT + timedelta(minutes=1)]
    return Pixel(jeu, depot, PSEUDO, "Pixel", llm, lambda: temps[0]), jeu, temps


def _dit(pixel, texte: str, joueur: str = PSEUDO) -> None:
    async def scenario():
        await pixel.recevoir(MessageChat(DEBUT, joueur, texte))
        if pixel._tache is not None:
            await pixel._tache

    asyncio.run(scenario())


@pytest.mark.parametrize(
    "message, reste",
    [("Pixel, aide-moi", "aide-moi"), ("pixel: tp", "tp"), ("PIXEL ramène-moi", "ramène-moi"),
     ("  Pïxel,  aide ", "aide"), ("Pixel", ""), ("pixel,", "")],
)
def test_adresse(message, reste):
    assert adresse(message, "Pixel") == reste


@pytest.mark.parametrize("message", ["Pixels, aide", "bonjour Pixel", "aide-moi", "Pix, aide"])
def test_pas_adresse(message):
    assert adresse(message, "Pixel") is None


@pytest.mark.parametrize(
    "message, intention",
    [("ramène-moi au coffre", TELEPORTER), ("téléporte moi", TELEPORTER), ("tp", TELEPORTER),
     ("aide-moi", INDICE), ("je suis bloqué", INDICE), ("je sais pas", INDICE),
     ("donne un indice", INDICE), ("c'est combien ?", INDICE), ("ça veut dire quoi", INDICE),
     ("je comprends pas", INDICE), ("tu aimes les creepers ?", BAVARDAGE)],
)
def test_mots_cles(message, intention):
    assert classer_par_mots_cles(message) == intention


def test_commande_de_teleportation():
    assert commande_teleportation(ATELIER, PSEUDO) == (
        "execute in minecraft:overworld run tp Steve_42 10 64 -3 180 0"
    )


def test_message_sans_le_nom_ou_d_un_autre_joueur_ignore(depot):
    _poser(depot)
    llm = FauxLlm()
    pixel, jeu, _ = _pixel(depot, llm)
    _dit(pixel, "aide-moi")
    _dit(pixel, "Pixel, aide-moi", joueur="Copain")
    assert llm.appels == [] and jeu.dits == [] and jeu.commandes == []


def test_teleportation_avec_coffre_actif_puis_patience(depot):
    ident = _poser(depot)
    pixel, jeu, temps = _pixel(depot, FauxLlm(intention=TELEPORTER))
    _dit(pixel, "Pixel, ramène-moi au coffre")
    assert jeu.commandes == [commande_teleportation(ATELIER, PSEUDO)]
    assert jeu.dits == ["<Pixel> …", "<Pixel> Te voilà devant le coffre !"]
    assert depot.compter("teleportation", ident) == 1
    temps[0] += timedelta(minutes=4)
    _dit(pixel, "Pixel, ramène-moi au coffre")
    assert len(jeu.commandes) == 1 and "patience" in jeu.dits[-1]
    temps[0] += timedelta(minutes=2)
    _dit(pixel, "Pixel, ramène-moi au coffre")
    assert len(jeu.commandes) == 2


def test_teleportation_sans_coffre_refusee(depot):
    pixel, jeu, _ = _pixel(depot, FauxLlm(intention=TELEPORTER))
    _dit(pixel, "Pixel, tp")
    assert jeu.commandes == [] and "Pas de coffre" in jeu.dits[-1]


def test_teleportation_vers_la_position_du_monde(depot):
    _poser(depot)
    pixel, jeu, _ = _pixel(depot, None, FauxJeu(atelier=(0, -60, 8)))
    _dit(pixel, "Pixel, ramène-moi au coffre")
    assert jeu.commandes == [commande_teleportation(Atelier(0, -60, 8), PSEUDO)]


def test_teleportation_refusee_sans_atelier_dans_le_monde(depot):
    """Nouveau monde, atelier jamais construit : le bloc d'arrivée peut être de la roche."""
    ident = _poser(depot)
    pixel, jeu, _ = _pixel(depot, None, FauxJeu(atelier=None))
    _dit(pixel, "Pixel, ramène-moi au coffre")
    assert jeu.commandes == []
    assert "pas encore construit" in jeu.dits[-1]
    assert depot.compter("teleportation", ident) == 0


def test_teleportation_enfant_deconnecte(depot):
    ident = _poser(depot)
    pixel, jeu, _ = _pixel(depot, None, FauxJeu(reponse_tp="No entity was found"))
    _dit(pixel, "Pixel, tp")
    assert "réessaie" in jeu.dits[-1] and depot.compter("teleportation", ident) == 0


def test_indices_dans_l_ordre_puis_le_dernier_repete(depot):
    ident = _poser(depot)
    paliers = indices(photo._calcul("Double de 17"))
    pixel, jeu, _ = _pixel(depot)  # sans LLM : indices bruts
    for _ in range(len(paliers) + 1):
        _dit(pixel, "Pixel, aide-moi")
    dits = [d for d in jeu.dits if d != "<Pixel> …"]
    assert dits == [f"<Pixel> {p}" for p in paliers] + [f"<Pixel> {paliers[-1]}"]
    assert depot.compter(EVENEMENT_INDICE, ident) == len(paliers) + 1


def test_reformulation_acceptee(depot):
    _poser(depot)
    llm = FauxLlm(reformulation="Astuce : ajoute 17 à lui-même, tu vas y arriver !")
    pixel, jeu, _ = _pixel(depot, llm)
    _dit(pixel, "Pixel, aide-moi")
    assert jeu.dits[-1] == "<Pixel> Astuce : ajoute 17 à lui-même, tu vas y arriver !"
    _, utilisateur = llm.appels[-1]
    assert "Double de 17 = ?" in utilisateur and "34" not in utilisateur


@pytest.mark.parametrize(
    "reformulation",
    ["Ça fait 34 !", "C'est trente-quatre.", "Le double de 7 = 15, ajoute 10.", "", "x" * 300],
)
def test_reformulation_dangereuse_remplacee_par_l_indice_brut(depot, reformulation):
    _poser(depot)
    pixel, jeu, _ = _pixel(depot, FauxLlm(reformulation=reformulation))
    _dit(pixel, "Pixel, aide-moi")
    assert jeu.dits[-1] == f"<Pixel> {indices(photo._calcul('Double de 17'))[0]}"


def test_bavardage_sans_chiffre(depot):
    pixel, jeu, _ = _pixel(depot, FauxLlm(intention=BAVARDAGE, bavardage="J'adore les moutons !"))
    _dit(pixel, "Pixel, tu aimes quoi ?")
    assert jeu.dits[-1] == "<Pixel> J'adore les moutons !"


def test_bavardage_avec_chiffre_remplace(depot):
    llm = FauxLlm(intention=BAVARDAGE, bavardage="348 divisé par 12, ça fait 29.")
    pixel, jeu, _ = _pixel(depot, llm)
    _dit(pixel, "Pixel, et 348 divisé par 12 ?")
    assert jeu.dits[-1].startswith("<Pixel> Je ne fais pas les calculs")
    # « combien » est un mot-clé d'indice : sans coffre, rien n'est calculé non plus.
    _dit(pixel, "Pixel, combien font 348 divisé par 12 ?")
    assert "Pas de coffre" in jeu.dits[-1]


def test_llm_distant_jamais_appele(depot):
    _poser(depot)
    llm = FauxLlm(distant=True)
    pixel, jeu, _ = _pixel(depot, llm)
    _dit(pixel, "Pixel, aide-moi")
    _dit(pixel, "Pixel, raconte une blague")
    assert llm.appels == []
    assert jeu.dits[1] == f"<Pixel> {indices(photo._calcul('Double de 17'))[0]}"
    assert "aide-moi" in jeu.dits[-1]


def test_llm_en_erreur_les_mots_cles_prennent_le_relais(depot):
    _poser(depot)
    pixel, jeu, _ = _pixel(depot, FauxLlm(erreur=True))
    _dit(pixel, "Pixel, ramène-moi")
    assert jeu.dits[-1] == "<Pixel> Te voilà devant le coffre !"
    _dit(pixel, "Pixel, aide")
    assert jeu.dits[-1] == f"<Pixel> {indices(photo._calcul('Double de 17'))[0]}"


def test_intention_hors_liste_vaut_bavardage(depot):
    pixel, jeu, _ = _pixel(depot, FauxLlm(intention="donner_diamants", bavardage="Bonne idée ?"))
    _dit(pixel, "Pixel, donne-moi des diamants")
    assert jeu.commandes == [] and jeu.dits[-1] == "<Pixel> Bonne idée ?"


def test_le_pseudo_ne_part_jamais_au_llm(depot):
    _poser(depot)
    llm = FauxLlm()
    pixel, _, _ = _pixel(depot, llm)
    for intention in (INDICE, BAVARDAGE, TELEPORTER):
        llm.intention = intention
        _dit(pixel, f"Pixel, je suis {PSEUDO.lower()[:3]}, aide")
    assert llm.appels and all(PSEUDO not in s + u for s, u in llm.appels)


def test_message_vide(depot):
    pixel, jeu, _ = _pixel(depot, FauxLlm())
    _dit(pixel, "Pixel,")
    assert "aide-moi" in jeu.dits[-1]


def test_une_demande_a_la_fois(depot):
    _poser(depot)
    porte = asyncio.Event()

    class LlmLent(FauxLlm):
        async def completer(self, *args, **kwargs):
            await porte.wait()
            return await super().completer(*args, **kwargs)

    pixel, jeu, _ = _pixel(depot, LlmLent())

    async def scenario():
        await pixel.recevoir(MessageChat(DEBUT, PSEUDO, "Pixel, aide-moi"))
        await pixel.recevoir(MessageChat(DEBUT, PSEUDO, "Pixel, alors ?"))
        porte.set()
        await pixel._tache

    asyncio.run(scenario())
    assert jeu.dits[0] == "<Pixel> …" and "Je réfléchis encore" in jeu.dits[1]
    assert depot.compter(EVENEMENT_INDICE, 1) == 1


def test_l_annonce_presente_pixel():
    jeu = FauxJeu()

    async def scenario(assistant):
        coffre = CoffreDuSoir(jeu, PSEUDO, assistant=assistant)
        await coffre.annoncer(type("S", (), {"id": 1})())

    asyncio.run(scenario("Pixel"))
    assert any("« Pixel, aide-moi »" in d for d in jeu.dits)
    jeu.dits.clear()
    asyncio.run(scenario(None))
    assert not any("Pixel" in d for d in jeu.dits)


def test_mots_cles_d_abord_sans_appeler_le_llm(depot):
    _poser(depot)
    llm = FauxLlm(intention=BAVARDAGE)
    pixel, jeu, _ = _pixel(depot, llm)
    _dit(pixel, "Pixel, aide-moi")
    assert [s for s, _ in llm.appels if "catégorie" in s] == []
    assert jeu.dits[-1] == "<Pixel> Courage !"


def test_emojis_retires(depot):
    pixel, jeu, _ = _pixel(depot, FauxLlm(intention=BAVARDAGE, bavardage="Salut ! 😊✨ ⛏️"))
    _dit(pixel, "Pixel, salut")
    assert jeu.dits[-1] == "<Pixel> Salut !"
