"""Point d'entrée en ligne de commande."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

from mine_de_rien import __version__
from mine_de_rien.config import charger_config


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="mine-de-rien", description=__doc__)
    parser.add_argument("--config", type=Path, default=Path("config.toml"))
    parser.add_argument("--version", action="version", version=f"mine-de-rien {__version__}")
    args = parser.parse_args(argv)

    config = charger_config(args.config)
    print(
        f"Mine de rien {__version__} : jeu={config.jeu.adaptateur} "
        f"canal={config.canal.adaptateur} llm={config.llm.adaptateur}"
    )
    print("L'orchestrateur n'est pas encore écrit (voir docs/architecture.md, version 0.1).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
