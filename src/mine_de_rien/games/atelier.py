"""L'atelier : le lieu fixe, près de la base de l'enfant, où apparaissent les coffres du soir.

Plan, vu de dessus, `C` étant les coordonnées données dans `config.toml` (`[atelier]`) :

    z-2  . . . . L        L : lanterne
    z-1  . . . . .        P : emplacement du panneau d'énoncé (posé par le coffre du soir)
    z    . P C E .        C : coffre, ouverture vers le sud (z+)
    z+1  . . . . .        E : enclume
    z+2  . . . . .        plancher 5 × 5 en pierre lisse, à y-1
       x-2     x+2

Commandes, vérifiées sur Minecraft Java 26.3 (Paper) :

    forceload add X-2 Z-2 X+2 Z+2          # sans joueur proche, rien n'est chargé depuis 1.21
    execute if block X Y Z <bloc|#tag>     # « Test passed » / « Test failed », bloc par bloc
    fill X-2 Y-1 Z-2 X+2 Y-1 Z+2 minecraft:smooth_stone
    execute unless block ... run setblock ...   # ne remplace jamais un coffre déjà là
    summon minecraft:marker X+0.5 Y Z+0.5 {Tags:["mdr_atelier"]}
    data modify storage mine_de_rien:atelier pos set value [X, Y, Z]

Le `forceload` reste en place : le coffre doit pouvoir être posé et surveillé pendant que
l'enfant joue ailleurs.

Le marqueur `mdr_atelier` est lu par le datapack (`datapacks/mine_de_rien`), qui met en mode
aventure les joueurs en survie autour de lui : en aventure, on ne casse pas le coffre et on ne
pose pas d'entonnoir dessous.

Avant de construire, l'agent vérifie que le volume au-dessus du plancher est de l'air ou de
l'herbe, et que le plancher est du sol naturel ; sinon il refuse, sans rien casser, en disant
où est le bloc qui gêne. Relancé, il répare sans dupliquer : il reconnaît alors ses propres
blocs, et ne touche pas au contenu d'un coffre déjà posé.

La position écrite dans `mine_de_rien:atelier` fait foi (`lire_atelier`) : l'agent la relit
au lieu de garder celle de `config.toml`, qui ne sert plus qu'à la construction. Sans position
dans le monde, le coffre du soir ne se pose pas et Pixel ne téléporte pas : l'enfant serait
emmuré.
"""

from __future__ import annotations

from collections.abc import Iterator
from dataclasses import dataclass

from mine_de_rien.adapters.game.base import AdaptateurJeu

TAG_MARQUEUR = "mdr_atelier"
STOCKAGE = "mine_de_rien:atelier"
PLANCHER = "minecraft:smooth_stone"

# Au-dessus du plancher : de l'air ou de l'herbe.
LIBRE = (
    "minecraft:air",
    "minecraft:cave_air",
    "minecraft:short_grass",
    "minecraft:tall_grass",
    "minecraft:fern",
    "minecraft:large_fern",
    "minecraft:short_dry_grass",
    "minecraft:tall_dry_grass",
    "minecraft:bush",
    "minecraft:dead_bush",
    "minecraft:snow",
    "#minecraft:small_flowers",
)
# Le plancher remplace du sol naturel, jamais une construction.
SOL = (
    "minecraft:air",
    "minecraft:grass_block",
    "minecraft:podzol",
    "minecraft:mycelium",
    "minecraft:moss_block",
    "minecraft:gravel",
    "minecraft:snow_block",
    "#minecraft:dirt",
    "#minecraft:sand",
    "#minecraft:base_stone_overworld",
)


class ErreurAtelier(Exception):
    """Construction refusée ; le message s'adresse au parent."""


Position = tuple[int, int, int]


@dataclass(frozen=True)
class Atelier:
    x: int
    y: int
    z: int

    @property
    def coffre(self) -> Position:
        return (self.x, self.y, self.z)

    @property
    def panneau(self) -> Position:
        return (self.x - 1, self.y, self.z)

    @property
    def enclume(self) -> Position:
        return (self.x + 1, self.y, self.z)

    @property
    def lanterne(self) -> Position:
        return (self.x + 2, self.y, self.z - 2)

    def volume(self) -> Iterator[Position]:
        """Les 5 × 5 × 3 blocs au-dessus du plancher."""
        for dy in range(3):
            for dz in range(-2, 3):
                for dx in range(-2, 3):
                    yield (self.x + dx, self.y + dy, self.z + dz)

    def plancher(self) -> Iterator[Position]:
        for dz in range(-2, 3):
            for dx in range(-2, 3):
                yield (self.x + dx, self.y - 1, self.z + dz)

    def blocs_propres(self) -> dict[Position, str]:
        """Les blocs que l'atelier pose lui-même, reconnus quand on le relance."""
        return {
            self.coffre: "minecraft:chest",
            self.panneau: "#minecraft:standing_signs",
            self.enclume: "#minecraft:anvil",
            self.lanterne: "minecraft:lantern",
        }


def _pos(p: Position) -> str:
    return f"{p[0]} {p[1]} {p[2]}"


