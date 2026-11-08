"""Indices par paliers : jamais la réponse, sur des centaines d'exercices de chaque famille."""

from __future__ import annotations

import json
import random
from pathlib import Path

import pytest

from mine_de_rien import photo
from mine_de_rien.exercises import registre
from mine_de_rien.exercises.anglais import vocabulaire
from mine_de_rien.exercises.base import Exercice
from mine_de_rien.exercises.expression import ErreurExpression, calculer
from mine_de_rien.exercises.indices import (
    LONGUEUR_MAX,
    SECOURS,
    _reperes,
    contient_la_reponse,
    en_lettres,
    indices,
)

GRAINES = range(500)


def _verifier(ex: Exercice) -> tuple[str, ...]:
    paliers = indices(ex)
    assert 1 <= len(paliers) <= 4, ex
    for palier in paliers:
        assert len(palier) <= LONGUEUR_MAX, palier
        assert not contient_la_reponse(palier, ex), (palier, ex)
    assert indices(ex) == paliers  # déterministe
    return paliers


@pytest.mark.parametrize("entree", registre.ENTREES, ids=lambda e: e.nom)
def test_generateurs_du_registre(entree):
    for graine in GRAINES:
        _verifier(entree.generer(random.Random(graine), 1))


def test_tout_le_lexique_dans_les_deux_sens():
    for p in vocabulaire.lexique():
        for sens in p.sens:
            paliers = _verifier(vocabulaire.exercice(p, sens))
            assert len(paliers) >= 2, p


def _calcul_au_hasard(rng: random.Random) -> Exercice | None:
    def nombre() -> str:
        if rng.random() < 0.2:
            return f"{rng.randint(0, 99)},{rng.randint(1, 99)}"
        return str(rng.choice([rng.randint(1, 9), rng.randint(10, 999), rng.randint(1000, 99999)]))

    try:
        return calculer(f"{nombre()} {rng.choice('+-×÷')} {nombre()}")
    except ErreurExpression:
        return None


def test_calculs_a_deux_nombres():
    vus = 0
    for graine in range(3000):
        ex = _calcul_au_hasard(random.Random(graine))
        if ex is not None:
            _verifier(ex)
            vus += 1
    assert vus > 500


def test_doubles_et_moities_de_1_a_2000_et_decimaux():
    textes = [f"{mot} de {n}" for mot in ("Double", "Moitié") for n in range(1, 2001)]
    textes += [f"Double de {n},{d}" for n in range(30) for d in range(1, 10)]
    textes += [f"Moitié de {n},{d}" for n in range(30) for d in (2, 4, 6, 8)]
    for texte in textes:
        assert len(_verifier(photo._calcul(texte))) >= 2, texte


def test_exercices_lus_sur_les_photos_enregistrees():
    dossier = Path(__file__).parent / "donnees" / "photo"
    for fichier in dossier.glob("*.json"):
        lecture = photo.interpreter(json.loads(fichier.read_text("utf-8")), random.Random(0))
        for ex in lecture.exercices:
            _verifier(ex)


def test_cas_ecrits_a_la_main():
    assert indices(photo._calcul("Double de 17")) == (
        "Le double de 17, c'est 17 ajouté à lui-même.",
        "Découpe : 17 = 10 + 7. Cherche le double de 10, puis le double de 7.",
        "Le double de 10 = 20 et le double de 7 = 14 : il reste à les ajouter.",
    )
    division = indices(calculer("348 ÷ 12"))
    assert division[0] == "Cherche combien de fois 12 entre dans 348."
    assert division[-1].startswith("Première étape : dans 34, 12 va 2 fois, il reste 10.")
    august = next(p for p in vocabulaire.lexique() if p.anglais == "august")
    assert indices(vocabulaire.exercice(august, vocabulaire.VERS_FRANCAIS)) == (
        "C'est un mois de l'année.",
        "Le mot français commence par « a » et a 4 lettres.",
        "C'est le 8e mois de l'année.",
    )


def test_priorites_pour_un_calcul_a_plusieurs_operations():
    (palier,) = indices(calculer("(2 + 3) × 4"))
    assert "parenthèses" in palier


def test_exercice_inconnu_donne_le_secours():
    assert indices(Exercice("francais/conjugaison", "Il (manger) ?", "mange")) == (SECOURS,)


def test_un_palier_qui_donnerait_la_reponse_est_retire():
    # 36 ÷ 6 = 6 : la liste des multiples de 6 commence par 6, elle est retirée.
    paliers = indices(calculer("36 ÷ 6"))
    assert not any("multiples" in p for p in paliers)


@pytest.mark.parametrize(
    "texte",
    ["c'est 34", "34,0 !", "34.00", "trente-quatre", "Trente quatre", "la réponse: 34."],
)
def test_reponse_numerique_reconnue(texte):
    assert contient_la_reponse(texte, Exercice("maths/x", "17 x 2 = ?", "34"))


@pytest.mark.parametrize("texte", ["341", "134", "3,4", "17 + 17", "trente", "quatre"])
def test_reponse_numerique_non_reconnue(texte):
    assert not contient_la_reponse(texte, Exercice("maths/x", "17 x 2 = ?", "34"))


def test_milliers_et_decimaux():
    assert contient_la_reponse("ça fait 13 545", Exercice("maths/x", "?", "13545"))
    assert contient_la_reponse("4.75", Exercice("maths/x", "?", "4,75"))
    assert not contient_la_reponse("4,7 et 5", Exercice("maths/x", "?", "4,75"))
    assert contient_la_reponse("vingt et un", Exercice("maths/x", "?", "21"))


def _mot(anglais: str, sens: str) -> Exercice:
    return vocabulaire.exercice(next(p for p in vocabulaire.lexique() if p.anglais == anglais), sens)


def test_reponse_en_mot():
    aout = _mot("august", vocabulaire.VERS_FRANCAIS)
    for texte in ("AOÛT", "c'est aout", "Août."):
        assert contient_la_reponse(texte, aout)
    assert not contient_la_reponse("le mois d'aoûtat", aout)
    nuit = _mot("good night", vocabulaire.VERS_FRANCAIS)
    assert contient_la_reponse("on dit Bonne Nuit", nuit)
    assert not contient_la_reponse("une bonne idée, la nuit", nuit)


def test_nombre_anglais_en_chiffres_donne_la_reponse():
    sept = _mot("seven", vocabulaire.VERS_FRANCAIS)
    assert contient_la_reponse("c'est 7", sept)
    assert contient_la_reponse("sept", sept)
    assert not contient_la_reponse("17", sept)


@pytest.mark.parametrize(
    "n, attendu",
    [(0, "zero"), (17, "dix-sept"), (21, "vingt-et-un"), (71, "soixante-et-onze"),
     (80, "quatre-vingts"), (99, "quatre-vingt-dix-neuf"), (200, "deux-cents"),
     (201, "deux-cent-un"), (1000, "mille"), (2026, "deux-mille-vingt-six")],
)
def test_en_lettres(n, attendu):
    assert en_lettres(n) == attendu


def test_reperes_relus_couvrent_des_mots_du_lexique():
    anglais = {(p.famille, p.anglais) for p in vocabulaire.lexique()}
    for famille, table in _reperes().items():
        for mot in table:
            assert (famille, mot) in anglais, mot
