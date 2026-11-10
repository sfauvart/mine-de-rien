"""Écrit les fichiers de référence que la version Rust doit reproduire au caractère près.

Usage : uv run python scripts/oracle.py

Temporaire : vit le temps du passage à Rust (spec `2026-11-agent-en-rust`) et disparaît avec
le Python. Chaque fonction `oracle_<module>` rend une liste de cas `{"entree": …, "sortie": …}`
(ou `"erreur"` à la place de `"sortie"` quand le Python refuse), écrite dans
`tests/donnees/oracle/<module>.json`. Les fichiers produits restent : ce sont les tests de
non-régression du port.
"""

from __future__ import annotations

import dataclasses
import json
import os
import tempfile
from pathlib import Path

RACINE = Path(__file__).resolve().parents[1]
SORTIE = RACINE / "tests" / "donnees" / "oracle"


def oracle_config() -> list[dict]:
    from mine_de_rien.config import charger_config

    minimale = '[enfant]\npseudo_minecraft = "Steve"\n'
    entrees = [
        {"toml": (RACINE / "config.example.toml").read_text(encoding="utf-8"),
         "variables": {"RCON_PASSWORD": "secret", "TELEGRAM_BOT_TOKEN": "123:abc"}},
        {"toml": (RACINE / "config.example.toml").read_text(encoding="utf-8"), "variables": {}},
        {"toml": (RACINE / "config.essai.example.toml").read_text(encoding="utf-8"),
         "variables": {}},
        {"toml": minimale, "variables": {}},
        {"toml": minimale + '[canal]\nparents = [12, 34]\njeton = "a${B}c${B}"\n',
         "variables": {"B": "-"}},
        {"toml": minimale + '[llm]\nadaptateur = "openai-compatible"\nurl = "http://llm/v1"\n',
         "variables": {}},
        {"toml": minimale + '[assistant]\nnom = "Zoé"\nactif = false\n', "variables": {}},
        {"toml": minimale + '[assistant]\nnom = "@a tp"\n', "variables": {}},
        {"toml": minimale + '[assistant]\nnom = "P"\n', "variables": {}},
        {"toml": minimale + '[assistant]\nnom = "l\'ami"\n', "variables": {}},
        {"toml": minimale + '[paisible]\ndifficulte_normale = "peaceful"\n', "variables": {}},
        {"toml": minimale + '[paisible]\nactif = false\ndifficulte_normale = "hard"\n',
         "variables": {}},
        {"toml": minimale + '[coffre]\nrecompense = "Diamant"\n', "variables": {}},
        {"toml": minimale + '[coffre]\nrecompense = "emerald"\nrecompense_nombre = 64\n',
         "variables": {}},
        {"toml": minimale + "[coffre]\nrecompense_nombre = 65\n", "variables": {}},
        {"toml": minimale + "[coffre]\nniveaux_offerts = 31\n", "variables": {}},
        {"toml": minimale + "[atelier]\nx = 120\ny = 64\nz = -35\n", "variables": {}},
    ]
    cas = []
    anciennes = dict(os.environ)
    for entree in entrees:
        os.environ.clear()
        os.environ.update(entree["variables"])
        with tempfile.TemporaryDirectory() as dossier:
            fichier = Path(dossier) / "config.toml"
            fichier.write_text(entree["toml"], encoding="utf-8")
            try:
                cas.append({"entree": entree,
                            "sortie": dataclasses.asdict(charger_config(fichier))})
            except ValueError as e:
                cas.append({"entree": entree, "erreur": str(e)})
    os.environ.clear()
    os.environ.update(anciennes)
    return cas


ORACLES = {
    "config": oracle_config,
}


def main() -> None:
    SORTIE.mkdir(parents=True, exist_ok=True)
    for nom, fabrique in ORACLES.items():
        cas = fabrique()
        chemin = SORTIE / f"{nom}.json"
        chemin.write_text(
            json.dumps(cas, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"{chemin.relative_to(RACINE)} : {len(cas)} cas")


if __name__ == "__main__":
    main()
