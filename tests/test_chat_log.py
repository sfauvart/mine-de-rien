"""Chat lu dans latest.log : analyse des lignes et suivi du fichier, rotation comprise."""

from __future__ import annotations

import asyncio
import os
from datetime import datetime

import pytest

from mine_de_rien.adapters.game.base import MessageChat
from mine_de_rien.adapters.game.java_rcon import JavaRcon
from mine_de_rien.adapters.game.log import SuiviLog, analyser, suivre_chat

JOUR = datetime(2026, 11, 8, 20, 0)
OBSERVEE = "[18:45:26] [Async Chat Thread - #0/INFO]: [Not Secure] <seb> salut"


def test_ligne_observee_sur_le_serveur_d_essai():
    # Daté de la réception : l'heure du log est celle du conteneur (UTC).
    assert analyser(OBSERVEE, JOUR) == MessageChat(JOUR, "seb", "salut")


@pytest.mark.parametrize(
    "ligne, attendu",
    [
        ("[09:01:02] [Server thread/INFO]: <Steve_42> Pixel, aide-moi\n",
         ("Steve_42", "Pixel, aide-moi")),
        ("[09:01:02] [Async Chat Thread - #3/INFO]: <seb> <autre> bonjour",
         ("seb", "<autre> bonjour")),
        ("[09:01:02] [Async Chat Thread - #0/INFO]: [Not Secure] <seb> août, ça marche\r\n",
         ("seb", "août, ça marche")),
    ],
)
def test_messages_reconnus(ligne, attendu):
    message = analyser(ligne, JOUR)
    assert (message.joueur, message.texte) == attendu


@pytest.mark.parametrize(
    "ligne",
    [
        "[09:01:02] [Server thread/INFO]: [seb] dit par /say",
        "[09:01:02] [Server thread/INFO]: seb joined the game",
        "[09:01:02] [Server thread/INFO]: [Rcon] <seb> faux",
        "[09:01:02] [Server thread/WARN]: <seb> avertissement",
        "[09:01:02] [Server thread/INFO]: <a> pseudo trop court",
        "<seb> sans horodatage",
        "",
    ],
)
def test_autres_lignes_ignorees(ligne):
    assert analyser(ligne, JOUR) is None


def _ajouter(chemin, *lignes: str) -> None:
    with open(chemin, "a", encoding="utf-8") as f:
        f.write("".join(f"{ligne}\n" for ligne in lignes))


def test_ne_rejoue_pas_ce_qui_est_deja_ecrit(tmp_path):
    log = tmp_path / "latest.log"
    _ajouter(log, "ancien")
    suivi = SuiviLog(log)
    assert suivi.lire() == []
    _ajouter(log, "nouveau")
    assert suivi.lire() == ["nouveau"]
    assert suivi.lire() == []


def test_ligne_incomplete_attend_sa_fin(tmp_path):
    log = tmp_path / "latest.log"
    log.write_text("")
    suivi = SuiviLog(log)
    suivi.lire()
    with open(log, "a", encoding="utf-8") as f:
        f.write("début")
    assert suivi.lire() == []
    _ajouter(log, " et fin")
    assert suivi.lire() == ["début et fin"]


def test_rotation_fichier_remplace(tmp_path):
    log = tmp_path / "latest.log"
    _ajouter(log, "avant")
    suivi = SuiviLog(log)
    suivi.lire()
    os.rename(log, tmp_path / "2026-11-08-1.log.gz")
    _ajouter(log, "après redémarrage")
    assert suivi.lire() == ["après redémarrage"]


def test_rotation_fichier_tronque(tmp_path):
    log = tmp_path / "latest.log"
    _ajouter(log, "une longue ligne d'avant")
    suivi = SuiviLog(log)
    suivi.lire()
    log.write_text("court\n")
    assert suivi.lire() == ["court"]


def test_fichier_absent_puis_cree(tmp_path):
    log = tmp_path / "latest.log"
    suivi = SuiviLog(log)
    assert suivi.lire() == []
    _ajouter(log, "premier message du serveur")
    assert suivi.lire() == ["premier message du serveur"]


def test_flux_des_messages(tmp_path):
    log = tmp_path / "latest.log"
    _ajouter(log, OBSERVEE)

    async def scenario():
        flux = suivre_chat(log, intervalle=0.01, horloge=lambda: JOUR)
        tache = asyncio.ensure_future(flux.__anext__())
        await asyncio.sleep(0.05)
        _ajouter(log, "[20:00:01] [Server thread/INFO]: seb joined the game",
                 "[20:00:02] [Async Chat Thread - #0/INFO]: [Not Secure] <seb> Pixel, aide")
        message = await asyncio.wait_for(tache, 2)
        await flux.aclose()
        return message

    message = asyncio.run(scenario())
    assert (message.joueur, message.texte) == ("seb", "Pixel, aide")


def test_sans_chemin_de_log():
    with pytest.raises(ValueError, match="log"):
        JavaRcon("localhost", 25575, "secret", "").chat()
