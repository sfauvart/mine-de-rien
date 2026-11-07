"""Adaptateur RCON contre un faux serveur local qui parle le protocole."""

from __future__ import annotations

import asyncio
import json
import socket
import struct
import threading

import pytest

from mine_de_rien.adapters.game.java_rcon import JavaRcon
from mine_de_rien.adapters.game.rcon import TAILLE_MAX_COMMANDE, ErreurRcon


def _paquet(ident: int, type_: int, corps: bytes) -> bytes:
    charge = struct.pack("<ii", ident, type_) + corps + b"\0\0"
    return struct.pack("<i", len(charge)) + charge


def _lire(conn: socket.socket, n: int) -> bytes:
    donnees = b""
    while len(donnees) < n:
        morceau = conn.recv(n - len(donnees))
        if not morceau:
            raise ConnectionError
        donnees += morceau
    return donnees


class FauxServeur:
    """Répond `reponses[commande]`, découpé en paquets de 10 octets pour tester le recollage.

    `couper_apres` : nombre de commandes après lesquelles le serveur ferme la connexion.
    """

    def __init__(self, reponses: dict[str, str], mot_de_passe: str = "essai") -> None:
        self.reponses = reponses
        self.mot_de_passe = mot_de_passe
        self.recues: list[str] = []
        self.connexions = 0
        self.couper_apres: int | None = None
        self._ecoute = socket.create_server(("127.0.0.1", 0))
        self.port = self._ecoute.getsockname()[1]
        threading.Thread(target=self._servir, daemon=True).start()

    def _servir(self) -> None:
        while True:
            try:
                conn, _ = self._ecoute.accept()
            except OSError:
                return
            self.connexions += 1
            threading.Thread(target=self._client, args=(conn,), daemon=True).start()

    def _client(self, conn: socket.socket) -> None:
        commandes = 0
        with conn:
            try:
                while True:
                    (longueur,) = struct.unpack("<i", _lire(conn, 4))
                    charge = _lire(conn, longueur)
                    ident, type_ = struct.unpack("<ii", charge[:8])
                    corps = charge[8:-2].decode("utf-8")
                    if type_ == 3:
                        ok = corps == self.mot_de_passe
                        conn.sendall(_paquet(ident if ok else -1, 2, b""))
                    elif type_ == 2:
                        self.recues.append(corps)
                        reponse = self.reponses.get(corps, "").encode("utf-8")
                        morceaux = [reponse[i : i + 10] for i in range(0, len(reponse), 10)]
                        for morceau in morceaux or [b""]:
                            conn.sendall(_paquet(ident, 0, morceau))
                        commandes += 1
                    else:
                        # Le vrai serveur répond « Unknown request » aux types inconnus.
                        conn.sendall(_paquet(ident, 0, b"Unknown request 0"))
                        if self.couper_apres is not None and commandes >= self.couper_apres:
                            return
            except ConnectionError:
                return

    def arreter(self) -> None:
        self._ecoute.close()


@pytest.fixture
def serveur():
    s = FauxServeur(
        {
            "list": "There are 0 of a max of 20 players online: ",
            "long": "abcdefghij" * 50 + "é",
        }
    )
    yield s
    s.arreter()


def _adaptateur(serveur: FauxServeur, mot_de_passe: str = "essai") -> JavaRcon:
    return JavaRcon("127.0.0.1", serveur.port, mot_de_passe, log="", delai=2)


def test_commande_et_reponse(serveur):
    jeu = _adaptateur(serveur)
    assert asyncio.run(jeu.commande("list")).startswith("There are 0")


def test_reponse_en_plusieurs_paquets_recollee(serveur):
    jeu = _adaptateur(serveur)
    assert asyncio.run(jeu.commande("long")) == "abcdefghij" * 50 + "é"


def test_mauvais_mot_de_passe(serveur):
    jeu = _adaptateur(serveur, mot_de_passe="faux")
    with pytest.raises(ErreurRcon, match="mot de passe"):
        asyncio.run(jeu.commande("list"))


def test_serveur_injoignable():
    libre = socket.create_server(("127.0.0.1", 0))
    port = libre.getsockname()[1]
    libre.close()
    jeu = JavaRcon("127.0.0.1", port, "essai", log="", delai=1)
    with pytest.raises(ErreurRcon, match="injoignable"):
        asyncio.run(jeu.commande("list"))


def test_commandes_simultanees_ne_se_melangent_pas(serveur):
    serveur.reponses.update({f"c{i}": f"r{i}" * 20 for i in range(20)})
    jeu = _adaptateur(serveur)

    async def toutes():
        return await asyncio.gather(*(jeu.commande(f"c{i}") for i in range(20)))

    assert asyncio.run(toutes()) == [f"r{i}" * 20 for i in range(20)]
    assert serveur.connexions == 1


def test_reconnexion_si_la_connexion_est_tombee(serveur):
    serveur.couper_apres = 1
    jeu = _adaptateur(serveur)

    async def deux():
        premiere = await jeu.commande("list")
        await asyncio.sleep(0.1)  # le serveur a fermé entre-temps
        seconde = await jeu.commande("list")
        return premiere, seconde

    premiere, seconde = asyncio.run(deux())
    assert premiere == seconde
    assert serveur.connexions == 2
    # Aucune commande rejouée.
    assert serveur.recues == ["list", "list"]


def test_commande_trop_longue_refusee(serveur):
    jeu = _adaptateur(serveur)
    with pytest.raises(ErreurRcon, match="trop longue"):
        asyncio.run(jeu.commande("say " + "x" * TAILLE_MAX_COMMANDE))
    assert serveur.recues == []


def test_dire_envoie_un_tellraw_serialise(serveur):
    jeu = _adaptateur(serveur)
    asyncio.run(jeu.dire("Steve", 'l\'élève a dit "bonjour" \\ ]}'))
    (commande,) = serveur.recues
    assert commande.startswith("tellraw Steve ")
    assert json.loads(commande.removeprefix("tellraw Steve ")) == {
        "text": 'l\'élève a dit "bonjour" \\ ]}'
    }


def test_dire_remonte_le_refus_du_serveur(serveur):
    serveur.reponses['tellraw Steve {"text": "coucou"}'] = "No player was found"
    jeu = _adaptateur(serveur)
    with pytest.raises(ErreurRcon, match="No player was found"):
        asyncio.run(jeu.dire("Steve", "coucou"))


def test_dire_refuse_un_selecteur(serveur):
    jeu = _adaptateur(serveur)
    with pytest.raises(ValueError):
        asyncio.run(jeu.dire("@a", "coucou"))
    assert serveur.recues == []
