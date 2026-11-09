"""Recopie du datapack `mine_de_rien` vers le dossier que lit le serveur Minecraft.

Dans l'image publiée, le datapack voyage avec l'agent : un service ponctuel lance
`mine-de-rien datapack /datapacks` avant le serveur, qui le recopie à son tour dans
`world/datapacks`. Code et datapack ont donc toujours la même version.

La source est `datapacks/mine_de_rien` à la racine du projet : le clone en développement,
`/app` dans l'image, où `uv sync` installe le projet en mode éditable.
"""

from __future__ import annotations

import shutil
import tempfile
from pathlib import Path

NOM = "mine_de_rien"
SOURCE_PAR_DEFAUT = Path(__file__).resolve().parents[2] / "datapacks" / NOM


class ErreurDatapack(Exception):
    pass


def recopier(destination: Path, source: Path = SOURCE_PAR_DEFAUT) -> Path:
    """Remplace `destination/mine_de_rien` par une copie complète de `source`.

    Un fichier retiré de la source disparaît aussi de la destination. La copie passe par un
    dossier temporaire voisin, renommé une fois complet : une copie interrompue ne laisse
    jamais un datapack à moitié écrit sous le nom attendu.
    """
    source = source.resolve()
    if not (source / "pack.mcmeta").is_file():
        raise ErreurDatapack(f"{source} n'est pas un datapack : pack.mcmeta introuvable.")
    destination.mkdir(parents=True, exist_ok=True)
    cible = destination.resolve() / NOM
    if cible == source or cible.is_relative_to(source) or source.is_relative_to(cible):
        raise ErreurDatapack(f"La destination {cible} recouvre la source {source}.")

    temporaire = Path(tempfile.mkdtemp(prefix=f".{NOM}-", dir=cible.parent))
    try:
        copie = temporaire / NOM
        shutil.copytree(source, copie)
        if cible.exists() or cible.is_symlink():
            if cible.is_dir() and not cible.is_symlink():
                shutil.rmtree(cible)
            else:
                cible.unlink()
        copie.rename(cible)
    finally:
        shutil.rmtree(temporaire, ignore_errors=True)
    return cible
