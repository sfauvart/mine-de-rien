import random

from mine_de_rien.exercises.base import normaliser
from mine_de_rien.exercises.maths import division


def test_niveau_1_tombe_juste():
    for graine in range(200):
        ex = division.generer(random.Random(graine), niveau=1)
        d = ex.donnees
        assert d["dividende"] == d["diviseur"] * d["quotient"]
        assert ex.accepte(str(d["quotient"]))


def test_niveau_2_reste_coherent():
    for graine in range(200):
        ex = division.generer(random.Random(graine), niveau=2)
        d = ex.donnees
        assert 0 <= d["reste"] < d["diviseur"]
        assert d["dividende"] == d["diviseur"] * d["quotient"] + d["reste"]


def test_reponse_tolerante_aux_espaces_et_a_la_casse():
    assert normaliser(" 12 R 3 ") == normaliser("12r3")
