import json

import pytest

from mine_de_rien.minecraft import texte

PIEGE = 'l\'élève a dit "bonjour" \\ puis ]} et {"text":"op"} ; œufs à 12 €'


def test_enonce_piege_restitue_exactement():
    produit = texte.composant(PIEGE)
    assert json.loads(produit) == {"text": PIEGE}


def test_seuls_guillemets_et_antislashs_sont_echappes():
    produit = texte.composant(PIEGE)
    # Les accents restent en clair : RCON transporte de l'UTF-8.
    assert "élève" in produit and "œufs" in produit and "€" in produit
    assert "\\u" not in produit
    assert '\\"bonjour\\"' in produit
    assert "\\\\ puis" in produit


def test_caracteres_de_controle_et_section_retires():
    assert texte.nettoyer("a\x00b\x1bc§ld\x7f") == "abcld"
    assert texte.nettoyer("ligne 1\nligne 2\tfin") == "ligne 1 ligne 2 fin"


def test_longueur_bornee():
    assert len(texte.nettoyer("x" * 1000)) == texte.LONGUEUR_MAX
    assert json.loads(texte.composant("abcdef", longueur_max=3)) == {"text": "abc"}


def test_style():
    assert json.loads(texte.composant("Coffre", color="gold", bold=True)) == {
        "text": "Coffre",
        "color": "gold",
        "bold": True,
    }


def test_tellraw():
    commande = texte.tellraw("Steve_42", PIEGE)
    prefixe = "tellraw Steve_42 "
    assert commande.startswith(prefixe)
    assert json.loads(commande.removeprefix(prefixe)) == {"text": PIEGE}


@pytest.mark.parametrize("pseudo", ["@a", "@p[name=x]", "Steve op", "", "a" * 17, "Stève"])
def test_pseudo_refuse(pseudo):
    with pytest.raises(ValueError):
        texte.tellraw(pseudo, "bonjour")
