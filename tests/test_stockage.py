import sqlite3
from datetime import UTC, datetime, timedelta

import pytest

from mine_de_rien.exercises.base import Exercice
from mine_de_rien.storage.sqlite import VERSION_SCHEMA, Depot, ErreurStockage

T0 = datetime(2026, 11, 7, 18, 0, tzinfo=UTC)


def _ex(n: int) -> Exercice:
    return Exercice("maths/division-posee", f"{n * 12} ÷ 12 = ?", str(n), {"n": n})


@pytest.fixture
def depot(tmp_path):
    d = Depot.ouvrir(tmp_path / "mdr.sqlite")
    yield d
    d.fermer()


def _ajouter(depot: Depot, n: int, quand: datetime = T0) -> int:
    return depot.ajouter(
        _ex(n), source="expression", demande_par="42", recompense="diamond",
        recompense_nombre=3, quand=quand,
    )


def test_schema_date(depot):
    assert depot.cx.execute("PRAGMA user_version").fetchone()[0] == VERSION_SCHEMA


def test_file_dans_l_ordre_et_sans_les_annules(depot):
    ids = [_ajouter(depot, n) for n in (1, 2, 3, 4)]
    depot.annuler(ids[1])
    assert [e.id for e in depot.en_file()] == [ids[0], ids[2], ids[3]]
    assert depot.suivant().id == ids[0]


def test_cycle_de_vie(depot):
    a, b = _ajouter(depot, 1), _ajouter(depot, 2)
    depot.marquer_pose(a, T0 + timedelta(minutes=1))
    assert depot.actif().id == a
    assert depot.suivant().id == b
    with pytest.raises(ErreurStockage, match="déjà posé"):
        depot.marquer_pose(b)
    depot.marquer_ouvert(a, T0 + timedelta(minutes=5))
    lu = depot.lire(a)
    assert lu.statut == "ouvert"
    assert lu.ouvert_le - lu.pose_le == timedelta(minutes=4)
    assert lu.exercice == _ex(1)
    assert depot.actif() is None


def test_annuler(depot):
    a = _ajouter(depot, 1)
    depot.marquer_pose(a)
    assert depot.annuler(a).statut == "pose"
    assert depot.annuler(a) is None
    assert depot.annuler(999) is None
    assert depot.actif() is None


def test_ouvrir_un_exercice_non_pose_echoue(depot):
    a = _ajouter(depot, 1)
    with pytest.raises(ErreurStockage):
        depot.marquer_ouvert(a)


def test_entre(depot):
    _ajouter(depot, 1, T0 - timedelta(days=8))
    b = _ajouter(depot, 2, T0 - timedelta(days=1))
    assert [e.id for e in depot.entre(T0 - timedelta(days=7), T0)] == [b]


def test_ancienne_base_sans_version_refusee(tmp_path):
    chemin = tmp_path / "vieille.sqlite"
    cx = sqlite3.connect(chemin)
    cx.execute("CREATE TABLE exercices (id INTEGER PRIMARY KEY)")
    cx.close()
    with pytest.raises(ErreurStockage, match="antérieure"):
        Depot.ouvrir(chemin)


def test_reouverture(tmp_path):
    chemin = tmp_path / "mdr.sqlite"
    d = Depot.ouvrir(chemin)
    _ajouter(d, 1)
    d.fermer()
    d = Depot.ouvrir(chemin)
    assert len(d.en_file()) == 1
    d.fermer()
