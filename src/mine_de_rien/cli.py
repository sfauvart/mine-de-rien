"""Point d'entrée en ligne de commande."""

from __future__ import annotations

import argparse
import asyncio
import logging
import sys
from pathlib import Path

from mine_de_rien import __version__
from mine_de_rien.adapters.game.java_rcon import JavaRcon
from mine_de_rien.adapters.game.rcon import ErreurRcon
from mine_de_rien.config import Config, charger_config
from mine_de_rien.games import atelier
from mine_de_rien.games.coffre_du_soir import CoffreDuSoir, NotificateurConsole
from mine_de_rien.orchestrateur import Orchestrateur
from mine_de_rien.saisie import ErreurSaisie, interpreter
from mine_de_rien.storage.sqlite import Depot


def _depot(config: Config) -> Depot:
    chemin = Path(config.stockage.sqlite)
    chemin.parent.mkdir(parents=True, exist_ok=True)
    return Depot.ouvrir(chemin)


def _atelier_config(config: Config) -> atelier.Atelier:
    if config.atelier is None:
        raise atelier.ErreurAtelier("Section [atelier] absente de la configuration.")
    a = config.atelier
    return atelier.Atelier(a.x, a.y, a.z)


def mettre_en_file(depot: Depot, config: Config, texte: str, demande_par: str) -> list[str]:
    """Ajoute à la file les exercices saisis ; renvoie une ligne par coffre. Lève ErreurSaisie."""
    saisie = interpreter(texte)
    lignes = []
    for ex in saisie.exercices:
        numero = depot.ajouter(
            ex,
            source=saisie.source,
            demande_par=demande_par,
            recompense=config.coffre.recompense,
            recompense_nombre=config.coffre.recompense_nombre,
        )
        lignes.append(f"Coffre n°{numero} en file : {ex.enonce} (réponse {ex.reponse})")
    return lignes


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
    a = _atelier_config(config)
    jeu = _jeu(config)
    try:
        return await atelier.construire(jeu, a)
    finally:
        await jeu.fermer()


async def _lancer(config: Config) -> None:
    jeu = _jeu(config)
    depot = _depot(config)
    coffre = CoffreDuSoir(
        jeu, _atelier_config(config), config.enfant.pseudo_minecraft,
        config.coffre.niveaux_offerts,
    )
    print(f"Surveillance de l'atelier, coffres dans {config.stockage.sqlite}. Ctrl+C pour arrêter.")
    try:
        await Orchestrateur(depot, coffre, NotificateurConsole()).tourner()
    finally:
        depot.fermer()
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
    coffre = sous.add_parser("coffre", help="met un exercice dans la file des coffres")
    coffre.add_argument("texte", help="un calcul (« 348 ÷ 12 ») ou une compétence (« division 2 »)")
    sous.add_parser("lancer", help="surveille l'atelier et pose les coffres de la file")
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

    if args.action == "coffre":
        depot = _depot(config)
        try:
            print("\n".join(mettre_en_file(depot, config, args.texte, "cli")))
        except ErreurSaisie as e:
            print(f"Refusé : {e}", file=sys.stderr)
            return 1
        finally:
            depot.fermer()
        return 0

    if args.action == "lancer":
        logging.basicConfig(level=logging.INFO, format="%(asctime)s %(message)s")
        try:
            asyncio.run(_lancer(config))
        except KeyboardInterrupt:
            pass
        except atelier.ErreurAtelier as e:
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
