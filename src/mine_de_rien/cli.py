"""Point d'entrée en ligne de commande."""

from __future__ import annotations

import argparse
import asyncio
import sys
from pathlib import Path

from mine_de_rien import __version__
from mine_de_rien.adapters.game.java_rcon import JavaRcon
from mine_de_rien.adapters.game.rcon import ErreurRcon
from mine_de_rien.config import Config, charger_config
from mine_de_rien.games import atelier


def _jeu(config: Config) -> JavaRcon:
    j = config.jeu
    return JavaRcon(j.hote, j.port, j.mot_de_passe, j.log)


async def _dire(config: Config, joueur: str, message: str) -> None:
    jeu = _jeu(config)
    try:
        await jeu.dire(joueur, message)
    finally:
        await jeu.fermer()


async def _atelier(config: Config) -> str:
    if config.atelier is None:
        raise atelier.ErreurAtelier("Section [atelier] absente de la configuration.")
    a = config.atelier
    jeu = _jeu(config)
    try:
        return await atelier.construire(jeu, atelier.Atelier(a.x, a.y, a.z))
    finally:
        await jeu.fermer()


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(prog="mine-de-rien", description=__doc__)
    parser.add_argument("--config", type=Path, default=Path("config.toml"))
    parser.add_argument("--version", action="version", version=f"mine-de-rien {__version__}")
    sous = parser.add_subparsers(dest="action")
    dire = sous.add_parser("dire", help="affiche un message à un joueur dans le chat")
    dire.add_argument("joueur")
    dire.add_argument("message")
    sous.add_parser("atelier", help="construit ou répare l'atelier des coffres")
    args = parser.parse_args(argv)

    config = charger_config(args.config)

    if args.action == "dire":
        try:
            asyncio.run(_dire(config, args.joueur, args.message))
        except (ErreurRcon, ValueError) as e:
            print(f"Erreur : {e}", file=sys.stderr)
            return 1
        return 0

    if args.action == "atelier":
        try:
            print(asyncio.run(_atelier(config)))
        except (ErreurRcon, atelier.ErreurAtelier) as e:
            print(f"Erreur : {e}", file=sys.stderr)
            return 1
        return 0

    print(
        f"Mine de rien {__version__} : jeu={config.jeu.adaptateur} "
        f"canal={config.canal.adaptateur} llm={config.llm.adaptateur}"
    )
    print("L'orchestrateur n'est pas encore écrit (voir docs/architecture.md, version 0.1).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
