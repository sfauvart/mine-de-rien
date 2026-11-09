"""Recherche d'un emplacement, sur un terrain simulé qui répond comme le serveur 26.3."""

import asyncio
import re

import pytest

from mine_de_rien.games.atelier import Atelier, ErreurAtelier
from mine_de_rien.games.emplacement import (
    AUCUN_TERRAIN,
    AUTRE_DIMENSION,
    DISTANCE_MIN,
    ECART_VERTICAL,
    ENFANT_ABSENT,
    RAYON,
    centres,
    chercher,
    commande_hauteur,
    construire_pres_de,
    direction,
    position_de,
)

X, Y, Z = 100, 64, 200
PSEUDO = "Steve_42"


class Terrain:
    """Sol à `Y` partout, sauf les colonnes de `sols` ; `blocs` pour le reste."""

    def __init__(
        self, sols=None, blocs=None, charge=lambda x, z: True, summon="Summoned new Marker",
        joueur=(X + 0.5, float(Y), Z + 0.5), dimension=True,
    ):
        self.joueur = joueur
        self.dimension = dimension
        self.dits: list[tuple[str, str]] = []
        self.sols = sols or {}
        self.blocs = blocs or {}
        self.charge = charge
        self.summon = summon
        self.sondes = 0
        self.recues: list[str] = []

    def sol(self, x, z):
        return self.sols.get((x, z), Y)

    def bloc(self, x, y, z):
        if (x, y, z) in self.blocs:
            return self.blocs[(x, y, z)]
        return "minecraft:air" if y >= self.sol(x, z) else "minecraft:grass_block"

    async def dire(self, joueur, texte, **style):
        self.dits.append((joueur, texte))

    async def commande(self, c: str) -> str:
        self.recues.append(c)
        if c.startswith(f"data get entity {PSEUDO} Pos"):
            if self.joueur is None:
                return "No entity was found"
            x, y, z = self.joueur
            return f"{PSEUDO} has the following entity data: [{x}d, {y}d, {z}d]"
        if c.startswith("execute in minecraft:overworld if entity"):
            return "Test passed. Count: 1" if self.dimension else "Test failed"
        if c.startswith("data get storage"):
            return "Found no elements matching pos"
        if c.startswith("forceload"):
            return "Marked 2 chunks to be force loaded"
        if c.startswith("kill"):
            self.sondes = 0
            return "Killed Marker"
        if c.startswith("summon"):
            self.sondes += 1 if self.summon.startswith("Summoned") else 0
            return self.summon
        m = re.match(r"execute positioned (-?\d+) 0 (-?\d+) positioned over", c)
        if m:
            x, z = int(m[1]), int(m[2])
            if not self.charge(x, z):
                return "That position is not loaded"
            return f"Teleported Marker to {x}.500000, {self.sol(x, z)}.000000, {z}.500000"
        m = re.fullmatch(r"execute if block (-?\d+) (-?\d+) (-?\d+) (\S+)", c)
        if m:
            x, y, z = map(int, m.groups()[:3])
            if not self.charge(x, z):
                return "That position is not loaded"
            return "Test passed" if self.bloc(x, y, z) == m[4] else "Test failed"
        return ""


def _chercher(terrain):
    return asyncio.run(chercher(terrain, X, Y, Z))


PREMIER = (X - DISTANCE_MIN, Z - DISTANCE_MIN)


def test_couronnes_du_plus_proche_au_plus_loin_sans_doublon():
    tous = list(centres(0, 0))
    assert len(tous) == len(set(tous)) == (2 * RAYON + 1) ** 2 - (2 * DISTANCE_MIN - 1) ** 2
    distances = [max(abs(x), abs(z)) for x, z in tous]
    assert distances == sorted(distances)
    assert distances[0] == DISTANCE_MIN and distances[-1] == RAYON


def test_terrain_plat_premier_centre():
    terrain = Terrain()
    assert _chercher(terrain) == Atelier(PREMIER[0], Y, PREMIER[1])


def test_ordre_deterministe():
    assert _chercher(Terrain()) == _chercher(Terrain())


def test_commande_de_hauteur():
    assert commande_hauteur(-3, 12) == (
        "execute positioned -3 0 12 positioned over motion_blocking_no_leaves "
        "run tp @e[type=minecraft:marker,tag=mdr_sonde,limit=1] ~ ~ ~"
    )


def test_une_bosse_d_un_bloc_ecarte_les_centres_qui_la_couvrent():
    px, pz = PREMIER
    terrain = Terrain(sols={(px, pz): Y + 1})
    a = _chercher(terrain)
    assert a == Atelier(px + 3, Y, pz)


def test_l_eau_au_niveau_du_plancher_est_refusee():
    px, pz = PREMIER
    terrain = Terrain(blocs={(px, Y - 1, pz): "minecraft:water"})
    assert _chercher(terrain) == Atelier(px + 3, Y, pz)


