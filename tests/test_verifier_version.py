import importlib.util
from pathlib import Path

import pytest

from mine_de_rien import __version__

_CHEMIN = Path(__file__).resolve().parents[1] / "scripts" / "verifier_version.py"
_spec = importlib.util.spec_from_file_location("verifier_version", _CHEMIN)
verifier_version = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(verifier_version)


def _projet(
    racine: Path, pyproject: str, paquet: str, images: tuple[str, ...] = (":1.2.3", ":1.2.3")
) -> Path:
    """Un faux dépôt ; les images du compose suivent le tag v1.2.3 des tests par défaut."""
    (racine / "src" / "mine_de_rien").mkdir(parents=True)
    (racine / "pyproject.toml").write_text(f'[project]\nname = "x"\nversion = "{pyproject}"\n')
    (racine / "src" / "mine_de_rien" / "__init__.py").write_text(
        f'"""Doc."""\n\n__version__ = "{paquet}"\n'
    )
    services = "".join(
        f"  s{i}:\n    image: ghcr.io/sfauvart/mine-de-rien{tag}\n" for i, tag in enumerate(images)
    )
    (racine / "docker-compose.yml").write_text(
        f"services:\n  minecraft:\n    image: itzg/minecraft-server:2026.10.1\n{services}"
    )
    return racine


def test_le_depot_est_coherent_avec_sa_propre_version():
    assert verifier_version.verifier(f"v{__version__}") == []


def test_tout_concorde(tmp_path):
    assert verifier_version.verifier("v1.2.3", _projet(tmp_path, "1.2.3", "1.2.3")) == []


def test_pyproject_en_retard(tmp_path):
    ecarts = verifier_version.verifier("v1.2.3", _projet(tmp_path, "1.2.2", "1.2.3"))
    assert ecarts == ["pyproject.toml annonce 1.2.2, le tag 1.2.3."]


def test_version_du_paquet_oubliee(tmp_path):
    ecarts = verifier_version.verifier("v1.2.3", _projet(tmp_path, "1.2.3", "1.2.2"))
    assert ecarts == ["src/mine_de_rien/__init__.py annonce 1.2.2, le tag 1.2.3."]


def test_une_image_du_compose_oubliee(tmp_path):
    racine = _projet(tmp_path, "1.2.3", "1.2.3", (":1.2.3", ":1.2.2"))
    assert verifier_version.verifier("v1.2.3", racine) == [
        "docker-compose.yml cite l'image 1.2.2, le tag 1.2.3."
    ]


def test_une_image_du_compose_sans_tag(tmp_path):
    racine = _projet(tmp_path, "1.2.3", "1.2.3", ("", ":latest"))
    assert verifier_version.verifier("v1.2.3", racine) == [
        "docker-compose.yml cite l'image sans tag, le tag 1.2.3.",
        "docker-compose.yml cite l'image latest, le tag 1.2.3.",
    ]


def test_le_compose_doit_citer_l_image(tmp_path):
    racine = _projet(tmp_path, "1.2.3", "1.2.3", ())
    assert verifier_version.verifier("v1.2.3", racine) == [
        "docker-compose.yml ne cite pas l'image ghcr.io/sfauvart/mine-de-rien."
    ]


@pytest.mark.parametrize("tag", ["1.2.3", "v1.2", "v1.2.3-rc1", "main"])
def test_tag_mal_forme(tmp_path, tag):
    ecarts = verifier_version.verifier(tag, _projet(tmp_path, "1.2.3", "1.2.3"))
    assert len(ecarts) == 1 and "vX.Y.Z" in ecarts[0]


def test_code_de_sortie(capsys):
    assert verifier_version.main([f"v{__version__}"]) == 0
    assert verifier_version.main(["v999.0.0"]) == 1
    assert "pyproject.toml annonce" in capsys.readouterr().err
    assert verifier_version.main([]) == 2
