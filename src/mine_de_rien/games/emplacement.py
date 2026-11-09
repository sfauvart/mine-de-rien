"""Trouver où construire l'atelier, autour de l'enfant, sans relevé manuel.

**Le désir.** Le parent ne connaît pas Minecraft : pas de F3, pas de coordonnées à recopier.
L'agent cherche lui-même, près de l'enfant, un endroit où l'atelier tient sans rien casser.
`chercher` ne construit rien ; `construire_pres_de` enchaîne position de l'enfant, recherche et
construction, pour la commande `/atelier` des parents.

**Commandes**, vérifiées sur Minecraft Java 26.3 (Paper) :

    kill @e[type=minecraft:marker,tag=mdr_sonde]                # une sonde oubliée
    summon minecraft:marker X Y Z {Tags:["mdr_sonde"]}          # à la position de l'enfant
    execute positioned X 0 Z positioned over motion_blocking_no_leaves
        run tp @e[type=minecraft:marker,tag=mdr_sonde,limit=1] ~ ~ ~
    # « Teleported Marker to 141.500000, -59.000000, 150.500000 » : Y est le niveau juste
    # au-dessus du sol, donc celui du coffre. « That position is not loaded » hors chunk chargé.
    execute if block X Y Z <bloc|#tag>                          # voir `atelier.py`
    data get entity <pseudo> Pos
    # « Steve has the following entity data: [3.5d, -60.0d, 4.25d] », « No entity was found »
    execute in minecraft:overworld if entity @a[name=<pseudo>,distance=0..]
    # `distance` ne retient que les entités de la dimension de l'exécution

La sonde porte son propre tag : le datapack ne regarde que `mdr_atelier`, elle ne crée donc
pas de zone en mode aventure. Elle est retirée même en cas d'erreur.

**Recherche.** Par couronnes carrées, de 8 à 32 blocs de l'enfant, toujours dans le même ordre :
le premier centre valide gagne. Un centre est valide si :
- ses 25 colonnes sont exactement au même niveau, sans terrassement ;
- ce niveau est à 16 blocs au plus de celui de l'enfant, pour ne pas construire loin au-dessus
  d'une mine ;
- les blocs passent `atelier.place_libre` : sol naturel, de l'air ou de l'herbe au-dessus. L'eau,
  les troncs et les constructions sont écartés par là ou par le niveau des colonnes.

8 blocs au moins : le 5 × 5 ne tombe pas sur l'enfant, et il reste hors de la zone en mode
aventure (6 blocs autour du marqueur).

Autour d'un joueur connecté, les chunks sont chargés : la recherche ne fait pas de `forceload`,
et un chunk non chargé écarte simplement le centre.

**Message à l'enfant et rapport aux parents.** Une ligne dans le chat de l'enfant, avec la
distance et la direction, calculées ici. Les parents reçoivent les coordonnées et la distance,
jamais le pseudo.

**Contournements.** L'enfant ne déclenche rien : seuls les parents le peuvent, sur Telegram.
Il peut choisir où il se tient au moment de la commande, sans autre effet qu'un atelier à cet
endroit, que les parents peuvent déplacer. Une terrasse en terre qu'il aurait posée passe pour du
sol : la commande ne distingue pas un bloc posé d'un bloc naturel.
"""

from __future__ import annotations

import math
import re
from collections.abc import Iterator
from dataclasses import dataclass

from mine_de_rien.adapters.game.base import AdaptateurJeu
from mine_de_rien.games.atelier import Atelier, ErreurAtelier, construire, place_libre
from mine_de_rien.minecraft import texte

DISTANCE_MIN = 8
RAYON = 32
ECART_VERTICAL = 16

TAG_SONDE = "mdr_sonde"
_SONDES = f"@e[type=minecraft:marker,tag={TAG_SONDE}]"
_SONDE = f"@e[type=minecraft:marker,tag={TAG_SONDE},limit=1]"
_NOMBRE = r"(-?\d+(?:\.\d+)?)"
_TELEPORTE = re.compile(rf"Teleported .* to {_NOMBRE}, {_NOMBRE}, {_NOMBRE}")
_POSITION = re.compile(rf"\[{_NOMBRE}d, {_NOMBRE}d, {_NOMBRE}d\]")

# Pour les parents : pourquoi rien n'a été construit, et quel geste faire.
ENFANT_ABSENT = "L'enfant n'est pas connecté : je cherche autour de lui. Réessayer quand il joue."
AUTRE_DIMENSION = (
    "L'enfant est dans le Nether ou l'End : réessayer quand il est revenu dans le monde normal."
)
LOIN_DE_LA_SURFACE = (
    "L'enfant est sous terre ou très haut : réessayer quand il est à la surface, près de sa base."
)
AUCUN_TERRAIN = (
    f"Pas de terrain plat et dégagé à moins de {RAYON} blocs de l'enfant. L'emmener vers un "
    "terrain dégagé (prairie, plaine), puis relancer /atelier."
)


