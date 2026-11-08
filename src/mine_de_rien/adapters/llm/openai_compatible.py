"""Tout serveur exposant l'API compatible OpenAI : llama-server (llama.cpp), Ollama, LM Studio.

C'est la voie 100 % locale, la seule à qui l'on confie une photo. llama-server ignore le
champ `model` quand il ne sert qu'un modèle ; Ollama exige l'identifiant exact que donne
`GET {url}/models`.

Ni les images ni les réponses ne sont journalisées.
"""

from __future__ import annotations

import base64

import httpx

from mine_de_rien.adapters.llm.base import ErreurLlm

DELAI = 180.0


class OpenAICompatible:
    distant = False

    def __init__(
        self, url: str, modele: str, cle: str = "", transport: httpx.AsyncBaseTransport | None = None
    ) -> None:
        self.url = url.rstrip("/")
        self.modele = modele
        self.cle = cle
        self._transport = transport

    def _corps(self, systeme: str, utilisateur: str, image: bytes | None, en_json: bool) -> dict:
        contenu: list[dict] = [{"type": "text", "text": utilisateur}]
        if image is not None:
            donnees = base64.b64encode(image).decode()
            contenu.insert(
                0, {"type": "image_url", "image_url": {"url": f"data:image/jpeg;base64,{donnees}"}}
            )
        corps = {
            "model": self.modele,
            "temperature": 0,
            "messages": [
                {"role": "system", "content": systeme},
                {"role": "user", "content": contenu},
            ],
        }
        if en_json:
            corps["response_format"] = {"type": "json_object"}
        return corps

    async def completer(
        self,
        systeme: str,
        utilisateur: str,
        image: bytes | None = None,
        en_json: bool = False,
    ) -> str:
        entetes = {"Authorization": f"Bearer {self.cle}"} if self.cle else {}
        try:
            async with httpx.AsyncClient(timeout=DELAI, transport=self._transport) as client:
                reponse = await client.post(
                    f"{self.url}/chat/completions",
                    json=self._corps(systeme, utilisateur, image, en_json),
                    headers=entetes,
                )
                reponse.raise_for_status()
                return reponse.json()["choices"][0]["message"]["content"]
        except httpx.TimeoutException as e:
            raise ErreurLlm(f"Le LLM local n'a pas répondu en {DELAI:.0f} s.") from e
        except httpx.HTTPStatusError as e:
            raise ErreurLlm(f"Le LLM local a répondu l'erreur {e.response.status_code}.") from e
        except httpx.HTTPError as e:
            raise ErreurLlm(f"Le LLM local est injoignable ({self.url}).") from e
        except (KeyError, IndexError, TypeError, ValueError) as e:
            raise ErreurLlm("Réponse du LLM local dans un format inattendu.") from e
