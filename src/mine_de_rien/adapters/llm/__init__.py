"""Fournisseurs de LLM."""

from __future__ import annotations

from mine_de_rien.adapters.llm.anthropic import Anthropic
from mine_de_rien.adapters.llm.base import Llm
from mine_de_rien.adapters.llm.openai_compatible import OpenAICompatible
from mine_de_rien.config import Llm as ConfigLlm


def creer(config: ConfigLlm) -> Llm:
    if config.adaptateur == "openai-compatible":
        if not config.url:
            raise ValueError("[llm] url manquante pour l'adaptateur openai-compatible")
        return OpenAICompatible(config.url, config.modele, config.cle)
    if config.adaptateur == "anthropic":
        return Anthropic(config.modele, config.cle)
    raise ValueError(f"[llm] adaptateur inconnu : {config.adaptateur!r}")
