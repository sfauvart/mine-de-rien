"""Lecture d'une photo de fiche : réponses du LLM enregistrées le 2026-11-08, jamais les photos.

Les photos montrent le prénom de l'enfant : elles n'entrent pas dans le dépôt. Les fichiers de
`tests/donnees/photo/` sont les réponses JSON brutes du LLM local (Gemma 4 E4B, llama.cpp),
une par morceau d'image, sur des photos réduites à 1280 px comme le fait Telegram.
"""

from __future__ import annotations

import asyncio
import io
import json
import random
from fractions import Fraction
from pathlib import Path

import pytest
from PIL import Image

from mine_de_rien import photo
from mine_de_rien.adapters.llm.base import ErreurLlm
from mine_de_rien.saisie import cle_valide

DONNEES = Path(__file__).parent / "donnees" / "photo"


def _enregistre(nom: str) -> list[dict]:
    return json.loads((DONNEES / f"{nom}.json").read_text("utf-8"))


def _lecture(lignes: list[tuple[str, str]]) -> photo.Lecture:
    reponse = {"lignes": [{"genre": g, "texte": t} for g, t in lignes]}
    return photo.interpreter([reponse], random.Random(0))


def _image(largeur: int, hauteur: int) -> bytes:
    tampon = io.BytesIO()
    Image.new("RGB", (largeur, hauteur), "white").save(tampon, format="JPEG")
    return tampon.getvalue()


class FauxLlm:
    def __init__(self, reponses: list[dict], distant: bool = False) -> None:
        self.reponses = list(reponses)
        self.distant = distant
        self.appels: list[tuple[str, str, bytes | None, bool]] = []

    async def completer(self, systeme, utilisateur, image=None, en_json=False):
        self.appels.append((systeme, utilisateur, image, en_json))
        return json.dumps(self.reponses.pop(0))


def _valeur(reponse: str) -> Fraction:
    return Fraction(reponse.replace(",", "."))


def test_fiche_doubles_moities_redressee():
    lecture = photo.interpreter(_enregistre("doubles-moities"), random.Random(0))
    doubles = [ex for ex in lecture.exercices if ex.donnees["famille"] == "doubles"]
    moities = [ex for ex in lecture.exercices if ex.donnees["famille"] == "moities"]
    assert len(doubles) == 37 and len(moities) == 35  # 72 des 73 lignes, aucune inventée
    for ex in doubles + moities:
        n = _valeur(ex.donnees["nombre"])
        attendu = n * 2 if ex.donnees["famille"] == "doubles" else n / 2
        assert _valeur(ex.reponse) == attendu, ex
        assert cle_valide(ex.reponse)
        assert ex.competence == photo.COMPETENCE_DOUBLES
    assert {ex.enonce for ex in doubles} >= {"Double de 17 = ?", "Double de 1000 = ?"}
    assert "Moitié de 1200 = ?" in {ex.enonce for ex in moities}


def test_fiche_anglais_exercice_7():
    lecture = photo.interpreter(_enregistre("anglais-exercice-7"), random.Random(0))
    anglais = [ex.donnees["anglais"] for ex in lecture.exercices]
    assert anglais[:3] == ["wednesday", "saturday", "monday"]  # ordre de la fiche
    assert len(anglais) == 19
    assert {ex.donnees["famille"] for ex in lecture.exercices} == {"jours", "mois"}


def test_problemes_ne_donnent_aucun_exercice_faux():
    lecture = photo.interpreter(_enregistre("problemes"), random.Random(0))
    assert {(ex.enonce, ex.reponse) for ex in lecture.exercices} <= {
        ("799 - 760 = ?", "39"),
        ("75,5 - 50 = ?", "25,5"),
    }


def test_photo_de_travers_reste_juste_mais_infidele():
    """Limite connue : de travers, le modèle invente des suites. Python ne peut pas les écarter
    (ce sont des calculs justes) ; les réponses restent exactes, le parent juge la liste."""
    lecture = photo.interpreter(_enregistre("doubles-moities-de-travers"), random.Random(0))
    for ex in lecture.exercices:
        n = _valeur(ex.donnees["nombre"])
        attendu = n * 2 if ex.donnees["famille"] == "doubles" else n / 2
        assert _valeur(ex.reponse) == attendu


