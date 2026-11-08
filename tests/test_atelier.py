import asyncio
import re

import pytest

from mine_de_rien.games.atelier import (
    Atelier,
    ErreurAtelier,
    commande_forceload,
    commandes_construction,
    construire,
    verifications_de_place,
)


class FauxMonde:
    """Répond aux commandes de l'atelier comme le serveur 26.3, à partir d'un dict de blocs."""

    def __init__(self, blocs: dict[tuple[int, int, int], str], pos_enregistree=None):
        self.blocs = blocs
        self.pos = pos_enregistree
        self.recues: list[str] = []

    def _bloc(self, x, y, z) -> str:
        if (x, y, z) in self.blocs:
            return self.blocs[(x, y, z)]
        return "minecraft:air" if y >= -60 else "minecraft:grass_block"

    async def commande(self, commande: str) -> str:
        self.recues.append(commande)
        if commande.startswith("data get storage"):
            if self.pos is None:
                return "Found no elements matching pos"
            return f"Storage mine_de_rien:atelier has the following contents: {list(self.pos)}"
        if commande.startswith("forceload"):
            return "Marked 2 chunks to be force loaded"
        m = re.fullmatch(r"execute if block (-?\d+) (-?\d+) (-?\d+) (\S+)", commande)
        if m:
            x, y, z = map(int, m.groups()[:3])
            attendu = m.group(4)
            bloc = self._bloc(x, y, z)
            tags = {
                "#minecraft:dirt": {"minecraft:dirt"},
                "#minecraft:anvil": {"minecraft:anvil"},
                "#minecraft:standing_signs": {"minecraft:oak_sign"},
            }
            ok = bloc == attendu or bloc in tags.get(attendu, set())
            return "Test passed" if ok else "Test failed"
        return ""


A = Atelier(0, -60, 8)


def test_commandes_pour_des_coordonnees_fixees():
    assert commande_forceload(A) == "forceload add -2 6 2 10"
    assert commandes_construction(A) == [
        "fill -2 -61 6 2 -61 10 minecraft:smooth_stone",
        "execute unless block 0 -60 8 minecraft:chest run setblock 0 -60 8 minecraft:chest[facing=south]",
        "execute unless block 1 -60 8 #minecraft:anvil run setblock 1 -60 8 minecraft:anvil[facing=east]",
        "execute unless block 2 -60 6 minecraft:lantern run setblock 2 -60 6 minecraft:lantern",
        "kill @e[type=minecraft:marker,tag=mdr_atelier]",
        'summon minecraft:marker 0.5 -60 8.5 {Tags:["mdr_atelier"]}',
        "data modify storage mine_de_rien:atelier pos set value [0, -60, 8]",
    ]


def test_marqueur_au_centre_du_bloc_en_negatif():
    (summon,) = [c for c in commandes_construction(Atelier(-3, 64, -7)) if "summon" in c]
    assert "marker -2.5 64 -6.5 " in summon


def test_volume_verifie():
    positions = [p for p, _ in verifications_de_place(A, deja_construit=False)]
    assert len(positions) == 5 * 5 * 3 + 5 * 5
    assert (-2, -60, 6) in positions and (2, -58, 10) in positions and (0, -61, 8) in positions


def test_construit_sur_un_terrain_libre():
    monde = FauxMonde({})
    assert asyncio.run(construire(monde, A)) == "Atelier construit en 0 -60 8."
    assert monde.recues[-1].startswith("data modify storage")


def test_refus_si_un_bloc_gene_sans_rien_casser():
    monde = FauxMonde({(1, -59, 9): "minecraft:oak_planks"})
    with pytest.raises(ErreurAtelier, match="1 -59 9"):
        asyncio.run(construire(monde, A))
    assert not any(c.startswith(("fill", "setblock", "summon", "kill")) for c in monde.recues)
    assert monde.recues[-1].startswith("forceload remove")


def test_refus_si_le_plancher_est_une_construction():
    monde = FauxMonde({(0, -61, 8): "minecraft:oak_planks"})
    with pytest.raises(ErreurAtelier, match="0 -61 8"):
        asyncio.run(construire(monde, A))


def test_le_coffre_d_autrui_n_est_pas_pris_pour_le_notre():
    monde = FauxMonde({(0, -60, 8): "minecraft:chest"})
    with pytest.raises(ErreurAtelier, match="0 -60 8"):
        asyncio.run(construire(monde, A))


def test_relance_reconnait_ses_propres_blocs():
    monde = FauxMonde(
        {
            (0, -60, 8): "minecraft:chest",
            (1, -60, 8): "minecraft:anvil",
            (-1, -60, 8): "minecraft:oak_sign",
            (2, -60, 6): "minecraft:lantern",
            **{(x, -61, z): "minecraft:smooth_stone" for x in range(-2, 3) for z in range(6, 11)},
        },
        pos_enregistree=(0, -60, 8),
    )
    assert asyncio.run(construire(monde, A)) == "Atelier réparé en 0 -60 8."
    # Un seul marqueur : l'ancien est retiré avant d'en poser un nouveau.
    assert next(c for c in monde.recues if "marker" in c).startswith("kill")


def test_refus_si_l_atelier_est_ailleurs():
    monde = FauxMonde({}, pos_enregistree=(100, 64, 100))
    with pytest.raises(ErreurAtelier, match="100 64 100"):
        asyncio.run(construire(monde, A))
