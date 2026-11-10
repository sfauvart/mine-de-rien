import os
from pathlib import Path

import pytest

from mine_de_rien.config import charger_config


def test_substitution_des_variables(tmp_path: Path, monkeypatch):
    monkeypatch.setenv("RCON_PASSWORD", "secret")
    fichier = tmp_path / "config.toml"
    fichier.write_text(
        '[enfant]\npseudo_minecraft = "Steve"\n[jeu]\nmot_de_passe = "${RCON_PASSWORD}"\n',
        encoding="utf-8",
    )
    config = charger_config(fichier)
    assert config.jeu.mot_de_passe == "secret"
    assert config.enfant.alias_llm == "l'élève"


def test_exemple_fourni_se_charge():
    os.environ.pop("TELEGRAM_BOT_TOKEN", None)
    config = charger_config(Path(__file__).resolve().parents[1] / "config.example.toml")
    assert config.llm.adaptateur == "anthropic"


def test_config_essai_vise_le_serveur_d_essai():
    config = charger_config(Path(__file__).resolve().parents[1] / "config.essai.example.toml")
    assert config.jeu.hote == "localhost"
    assert config.jeu.port == 25576  # jamais 25575, le monde de l'enfant


def test_assistant_par_defaut_et_nom_verifie(tmp_path: Path):
    fichier = tmp_path / "config.toml"
    fichier.write_text('[enfant]\npseudo_minecraft = "Steve"\n', encoding="utf-8")
    config = charger_config(fichier)
    assert (config.assistant.nom, config.assistant.actif) == ("Pixel", True)
    fichier.write_text(
        '[enfant]\npseudo_minecraft = "Steve"\n[assistant]\nnom = "@a tp"\n', encoding="utf-8"
    )
    with pytest.raises(ValueError, match="assistant"):
        charger_config(fichier)


def test_paisible_par_defaut_et_difficulte_verifiee(tmp_path: Path):
    fichier = tmp_path / "config.toml"
    fichier.write_text('[enfant]\npseudo_minecraft = "Steve"\n', encoding="utf-8")
    config = charger_config(fichier)
    assert (config.paisible.actif, config.paisible.difficulte_normale) == (True, "easy")
    fichier.write_text(
        '[enfant]\npseudo_minecraft = "Steve"\n[paisible]\ndifficulte_normale = "peaceful"\n',
        encoding="utf-8",
    )
    with pytest.raises(ValueError, match="paisible"):
        charger_config(fichier)