def test_une_construction_au_dessus_du_sol_est_refusee():
    px, pz = PREMIER
    terrain = Terrain(blocs={(px, Y + 1, pz): "minecraft:oak_planks"})
    assert _chercher(terrain) == Atelier(px + 3, Y, pz)


def test_jamais_sur_l_enfant_ni_dans_sa_zone():
    a = _chercher(Terrain())
    assert max(abs(a.x - X), abs(a.z - Z)) >= DISTANCE_MIN


def test_surface_trop_loin_verticalement_refusee():
    """Au fond d'une mine : tout le terrain est plat, mais très au-dessus de l'enfant."""
    haut = Y + ECART_VERTICAL + 1

    class Mine(Terrain):
        def sol(self, x, z):
            return haut

    terrain = Mine()
    with pytest.raises(ErreurAtelier, match="sous terre"):
        _chercher(terrain)
    assert not any("execute if block" in c for c in terrain.recues)
    assert terrain.sondes == 0


def test_ecart_vertical_limite_accepte():
    limite = Y + ECART_VERTICAL

    class Colline(Terrain):
        def sol(self, x, z):
            return limite

    assert _chercher(Colline()).y == limite


def test_chunk_non_charge_ecarte_le_centre():
    px, _pz = PREMIER
    terrain = Terrain(charge=lambda x, z: x > px + 2)
    a = _chercher(terrain)
    assert a is not None and a.x - 2 > px + 2


def test_aucun_emplacement_valide():
    class Bosselé(Terrain):
        def sol(self, x, z):
            return Y + (x + z) % 2

    terrain = Bosselé()
    assert _chercher(terrain) is None
    assert terrain.sondes == 0


def test_la_sonde_est_retiree_apres_succes():
    terrain = Terrain()
    _chercher(terrain)
    assert terrain.recues[-1] == "kill @e[type=minecraft:marker,tag=mdr_sonde]"
    assert terrain.sondes == 0


def test_la_sonde_est_retiree_si_le_serveur_tombe():
    class Coupure(Terrain):
        async def commande(self, c):
            if "positioned over" in c:
                raise ConnectionError("RCON coupé")
            return await super().commande(c)

    terrain = Coupure()
    with pytest.raises(ConnectionError):
        _chercher(terrain)
    assert terrain.sondes == 0


def test_refus_si_la_sonde_ne_peut_pas_etre_posee():
    with pytest.raises(ErreurAtelier, match="sonder"):
        _chercher(Terrain(summon="Unable to summon entity"))


def test_hauteurs_lues_une_seule_fois():
    terrain = Terrain(sols={(X - 8, Z - 8): Y + 1})
    _chercher(terrain)
    hauteurs = [c for c in terrain.recues if "positioned over" in c]
    assert len(hauteurs) == len(set(hauteurs))


@pytest.mark.parametrize(
    "dx, dz, attendu",
    [(10, 3, "l'est"), (-10, 3, "l'ouest"), (2, 9, "le sud"), (2, -9, "le nord")],
)
def test_direction(dx, dz, attendu):
    assert direction(dx, dz) == attendu


def test_position_arrondie_vers_le_bas_en_negatif():
    terrain = Terrain(joueur=(-27.5, 68.0, -0.25))
    assert asyncio.run(position_de(terrain, PSEUDO)) == (-28, 68, -1)


def test_position_refuse_un_pseudo_dangereux():
    with pytest.raises(ValueError):
        asyncio.run(position_de(Terrain(), "@a"))


def test_enfant_hors_ligne():
    terrain = Terrain(joueur=None)
    with pytest.raises(ErreurAtelier, match=ENFANT_ABSENT):
        asyncio.run(construire_pres_de(terrain, PSEUDO))
    assert not any(c.startswith("summon") for c in terrain.recues)


def test_enfant_dans_une_autre_dimension():
    with pytest.raises(ErreurAtelier, match=AUTRE_DIMENSION):
        asyncio.run(construire_pres_de(Terrain(dimension=False), PSEUDO))


def test_rien_trouve_rien_construit():
    class Bosselé(Terrain):
        def sol(self, x, z):
            return Y + (x + z) % 2

    terrain = Bosselé()
    with pytest.raises(ErreurAtelier, match="Pas de terrain plat") as e:
        asyncio.run(construire_pres_de(terrain, PSEUDO))
    assert str(e.value) == AUCUN_TERRAIN
    assert not any(c.startswith(("fill", "setblock", "forceload")) for c in terrain.recues)
    assert terrain.dits == []


def test_construit_et_previent_l_enfant():
    terrain = Terrain()
    c = asyncio.run(construire_pres_de(terrain, PSEUDO))
    assert c.atelier == Atelier(X - 8, Y, Z - 8)
    assert c.distance == 11
    assert terrain.recues[-1].startswith("data modify storage")
    assert terrain.dits == [(PSEUDO, "Ton atelier vient d'apparaître, à 11 blocs vers l'ouest.")]