def centres(x: int, z: int, minimum: int = DISTANCE_MIN, maximum: int = RAYON) -> Iterator[
    tuple[int, int]
]:
    """Les centres possibles, couronne par couronne, du plus proche au plus loin."""
    for r in range(minimum, maximum + 1):
        for d in range(-r, r + 1):
            yield (x + d, z - r)
        for d in range(-r, r + 1):
            yield (x + d, z + r)
        for d in range(-r + 1, r):
            yield (x - r, z + d)
        for d in range(-r + 1, r):
            yield (x + r, z + d)


def commande_hauteur(x: int, z: int) -> str:
    return (
        f"execute positioned {x} 0 {z} positioned over motion_blocking_no_leaves "
        f"run tp {_SONDE} ~ ~ ~"
    )


async def _hauteur(
    jeu: AdaptateurJeu, x: int, z: int, connues: dict[tuple[int, int], int | None]
) -> int | None:
    """Le niveau juste au-dessus du sol, gardé d'un centre à l'autre ; None si non chargé."""
    if (x, z) not in connues:
        trouve = _TELEPORTE.search(await jeu.commande(commande_hauteur(x, z)))
        connues[(x, z)] = int(float(trouve.group(2))) if trouve else None
    return connues[(x, z)]


async def _niveau_commun(
    jeu: AdaptateurJeu, x: int, z: int, connues: dict[tuple[int, int], int | None]
) -> int | None:
    """Le niveau des 25 colonnes autour de (x, z), s'il est le même pour toutes."""
    niveau = await _hauteur(jeu, x, z, connues)
    if niveau is None:
        return None
    for dz in range(-2, 3):
        for dx in range(-2, 3):
            if await _hauteur(jeu, x + dx, z + dz, connues) != niveau:
                return None
    return niveau


async def chercher(jeu: AdaptateurJeu, x: int, y: int, z: int) -> Atelier | None:
    """Le premier emplacement valide autour de (x, y, z), ou None. Ne construit rien."""
    await jeu.commande(f"kill {_SONDES}")
    reponse = await jeu.commande(f'summon minecraft:marker {x} {y} {z} {{Tags:["{TAG_SONDE}"]}}')
    if "Summoned" not in reponse:
        raise ErreurAtelier(f"Impossible de sonder le terrain : {reponse.strip()}")
    connues: dict[tuple[int, int], int | None] = {}
    try:
        ici = await _hauteur(jeu, x, z, connues)
        if ici is not None and abs(ici - y) > ECART_VERTICAL:
            raise ErreurAtelier(LOIN_DE_LA_SURFACE)
        for cx, cz in centres(x, z):
            niveau = await _niveau_commun(jeu, cx, cz, connues)
            if niveau is None or abs(niveau - y) > ECART_VERTICAL:
                continue
            candidat = Atelier(cx, niveau, cz)
            if await place_libre(jeu, candidat):
                return candidat
        return None
    finally:
        await jeu.commande(f"kill {_SONDES}")


async def position_de(jeu: AdaptateurJeu, pseudo: str) -> tuple[int, int, int]:
    """Le bloc où se tient le joueur, dans l'Overworld ; `ErreurAtelier` sinon."""
    pseudo = texte.verifier_pseudo(pseudo)
    trouve = _POSITION.search(await jeu.commande(f"data get entity {pseudo} Pos"))
    if trouve is None:
        raise ErreurAtelier(ENFANT_ABSENT)
    reponse = await jeu.commande(
        f"execute in minecraft:overworld if entity @a[name={pseudo},distance=0..]"
    )
    if not reponse.strip().startswith("Test passed"):
        raise ErreurAtelier(AUTRE_DIMENSION)
    x, y, z = (math.floor(float(v)) for v in trouve.groups())
    return (x, y, z)


def direction(dx: int, dz: int) -> str:
    """Le point cardinal dominant : dans Minecraft, x croît vers l'est et z vers le sud."""
    if abs(dx) >= abs(dz):
        return "l'est" if dx > 0 else "l'ouest"
    return "le sud" if dz > 0 else "le nord"


@dataclass(frozen=True)
class Construction:
    atelier: Atelier
    distance: int
    direction: str


async def trouver_pres_de(jeu: AdaptateurJeu, pseudo: str) -> Construction:
    """Où construire près du joueur. Ne construit rien ; `ErreurAtelier` sinon."""
    x, y, z = await position_de(jeu, pseudo)
    a = await chercher(jeu, x, y, z)
    if a is None:
        raise ErreurAtelier(AUCUN_TERRAIN)
    dx, dz = a.x - x, a.z - z
    return Construction(a, round(math.hypot(dx, dz)), direction(dx, dz))


async def annoncer(jeu: AdaptateurJeu, pseudo: str, c: Construction) -> None:
    await jeu.dire(
        pseudo,
        f"Ton atelier vient d'apparaître, à {c.distance} blocs vers {c.direction}.",
        color="gold",
    )


async def construire_pres_de(jeu: AdaptateurJeu, pseudo: str) -> Construction:
    """Construit l'atelier près du joueur et le lui dit."""
    c = await trouver_pres_de(jeu, pseudo)
    await construire(jeu, c.atelier)
    await annoncer(jeu, pseudo, c)
    return c
