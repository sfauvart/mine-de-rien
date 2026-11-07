import random

import pytest

from mine_de_rien.saisie import NOMBRE_MAX, ErreurSaisie, interpreter


def test_expression():
    s = interpreter("348 ÷ 12")
    assert s.source == "expression"
    (ex,) = s.exercices
    assert (ex.enonce, ex.reponse) == ("348 ÷ 12 = ?", "29")


def test_competence_trois_exercices_differents_et_verifies():
    s = interpreter("division 3", random.Random(1))
    assert s.source == "competence"
    assert len(s.exercices) == 3
    assert len({ex.enonce for ex in s.exercices}) == 3
    for ex in s.exercices:
        d = ex.donnees
        assert d["dividende"] == d["diviseur"] * d["quotient"]
        assert ex.reponse == str(d["quotient"])


def test_competence_sans_nombre_et_casse():
    s = interpreter("  Division ", random.Random(2))
    assert len(s.exercices) == 1


def test_propriete_competence_toujours_utilisable():
    for graine in range(200):
        s = interpreter(f"division {NOMBRE_MAX}", random.Random(graine))
        assert len({ex.enonce for ex in s.exercices}) == NOMBRE_MAX


def test_meme_graine_memes_exercices():
    a = interpreter("division 4", random.Random(7)).exercices
    b = interpreter("division 4", random.Random(7)).exercices
    assert a == b


@pytest.mark.parametrize("texte", ["division 0", "division 11", "division 500"])
def test_nombre_borne(texte):
    with pytest.raises(ErreurSaisie, match="Entre 1 et 10"):
        interpreter(texte)


def test_competence_inconnue_liste_les_disponibles():
    with pytest.raises(ErreurSaisie, match="division"):
        interpreter("fractions 2")


@pytest.mark.parametrize("texte", ["10 ÷ 3", "5 - 9", "2 ** 8", "__import__", ""])
def test_refus_avec_message(texte):
    with pytest.raises(ErreurSaisie) as e:
        interpreter(texte)
    assert str(e.value)
