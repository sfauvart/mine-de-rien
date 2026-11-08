"""Refuse de publier une image dont la version ne correspond pas au tag git.

Usage : python scripts/verifier_version.py v0.1.0

Le tag `vX.Y.Z` doit égaler la version de `pyproject.toml`, `__version__` du paquet (ce
qu'affiche `mine-de-rien --version`) et chaque tag de l'image `ghcr.io/sfauvart/mine-de-rien`
cité dans `docker-compose.yml`. Sort avec le code 1 et la liste des écarts sinon.
"""

from __future__ import annotations

import re
import sys
import tomllib
from pathlib import Path

RACINE = Path(__file__).resolve().parents[1]
_TAG = re.compile(r"^v(\d+\.\d+\.\d+)$")
_VERSION_PAQUET = re.compile(r'^__version__\s*=\s*"([^"]+)"', re.MULTILINE)
_IMAGE = re.compile(r"^\s*image:\s*[\"']?ghcr\.io/sfauvart/mine-de-rien(?::([^\s\"']+))?", re.MULTILINE)


def verifier(tag: str, racine: Path = RACINE) -> list[str]:
    """Liste des écarts entre le tag et les versions déclarées ; vide si tout concorde."""
    m = _TAG.match(tag)
    if not m:
        return [f"Le tag « {tag} » n'a pas la forme vX.Y.Z."]
    attendue = m.group(1)
    ecarts = []

    with (racine / "pyproject.toml").open("rb") as f:
        projet = tomllib.load(f)["project"]["version"]
    if projet != attendue:
        ecarts.append(f"pyproject.toml annonce {projet}, le tag {attendue}.")

    init = (racine / "src" / "mine_de_rien" / "__init__.py").read_text(encoding="utf-8")
    trouvee = _VERSION_PAQUET.search(init)
    paquet = trouvee.group(1) if trouvee else "rien"
    if paquet != attendue:
        ecarts.append(f"src/mine_de_rien/__init__.py annonce {paquet}, le tag {attendue}.")

    compose = (racine / "docker-compose.yml").read_text(encoding="utf-8")
    images = [m.group(1) or "sans tag" for m in _IMAGE.finditer(compose)]
    if not images:
        ecarts.append("docker-compose.yml ne cite pas l'image ghcr.io/sfauvart/mine-de-rien.")
    for image in images:
        if image != attendue:
            ecarts.append(f"docker-compose.yml cite l'image {image}, le tag {attendue}.")

    return ecarts


def main(argv: list[str]) -> int:
    if len(argv) != 1:
        print(__doc__, file=sys.stderr)
        return 2
    ecarts = verifier(argv[0])
    for ecart in ecarts:
        print(f"Erreur : {ecart}", file=sys.stderr)
    if ecarts:
        return 1
    print(f"Versions cohérentes avec {argv[0]}.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