def test_resultat_du_llm_jamais_lu():
    (ex,) = _lecture([("calcul", "Double de 17 = 35")]).exercices
    assert ex.reponse == "34"
    (ex,) = _lecture([("calcul", "348 ÷ 12 = 30")]).exercices
    assert ex.reponse == "29"


def test_doubles_et_moities():
    (ex,) = _lecture([("calcul", "Moitié de 3")]).exercices
    assert (ex.enonce, ex.reponse) == ("Moitié de 3 = ?", "1,5")
    (ex,) = _lecture([("calcul", "moitie  de 1 000")]).exercices
    assert (ex.enonce, ex.reponse) == ("Moitié de 1000 = ?", "500")
    (ex,) = _lecture([("calcul", "Double de 2,5")]).exercices
    assert ex.reponse == "5"


def test_calculs_rejetes():
    lecture = _lecture([("calcul", "Moitié de 1,25"), ("calcul", "10 ÷ 3"), ("calcul", "799")])
    assert lecture.exercices == []
    assert [r.texte for r in lecture.rejets] == ["Moitié de 1,25", "10 ÷ 3", "799"]


def test_tiret_typographique_et_milliers():
    (ex,) = _lecture([("calcul", "799 – 760")]).exercices
    assert ex.reponse == "39"
    (ex,) = _lecture([("calcul", "1 200 + 12 345")]).exercices
    assert ex.reponse == "13545"


def test_mots():
    lecture = _lecture([("mot", "Wednesday"), ("mot", "book"), ("mot", "orange")])
    assert [ex.donnees["anglais"] for ex in lecture.exercices] == ["wednesday"]
    assert [r.raison for r in lecture.rejets] == [photo.HORS_LEXIQUE, photo.HORS_LEXIQUE]


def test_doublons_retires():
    lecture = _lecture([("calcul", "Double de 20"), ("calcul", "Double de 20 = 40"),
                        ("mot", "May"), ("mot", "may")])
    assert len(lecture.exercices) == 2


def test_lignes_mal_formees():
    lecture = photo.interpreter([{"lignes": ["texte nu", {"genre": "mot"}]}], random.Random(0))
    assert lecture.exercices == [] and len(lecture.rejets) == 2
    with pytest.raises(ErreurLlm):
        photo.interpreter([{"autre": []}], random.Random(0))


def test_paysage_deux_appels_portrait_un_seul():
    assert len(photo.preparer(_image(1280, 885))) == 2
    assert len(photo.preparer(_image(885, 1280))) == 1
    grand = photo.preparer(_image(4000, 3000))
    assert all(max(Image.open(io.BytesIO(m)).size) <= photo.COTE_MAX for m in grand)


def test_image_illisible():
    with pytest.raises(photo.ErreurPhoto):
        photo.preparer(b"pas une image")


def test_lire_appelle_le_llm_par_moitie_avec_la_consigne_du_parent():
    llm = FauxLlm([{"lignes": [{"genre": "calcul", "texte": "Double de 1"}]},
                   {"lignes": [{"genre": "calcul", "texte": "Moitié de 2"}]}])
    lecture = asyncio.run(photo.lire(llm, _image(1280, 885), "exercice 7", random.Random(0)))
    assert len(llm.appels) == 2
    assert all(en_json and image for _, _, image, en_json in llm.appels)
    assert "exercice 7" in llm.appels[0][1]
    assert len(lecture.exercices) == 2


def test_lire_refuse_un_llm_distant_sans_l_appeler():
    llm = FauxLlm([], distant=True)
    with pytest.raises(ErreurLlm, match="local"):
        asyncio.run(photo.lire(llm, _image(100, 100)))
    assert llm.appels == []


@pytest.mark.parametrize("n", [0, 3, 72, 500])
def test_tirage_sans_remise(n):
    lecture = photo.interpreter(_enregistre("doubles-moities"), random.Random(0))
    tires = photo.tirer(lecture, n, random.Random(n))
    assert len(tires) == min(n, 72)
    assert len({ex.enonce for ex in tires}) == len(tires)


def test_resume_montre_les_calculs_ecartes_et_compte_les_mots():
    lecture = _lecture([("calcul", "Double de 4"), ("calcul", "10 ÷ 3"), ("mot", "Greetings")])
    texte = "\n".join(photo.resumer(lecture))
    assert "Doubles de (1) : 4" in texte
    assert "« 10 ÷ 3 »" in texte
    assert "Greetings" not in texte and "1 texte(s) hors lexique" in texte
