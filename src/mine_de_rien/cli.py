"""Point d'entrée en ligne de commande."""

from __future__ import annotations

import argparse
import asyncio
import logging
import random
import sys
import time
from pathlib import Path

from mine_de_rien import __version__, datapack, photo
from mine_de_rien.adapters.channel.telegram import Telegram
from mine_de_rien.adapters.game.java_rcon import JavaRcon
from mine_de_rien.adapters.game.rcon import ErreurRcon
from mine_de_rien.adapters.llm import creer as creer_llm
from mine_de_rien.adapters.llm.base import ErreurLlm
from mine_de_rien.config import Config, charger_config
from mine_de_rien.games import atelier
from mine_de_rien.games.coffre_du_soir import CoffreDuSoir, NotificateurConsole
from mine_de_rien.games.pixel import Pixel
from mine_de_rien.orchestrateur import (
    CommandesParents,
    NotificateurCanal,
    Orchestrateur,
    mettre_en_file,
    servir,
)
from mine_de_rien.saisie import ErreurSaisie
from mine_de_rien.storage.sqlite import Depot


def _depot(config: Config) -> Depot:
    chemin = Path(config.stockage.sqlite)
    chemin.parent.mkdir(parents=True, exist_ok=True)
    return Depot.ouvrir(chemin)


def _atelier_config(config: Config) -> atelier.Atelier:
    if config.atelier is None:
        raise atelier.ErreurAtelier(
            "Section [atelier] absente de config.toml. Sans coordonnées : /atelier sur Telegram."
        )
    a = config.atelier
    return atelier.Atelier(a.x, a.y, a.z)


def _jeu(config: Config) -> JavaRcon:
    j = config.jeu
    return JavaRcon(j.hote, j.port, j.mot_de_passe, j.log)


async def _dire(config: Config, joueur: str, message: str) -> None:
    jeu = _jeu(config)
    try:
        await jeu.dire(joueur, message)
    finally:
        await jeu.fermer()


async def _ecouter(config: Config) -> None:
    print(f"Chat lu dans {config.jeu.log}. Ctrl+C pour arrêter.", flush=True)
    async for message in _jeu(config).chat():
        print(f"{message.horodatage:%H:%M:%S} <{message.joueur}> {message.texte}", flush=True)


async def _atelier(config: Config) -> str:
    a = _atelier_config(config)
    jeu = _jeu(config)
    try:
        return await atelier.construire(jeu, a)
    finally:
        await jeu.fermer()


async def _llm(config: Config, consigne: str, image: Path | None) -> str:
    llm = creer_llm(config.llm)
    octets = image.read_bytes() if image is not None else None
    return await llm.completer("Tu réponds en français.", consigne, octets)


async def _lire_photo(config: Config, image: Path, consigne: str, nombre: int) -> str:
    debut = time.monotonic()
    lecture = await photo.lire(creer_llm(config.llm), image.read_bytes(), consigne)
    lignes = photo.resumer(lecture)
    lignes.append(f"Lu en {time.monotonic() - debut:.0f} s. Tirage de {nombre} :")
    lignes += [f"• {ex.enonce} → {ex.reponse}" for ex in photo.tirer(lecture, nombre,
                                                                    random.Random())]
    return "\n".join(lignes)


