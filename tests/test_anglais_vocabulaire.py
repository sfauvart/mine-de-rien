"""Générateur de vocabulaire anglais : propriétés sur des centaines de graines."""

from __future__ import annotations

import random

import pytest

from mine_de_rien.exercises.anglais import vocabulaire as v
from mine_de_rien.exercises.base import normaliser
from mine_de_rien.minecraft.texte import lignes_panneau
from mine_de_rien.saisie import Saisie, cle_valide, interpreter

GRAINES = range(500)


def _par_anglais(mot: str) -> v.Paire:
    (p,) = [p for p in v.lexique() if p.anglais == mot]
    return p


def test_lexique_charge_les_cinq_familles():
    familles = {p.famille for p in v.lexique()}
    assert familles == {"jours", "mois", "nombres", "couleurs", "salutations"}
    assert len([p for p in v.lexique() if p.famille == "jours"]) == 7
    assert len([p for p in v.lexique() if p.famille == "mois"]) == 12
    assert len([p for p in v.lexique() if p.famille == "nombres"]) == 21


@pytest.mark.parametrize("graine", GRAINES)
def test_proprietes(graine):
    ex = v.generer(random.Random(graine))
    assert ex.competence == v.competence
    assert lignes_panneau(ex.enonce) is not None, ex.enonce
    assert cle_valide(ex.reponse), ex.reponse
    assert ex.reponse == ex.reponse.lower()
    p = _par_anglais(ex.donnees["anglais"])
    attendu = p.anglais if ex.donnees["sens"] == v.VERS_ANGLAIS else p.francais
    assert ex.reponse == attendu
    assert ex.accepte(ex.reponse)
    assert not ex.accepte(ex.reponse + "s")
    assert v.generer(random.Random(graine)) == ex


def test_bonjour_jamais_demande_vers_l_anglais():
    for mot in ("hello", "good morning"):
        assert _par_anglais(mot).sens == (v.VERS_FRANCAIS,)
    for graine in GRAINES:
        ex = v.generer(random.Random(graine))
        assert not ex.enonce.startswith("bonjour")


def test_mots_identiques_jamais_tires():
    assert _par_anglais("orange").sens == ()
    assert _par_anglais("six").sens == ()
    tires = {v.generer(random.Random(g)).donnees["anglais"] for g in GRAINES}
    assert "orange" not in tires and "six" not in tires


def test_goodbye_seulement_vers_le_francais():
    assert _par_anglais("goodbye").sens == (v.VERS_FRANCAIS,)


def test_deux_sens_et_cinq_familles_tires():
    exercices = [v.generer(random.Random(g)) for g in GRAINES]
    assert {ex.donnees["sens"] for ex in exercices} == {v.VERS_ANGLAIS, v.VERS_FRANCAIS}
    assert len({ex.donnees["famille"] for ex in exercices}) == 5


def test_enonces():
    mercredi = _par_anglais("wednesday")
    assert v.exercice(mercredi, v.VERS_ANGLAIS).enonce == "mercredi en anglais ?"
    assert v.exercice(mercredi, v.VERS_FRANCAIS).enonce == "Wednesday en français ?"
    dix_sept = v.exercice(_par_anglais("seventeen"), v.VERS_FRANCAIS)
    assert dix_sept.enonce == "seventeen en français, en lettres ?"
    assert dix_sept.reponse == "dix-sept"
    assert v.exercice(_par_anglais("good night"), v.VERS_ANGLAIS).reponse == "good night"


def test_repli_sans_accents():
    entrees = [("mois", "august", "août", True), ("mois", "may", "mai", True)]
    aout, mai = v.construire(entrees, accents=False)
    assert aout.sens == (v.VERS_ANGLAIS,)
    assert mai.sens == (v.VERS_ANGLAIS, v.VERS_FRANCAIS)
    aout, _ = v.construire(entrees, accents=True)
    assert aout.sens == (v.VERS_ANGLAIS, v.VERS_FRANCAIS)


def test_majuscule_refusee_dans_le_lexique():
    with pytest.raises(ValueError):
        v.construire([("jours", "Monday", "lundi", True)], accents=True)


def test_exercice_pour_un_mot_lu():
    rng = random.Random(1)
    ex = v.exercice_pour("Wednesday", rng)
    assert ex is not None and normaliser(ex.donnees["francais"]) == "mercredi"
    assert v.exercice_pour("GOOD NIGHT", rng) is not None
    assert v.exercice_pour("février", rng).donnees["anglais"] == "february"
    assert v.exercice_pour("bonjour", rng) is None  # ambigu
    assert v.exercice_pour("orange", rng) is None  # aucun sens autorisé
    assert v.exercice_pour("book", rng) is None  # hors lexique


def test_cle_valide():
    for bonne in ("29", "4,75", "mercredi", "août", "dix-sept", "good night", "bon après-midi"):
        assert cle_valide(bonne), bonne
    for mauvaise in ("Mercredi", " lundi", "lundi ", "good  night", "3.5", "3,50", "a" * 51, ""):
        assert not cle_valide(mauvaise), mauvaise


def test_coffre_anglais_3():
    saisie = interpreter("anglais 3", random.Random(4))
    assert isinstance(saisie, Saisie) and saisie.source == "competence"
    assert len({ex.enonce for ex in saisie.exercices}) == 3


def test_exemples_pour_le_parent():
    for graine in (1, 2, 3):
        ex = v.generer(random.Random(graine))
        print(f"{ex.enonce} → {ex.reponse}")
