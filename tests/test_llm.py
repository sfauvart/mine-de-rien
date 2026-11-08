"""Adaptateurs LLM, contre un faux serveur httpx."""

from __future__ import annotations

import asyncio
import base64
import json

import httpx
import pytest

from mine_de_rien.adapters.llm import creer
from mine_de_rien.adapters.llm.anthropic import Anthropic
from mine_de_rien.adapters.llm.base import ErreurLlm, lire_json
from mine_de_rien.adapters.llm.openai_compatible import OpenAICompatible
from mine_de_rien.config import Llm as ConfigLlm


def _serveur(contenu: str, requetes: list[httpx.Request], statut: int = 200):
    def repondre(requete: httpx.Request) -> httpx.Response:
        requetes.append(requete)
        return httpx.Response(statut, json={"choices": [{"message": {"content": contenu}}]})

    return httpx.MockTransport(repondre)


def test_requete_porte_image_temperature_et_format_json():
    requetes: list[httpx.Request] = []
    llm = OpenAICompatible("http://llm:8080/v1/", "gemma", transport=_serveur("{}", requetes))
    reponse = asyncio.run(llm.completer("système", "lis", image=b"\xff\xd8jpeg", en_json=True))
    assert reponse == "{}"
    (requete,) = requetes
    assert str(requete.url) == "http://llm:8080/v1/chat/completions"
    assert "authorization" not in requete.headers
    corps = json.loads(requete.content)
    assert corps["temperature"] == 0
    assert corps["model"] == "gemma"
    assert corps["response_format"] == {"type": "json_object"}
    systeme, utilisateur = corps["messages"]
    assert systeme == {"role": "system", "content": "système"}
    image, texte = utilisateur["content"]
    assert image["image_url"]["url"] == "data:image/jpeg;base64," + base64.b64encode(
        b"\xff\xd8jpeg"
    ).decode()
    assert texte == {"type": "text", "text": "lis"}


def test_sans_image_ni_json():
    requetes: list[httpx.Request] = []
    llm = OpenAICompatible("http://llm/v1", "m", cle="secret", transport=_serveur("ok", requetes))
    assert asyncio.run(llm.completer("s", "u")) == "ok"
    corps = json.loads(requetes[0].content)
    assert "response_format" not in corps
    assert corps["messages"][1]["content"] == [{"type": "text", "text": "u"}]
    assert requetes[0].headers["authorization"] == "Bearer secret"


def test_erreur_http_devient_erreur_llm():
    llm = OpenAICompatible("http://llm/v1", "m", transport=_serveur("", [], statut=500))
    with pytest.raises(ErreurLlm, match="500"):
        asyncio.run(llm.completer("s", "u"))


def test_serveur_injoignable():
    def couper(_: httpx.Request) -> httpx.Response:
        raise httpx.ConnectError("refusé")

    llm = OpenAICompatible("http://llm/v1", "m", transport=httpx.MockTransport(couper))
    with pytest.raises(ErreurLlm, match="injoignable"):
        asyncio.run(llm.completer("s", "u"))


def test_lire_json_avec_ou_sans_balises():
    assert lire_json('{"a": 1}') == {"a": 1}
    assert lire_json('```json\n{"lignes": []}\n```') == {"lignes": []}
    assert lire_json('  ```\n{"b": 2}\n```  ') == {"b": 2}


def test_lire_json_refuse_le_texte_libre():
    with pytest.raises(ErreurLlm):
        lire_json("Voici les lignes : Double de 2")
    with pytest.raises(ErreurLlm):
        lire_json("[1, 2]")


def test_anthropic_refuse_toute_image():
    llm = Anthropic("claude", "cle")
    assert llm.distant
    with pytest.raises(ErreurLlm, match="local"):
        asyncio.run(llm.completer("s", "u", image=b"jpeg"))


def test_fabrique():
    local = creer(ConfigLlm(adaptateur="openai-compatible", url="http://llm/v1", modele="m"))
    assert isinstance(local, OpenAICompatible) and not local.distant
    assert isinstance(creer(ConfigLlm(adaptateur="anthropic")), Anthropic)
    with pytest.raises(ValueError):
        creer(ConfigLlm(adaptateur="openai-compatible"))
    with pytest.raises(ValueError):
        creer(ConfigLlm(adaptateur="autre"))
