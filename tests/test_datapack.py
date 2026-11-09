from pathlib import Path

import pytest

from mine_de_rien import cli, datapack


def _source(racine: Path) -> Path:
    source = racine / "source" / "mine_de_rien"
    (source / "data" / "f").mkdir(parents=True)
    (source / "pack.mcmeta").write_text('{"pack": {}}')
    (source / "data" / "f" / "tick.mcfunction").write_text("say tick")
    return source


def _arbre(dossier: Path) -> dict[str, str]:
    return {
        str(p.relative_to(dossier)): p.read_text() for p in dossier.rglob("*") if p.is_file()
    }


def test_le_datapack_du_depot_est_la_source_par_defaut():
    assert (datapack.SOURCE_PAR_DEFAUT / "pack.mcmeta").is_file()


def test_recopie_toute_l_arborescence(tmp_path):
    source = _source(tmp_path)
    cible = datapack.recopier(tmp_path / "sortie", source)
    assert cible == (tmp_path / "sortie" / "mine_de_rien").resolve()
    assert _arbre(cible) == _arbre(source)


def test_un_second_appel_remplace_sans_dupliquer(tmp_path):
    source = _source(tmp_path)
    datapack.recopier(tmp_path / "sortie", source)
    (source / "pack.mcmeta").write_text('{"pack": {"v": 2}}')
    datapack.recopier(tmp_path / "sortie", source)
    assert sorted(p.name for p in (tmp_path / "sortie").iterdir()) == ["mine_de_rien"]
    assert _arbre(tmp_path / "sortie" / "mine_de_rien") == _arbre(source)


def test_un_fichier_retire_de_la_source_disparait(tmp_path):
    source = _source(tmp_path)
    datapack.recopier(tmp_path / "sortie", source)
    (source / "data" / "f" / "tick.mcfunction").unlink()
    datapack.recopier(tmp_path / "sortie", source)
    assert not (tmp_path / "sortie" / "mine_de_rien" / "data" / "f" / "tick.mcfunction").exists()


def test_refuse_une_source_sans_pack_mcmeta(tmp_path):
    source = _source(tmp_path)
    (source / "pack.mcmeta").unlink()
    with pytest.raises(datapack.ErreurDatapack, match="pack.mcmeta"):
        datapack.recopier(tmp_path / "sortie", source)
    assert not (tmp_path / "sortie" / "mine_de_rien").exists()


def test_refuse_de_s_ecraser_elle_meme(tmp_path):
    source = _source(tmp_path)
    with pytest.raises(datapack.ErreurDatapack, match="recouvre"):
        datapack.recopier(source.parent, source)
    assert (source / "pack.mcmeta").is_file()


def test_la_commande_ne_lit_aucune_config(tmp_path, monkeypatch, capsys):
    monkeypatch.chdir(tmp_path)  # aucun config.toml ici
    source = _source(tmp_path)
    code = cli.main(["datapack", str(tmp_path / "sortie"), "--source", str(source)])
    assert code == 0
    assert (tmp_path / "sortie" / "mine_de_rien" / "pack.mcmeta").is_file()
    assert "Datapack recopié" in capsys.readouterr().out


def test_la_commande_signale_une_source_invalide(tmp_path, monkeypatch, capsys):
    monkeypatch.chdir(tmp_path)
    code = cli.main(["datapack", str(tmp_path / "sortie"), "--source", str(tmp_path)])
    assert code == 1
    assert "pack.mcmeta" in capsys.readouterr().err
