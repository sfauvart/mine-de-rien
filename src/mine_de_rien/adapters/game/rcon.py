"""Client RCON minimal et synchrone, fait pour tourner dans `asyncio.to_thread`.

On n'utilise pas `mcrcon` : il installe un gestionnaire `SIGALRM` dans son constructeur,
ce qui échoue hors du thread principal, et son délai d'attente lèverait l'exception dans la
boucle asyncio plutôt que dans le thread. Ici, les délais sont ceux du socket.

Protocole : paquets `<longueur:int32><id:int32><type:int32><corps>\\0\\0`, en petit-boutiste.
Une réponse longue arrive en plusieurs paquets ; on envoie après chaque commande un paquet
sentinelle d'un type inconnu, auquel le serveur répond une fois la commande traitée. Tout ce
qui arrive avant la réponse à la sentinelle appartient à la commande.
"""

from __future__ import annotations

import select
import socket
import struct

_CONNEXION = 3
_COMMANDE = 2
_SENTINELLE = 0

# Le serveur vanilla refuse les paquets entrants dont le corps dépasse cette taille.
TAILLE_MAX_COMMANDE = 1446


class ErreurRcon(Exception):
    """Le serveur est injoignable, a refusé le mot de passe ou a répondu de travers."""


class ClientRcon:
    def __init__(self, hote: str, port: int, mot_de_passe: str, delai: float = 5.0) -> None:
        self.hote = hote
        self.port = port
        self.mot_de_passe = mot_de_passe
        self.delai = delai
        self._socket: socket.socket | None = None
        self._prochain_id = 0

    @property
    def connecte(self) -> bool:
        return self._socket is not None

    def connecter(self) -> None:
        self.fermer()
        try:
            self._socket = socket.create_connection((self.hote, self.port), timeout=self.delai)
        except OSError as e:
            raise ErreurRcon(f"serveur RCON injoignable sur {self.hote}:{self.port} : {e}") from e
        try:
            ident = self._envoyer(_CONNEXION, self.mot_de_passe)
            reponse_id, _, _ = self._lire_paquet()
        except OSError as e:
            self.fermer()
            raise ErreurRcon(f"connexion RCON interrompue : {e}") from e
        if reponse_id == -1:
            self.fermer()
            raise ErreurRcon("mot de passe RCON refusé")
        if reponse_id != ident:
            self.fermer()
            raise ErreurRcon("réponse RCON inattendue à l'authentification")

    def fermer(self) -> None:
        if self._socket is not None:
            try:
                self._socket.close()
            finally:
                self._socket = None

    def connexion_tombee(self) -> bool:
        """Vrai si le serveur a fermé la connexion depuis le dernier échange.

        Le test se fait avant d'envoyer, pour pouvoir se reconnecter sans risquer
        d'exécuter deux fois une commande.
        """
        if self._socket is None:
            return True
        lisibles, _, _ = select.select([self._socket], [], [], 0)
        if not lisibles:
            return False
        try:
            return self._socket.recv(1, socket.MSG_PEEK) == b""
        except OSError:
            return True

    def commande(self, commande: str) -> str:
        """Envoie une commande et renvoie la réponse complète du serveur."""
        if len(commande.encode("utf-8")) > TAILLE_MAX_COMMANDE:
            raise ErreurRcon(f"commande trop longue (plus de {TAILLE_MAX_COMMANDE} octets)")
        if self._socket is None:
            raise ErreurRcon("pas de connexion RCON")
        # Une erreur à cet envoi-ci remonte telle quelle : rien n'est parti, l'appelant peut
        # se reconnecter et réessayer.
        ident = self._envoyer(_COMMANDE, commande)
        morceaux: list[bytes] = []
        try:
            sentinelle = self._envoyer(_SENTINELLE, "")
            while True:
                reponse_id, _, corps = self._lire_paquet()
                if reponse_id == sentinelle:
                    break
                if reponse_id == ident:
                    morceaux.append(corps)
        except OSError as e:
            # La commande a peut-être été exécutée : on ne la rejoue pas.
            self.fermer()
            raise ErreurRcon(f"connexion RCON interrompue pendant la réponse : {e}") from e
        return b"".join(morceaux).decode("utf-8", errors="replace")

    def _envoyer(self, type_: int, corps: str) -> int:
        assert self._socket is not None
        self._prochain_id = self._prochain_id % 2_000_000_000 + 1
        charge = struct.pack("<ii", self._prochain_id, type_) + corps.encode("utf-8") + b"\0\0"
        self._socket.sendall(struct.pack("<i", len(charge)) + charge)
        return self._prochain_id

    def _lire_paquet(self) -> tuple[int, int, bytes]:
        (longueur,) = struct.unpack("<i", self._lire(4))
        if not 10 <= longueur <= 1 << 20:
            raise OSError(f"longueur de paquet RCON invalide : {longueur}")
        charge = self._lire(longueur)
        ident, type_ = struct.unpack("<ii", charge[:8])
        return ident, type_, charge[8:-2]

    def _lire(self, n: int) -> bytes:
        assert self._socket is not None
        donnees = b""
        while len(donnees) < n:
            morceau = self._socket.recv(n - len(donnees))
            if not morceau:
                raise ConnectionResetError("connexion fermée par le serveur")
            donnees += morceau
        return donnees