async def _lancer(config: Config) -> None:
    jeu = _jeu(config)
    depot = _depot(config)
    pixel_actif = config.assistant.actif and bool(config.jeu.log)
    coffre = CoffreDuSoir(
        jeu, config.enfant.pseudo_minecraft, config.coffre.niveaux_offerts,
        assistant=config.assistant.nom if pixel_actif else None,
    )
    print(f"Surveillance de l'atelier, coffres dans {config.stockage.sqlite}. Ctrl+C pour arrêter.")
    canal = None
    if config.canal.jeton:
        if not config.canal.parents:
            print("Aucun parent dans [canal] parents : seul /start répondra.", file=sys.stderr)
        canal = Telegram(config.canal.jeton, config.canal.parents)
        await canal.demarrer()
        print(f"Telegram : {len(config.canal.parents)} parent(s) autorisé(s).")
    else:
        print("Pas de jeton Telegram : les notifications s'affichent ici.")
    try:
        llm = creer_llm(config.llm)
    except ValueError as e:
        print(f"LLM désactivé (photos, Pixel en mots-clés) : {e}", file=sys.stderr)
        llm = None
    notificateur = NotificateurConsole() if canal is None else NotificateurCanal(canal)
    taches = [Orchestrateur(depot, coffre, notificateur).tourner()]
    if pixel_actif:
        pixel = Pixel(jeu, depot, config.enfant.pseudo_minecraft, config.assistant.nom, llm)
        mode = "LLM local" if pixel.llm is not None else "mots-clés"
        print(f"{config.assistant.nom} écoute le chat ({mode}).")
        taches.append(pixel.ecouter())
    if canal is not None:
        commandes = CommandesParents(
            depot, config.coffre, coffre, config.canal.parents, llm=llm
        )
        taches.append(servir(canal, commandes))
    try:
        await asyncio.gather(*taches)
    finally:
        if canal is not None:
            await canal.arreter()
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
    sous.add_parser("ecouter", help="affiche le chat des joueurs au fil de l'eau, pour vérifier")
    sous.add_parser("atelier", help="construit ou répare l'atelier des coffres")
    coffre = sous.add_parser("coffre", help="met un exercice dans la file des coffres")
    coffre.add_argument("texte", help="un calcul (« 348 ÷ 12 ») ou une compétence (« division 2 »)")
    sous.add_parser("lancer", help="surveille l'atelier et pose les coffres de la file")
    llm = sous.add_parser("llm", help="pose une question au LLM configuré, pour l'essayer")
    llm.add_argument("consigne")
    llm.add_argument("--image", type=Path, help="photo JPEG jointe à la question")
    lire = sous.add_parser("lire-photo", help="lit une fiche photographiée, sans rien mettre en file")
    lire.add_argument("image", type=Path)
    lire.add_argument("--consigne", default="", help="partie de la page, ex. « exercice 7 »")
    lire.add_argument("--nombre", type=int, default=3, help="exercices à tirer au hasard")
    pack = sous.add_parser(
        "datapack", help="recopie le datapack dans un dossier lu par le serveur, sans config"
    )
    pack.add_argument("dossier", type=Path, help="reçoit le sous-dossier mine_de_rien")
    pack.add_argument(
        "--source", type=Path, default=datapack.SOURCE_PAR_DEFAUT, help="datapack à recopier"
    )
    args = parser.parse_args(argv)

    # Avant charger_config : le service qui livre le datapack n'a pas de config.toml.
    if args.action == "datapack":
        try:
            cible = datapack.recopier(args.dossier, args.source)
        except (datapack.ErreurDatapack, OSError) as e:
            print(f"Erreur : {e}", file=sys.stderr)
            return 1
        print(f"Datapack recopié dans {cible}")
        return 0

    config = charger_config(args.config)

    if args.action == "dire":
        try:
            asyncio.run(_dire(config, args.joueur, args.message))
        except (ErreurRcon, ValueError) as e:
            print(f"Erreur : {e}", file=sys.stderr)
            return 1
        return 0

    if args.action == "ecouter":
        try:
            asyncio.run(_ecouter(config))
        except KeyboardInterrupt:
            pass
        except ValueError as e:
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
            print("\n".join(mettre_en_file(depot, config.coffre, args.texte, "cli")))
        except ErreurSaisie as e:
            print(f"Refusé : {e}", file=sys.stderr)
            return 1
        finally:
            depot.fermer()
        return 0

    if args.action == "llm":
        try:
            print(asyncio.run(_llm(config, args.consigne, args.image)))
        except (ErreurLlm, ValueError, OSError) as e:
            print(f"Erreur : {e}", file=sys.stderr)
            return 1
        return 0

    if args.action == "lire-photo":
        try:
            print(asyncio.run(_lire_photo(config, args.image, args.consigne, args.nombre)))
        except (ErreurLlm, photo.ErreurPhoto, ValueError, OSError) as e:
            print(f"Erreur : {e}", file=sys.stderr)
            return 1
        return 0

    if args.action == "lancer":
        logging.basicConfig(level=logging.INFO, format="%(asctime)s %(message)s")
        try:
            asyncio.run(_lancer(config))
        except KeyboardInterrupt:
            pass
        return 0

    print(
        f"Mine de rien {__version__} : jeu={config.jeu.adaptateur} "
        f"canal={config.canal.adaptateur} llm={config.llm.adaptateur}"
    )
    print("L'orchestrateur n'est pas encore écrit (voir docs/architecture.md, version 0.1).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