def commande_forceload(a: Atelier) -> str:
    return f"forceload add {a.x - 2} {a.z - 2} {a.x + 2} {a.z + 2}"


def verifications_de_place(a: Atelier, deja_construit: bool) -> list[tuple[Position, list[str]]]:
    """Pour chaque position, les commandes `execute if block` dont une doit passer."""
    propres = a.blocs_propres() if deja_construit else {}
    tests: list[tuple[Position, list[str]]] = []
    for p in a.volume():
        permis = list(LIBRE) + ([propres[p]] if p in propres else [])
        tests.append((p, [f"execute if block {_pos(p)} {b}" for b in permis]))
    for p in a.plancher():
        permis = list(SOL) + ([PLANCHER] if deja_construit else [])
        tests.append((p, [f"execute if block {_pos(p)} {b}" for b in permis]))
    return tests


def commandes_construction(a: Atelier) -> list[str]:
    """Commandes de construction, idempotentes : relancées, elles ne dupliquent rien."""
    c, e, lt = _pos(a.coffre), _pos(a.enclume), _pos(a.lanterne)
    return [
        f"fill {a.x - 2} {a.y - 1} {a.z - 2} {a.x + 2} {a.y - 1} {a.z + 2} {PLANCHER}",
        f"execute unless block {c} minecraft:chest run setblock {c} minecraft:chest[facing=south]",
        f"execute unless block {e} #minecraft:anvil run setblock {e} minecraft:anvil[facing=east]",
        f"execute unless block {lt} minecraft:lantern run setblock {lt} minecraft:lantern",
        f"kill @e[type=minecraft:marker,tag={TAG_MARQUEUR}]",
        # Centre du bloc : x + 0,5, ce qui donne -2.5 pour x = -3 (et non « -3.5 »).
        f"summon minecraft:marker {a.x + 0.5} {a.y} {a.z + 0.5} {{Tags:[\"{TAG_MARQUEUR}\"]}}",
        f"data modify storage {STOCKAGE} pos set value [{a.x}, {a.y}, {a.z}]",
    ]


def _passe(reponse: str) -> bool:
    return reponse.strip().startswith("Test passed")


def _non_charge(reponse: str) -> bool:
    return "not loaded" in reponse


async def position_enregistree(jeu: AdaptateurJeu) -> Position | None:
    reponse = await jeu.commande(f"data get storage {STOCKAGE} pos")
    # « Storage mine_de_rien:atelier has the following contents: [0, -60, 8] »
    if "[" not in reponse:
        return None
    valeurs = reponse[reponse.rindex("[") + 1 : reponse.rindex("]")].split(",")
    try:
        x, y, z = (int(v.strip().rstrip("d")) for v in valeurs)
    except ValueError:
        return None
    return (x, y, z)


async def lire_atelier(jeu: AdaptateurJeu) -> Atelier | None:
    """L'atelier construit dans le monde du serveur, s'il y en a un."""
    position = await position_enregistree(jeu)
    return Atelier(*position) if position is not None else None


# Pour les parents : quoi faire quand le monde n'a pas d'atelier.
PAS_CONSTRUIT = (
    "l'atelier n'est pas construit dans ce monde. Le construire avec la commande "
    "« mine-de-rien atelier » (docs/installation.md, étape 7)."
)


async def construire(jeu: AdaptateurJeu, a: Atelier) -> str:
    """Construit ou répare l'atelier. Renvoie un compte rendu ; lève `ErreurAtelier` si refusé."""
    ancienne = await position_enregistree(jeu)
    if ancienne is not None and ancienne != a.coffre:
        raise ErreurAtelier(
            f"Un atelier existe déjà en {_pos(ancienne)}. Le déplacer n'est pas prévu : "
            "remettre ces coordonnées dans config.toml."
        )
    deja_construit = ancienne == a.coffre

    reponse = await jeu.commande(commande_forceload(a))
    if "orce" not in reponse and "already" not in reponse:
        raise ErreurAtelier(f"Impossible de garder l'atelier chargé : {reponse.strip()}")

    for position, tests in verifications_de_place(a, deja_construit):
        for test in tests:
            reponse = await jeu.commande(test)
            if _non_charge(reponse):
                raise ErreurAtelier(f"La position {_pos(position)} n'est pas chargée.")
            if _passe(reponse):
                break
        else:
            if not deja_construit:
                await jeu.commande(commande_forceload(a).replace(" add ", " remove "))
            raise ErreurAtelier(
                f"Le bloc en {_pos(position)} gêne : l'atelier a besoin d'un plancher de sol "
                "naturel et, au-dessus, de 5 × 5 × 3 blocs d'air ou d'herbe. "
                "Rien n'a été cassé."
            )

    for commande in commandes_construction(a):
        reponse = await jeu.commande(commande)
        if "rror" in reponse or "nknown" in reponse or "Incorrect" in reponse:
            raise ErreurAtelier(f"Le serveur a refusé « {commande} » : {reponse.strip()}")

    verbe = "réparé" if deja_construit else "construit"
    return f"Atelier {verbe} en {_pos(a.coffre)}."
