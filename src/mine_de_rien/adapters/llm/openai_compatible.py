"""Tout serveur exposant l'API compatible OpenAI : llama-server (llama.cpp), Ollama, LM Studio.

C'est la voie 100 % locale.
"""

from __future__ import annotations


class OpenAICompatible:
    def __init__(self, url: str, modele: str, cle: str = "") -> None:
        self.url = url.rstrip("/")
        self.modele = modele
        self.cle = cle

    async def completer(self, systeme: str, utilisateur: str, image: bytes | None = None) -> str:
        raise NotImplementedError("version 0.1 : POST {url}/chat/completions via httpx")
