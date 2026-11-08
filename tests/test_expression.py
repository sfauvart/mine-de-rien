import random
from fractions import Fraction

import pytest

from mine_de_rien.exercises.expression import ErreurExpression, calculer, ecrire_reponse


@pytest.mark.parametrize(
    "saisie, enonce, reponse",
    [
        ("348 ÷ 12", "348 ÷ 12 = ?", "29"),
        ("3,5 + 1,25", "3,5 + 1,25 = ?", "4,75"),
        ("(12 + 8) x 3", "(12 + 8) × 3 = ?", "60"),
        ("348:12", "348 ÷ 12 = ?", "29"),
        ("348 / 12 = ?", "348 ÷ 12 = ?", "29"),
        ("12*7 =", "12 × 7 = ?", "84"),
        ("3.5 + 1.5", "3,5 + 1,5 = ?", "5"),
        ("0,1 + 0,2", "0,1 + 0,2 = ?", "0,3"),
        ("7 - 7", "7 - 7 = ?", "0"),
        ("2 + 3 × 4", "2 + 3 × 4 = ?", "14"),
        ("(2 + 3) × 4", "(2 + 3) × 4 = ?", "20"),
        ("100 - 20 - 30", "100 - 20 - 30 = ?", "50"),
        ("100 ÷ 5 ÷ 2", "100 ÷ 5 ÷ 2 = ?", "10"),
        ("1 ÷ 4", "1 ÷ 4 = ?", "0,25"),
    ],
)
def test_calculs_acceptes(saisie, enonce, reponse):
    ex = calculer(saisie)
    assert ex.enonce == enonce
    assert ex.reponse == reponse


@pytest.mark.parametrize(
    "saisie, motif",
    [
        ("10 ÷ 3", "tombe pas juste"),
        ("1 ÷ 8", "tombe pas juste"),
        ("5 - 9", "négatif"),
        ("2 ** 8", "mal formée"),
        ("__import__('os')", "non reconnu"),
        ("2 ^ 8", "non reconnu"),
        ("3", "manque une opération"),
        ("(1 + 2", "non fermée"),
        ("1 + 2)", "mal formée"),
        ("1 ÷ 0", "zéro"),
        ("1 ÷ (2 - 2)", "zéro"),
        ("-5 + 9", "mal formée"),
        ("1 +", "incomplète"),
        ("", "vide"),
        ("1234567890 + 1", "trop grand"),
        ("1000+2000+3000+4000+5000+6000+7000+8000+9000", "panneau"),
    ],
)
def test_calculs_refuses(saisie, motif):
    with pytest.raises(ErreurExpression, match=motif):
        calculer(saisie)


def test_competence_selon_les_operations():
    assert calculer("348 ÷ 12").competence == "maths/division-posee"
    assert calculer("12 x 7").competence == "maths/multiplication-posee"
    assert calculer("3,5 + 1,25").competence == "maths/addition-posee"
    assert calculer("9 - 4").competence == "maths/soustraction-posee"
    assert calculer("(12 + 8) x 3").competence == "maths/calcul"


def test_ecriture_unique_de_la_reponse():
    assert ecrire_reponse(Fraction(7, 2)) == "3,5"
    assert ecrire_reponse(Fraction(350, 100)) == "3,5"
    assert ecrire_reponse(Fraction(301, 100)) == "3,01"
    assert ecrire_reponse(Fraction(30, 1)) == "30"


def test_propriete_le_calcul_est_exact():
    """Sur des opérations tirées au hasard, la réponse relue vaut le calcul exact."""
    for graine in range(300):
        rng = random.Random(graine)
        a = Fraction(rng.randint(0, 9999), rng.choice([1, 10, 100]))
        b = Fraction(rng.randint(1, 999), rng.choice([1, 10, 100]))
        op, attendu = rng.choice([("+", a + b), ("x", a * b), ("÷", a / b), ("-", a - b)])

        def ecrire(f: Fraction) -> str:
            return str(float(f)).replace(".", ",").removesuffix(",0")

        texte = f"{ecrire(a)} {op} {ecrire(b)}"
        try:
            ex = calculer(texte)
        except ErreurExpression:
            assert attendu < 0 or (attendu * 100).denominator != 1
            continue
        assert Fraction(ex.reponse.replace(",", ".")) == attendu
        assert "." not in ex.reponse and not ex.reponse.endswith("0") or "," not in ex.reponse
