"""Écrit les fichiers de référence que la version Rust doit reproduire au caractère près.

Usage : uv run python scripts/oracle.py

Temporaire : vit le temps du passage à Rust (spec `2026-11-agent-en-rust`) et disparaît avec
le Python. Chaque fonction `oracle_<module>` rend une liste de cas `{"entree": …, "sortie": …}`
(ou `"erreur"` à la place de `"sortie"` quand le Python refuse), écrite dans
`tests/donnees/oracle/<module>.json`. Les fichiers produits restent : ce sont les tests de
non-régression du port.
"""

from __future__ import annotations

import dataclasses
import json
import os
import tempfile
from pathlib import Path

RACINE = Path(__file__).resolve().parents[1]
SORTIE = RACINE / "tests" / "donnees" / "oracle"


def oracle_config() -> list[dict]:
    from mine_de_rien.config import charger_config

    minimale = '[enfant]\npseudo_minecraft = "Steve"\n'
    entrees = [
        {"toml": (RACINE / "config.example.toml").read_text(encoding="utf-8"),
         "variables": {"RCON_PASSWORD": "secret", "TELEGRAM_BOT_TOKEN": "123:abc"}},
        {"toml": (RACINE / "config.example.toml").read_text(encoding="utf-8"), "variables": {}},
        {"toml": (RACINE / "config.essai.example.toml").read_text(encoding="utf-8"),
         "variables": {}},
        {"toml": minimale, "variables": {}},
        {"toml": minimale + '[canal]\nparents = [12, 34]\njeton = "a${B}c${B}"\n',
         "variables": {"B": "-"}},
        {"toml": minimale + '[llm]\nadaptateur = "openai-compatible"\nurl = "http://llm/v1"\n',
         "variables": {}},
        {"toml": minimale + '[assistant]\nnom = "Zoé"\nactif = false\n', "variables": {}},
        {"toml": minimale + '[assistant]\nnom = "@a tp"\n', "variables": {}},
        {"toml": minimale + '[assistant]\nnom = "P"\n', "variables": {}},
        {"toml": minimale + '[assistant]\nnom = "l\'ami"\n', "variables": {}},
        {"toml": minimale + '[paisible]\ndifficulte_normale = "peaceful"\n', "variables": {}},
        {"toml": minimale + '[paisible]\nactif = false\ndifficulte_normale = "hard"\n',
         "variables": {}},
        {"toml": minimale + '[coffre]\nrecompense = "Diamant"\n', "variables": {}},
        {"toml": minimale + '[coffre]\nrecompense = "emerald"\nrecompense_nombre = 64\n',
         "variables": {}},
        {"toml": minimale + "[coffre]\nrecompense_nombre = 65\n", "variables": {}},
        {"toml": minimale + "[coffre]\nniveaux_offerts = 31\n", "variables": {}},
        {"toml": minimale + "[atelier]\nx = 120\ny = 64\nz = -35\n", "variables": {}},
    ]
    cas = []
    anciennes = dict(os.environ)
    for entree in entrees:
        os.environ.clear()
        os.environ.update(entree["variables"])
        with tempfile.TemporaryDirectory() as dossier:
            fichier = Path(dossier) / "config.toml"
            fichier.write_text(entree["toml"], encoding="utf-8")
            try:
                cas.append({"entree": entree,
                            "sortie": dataclasses.asdict(charger_config(fichier))})
            except ValueError as e:
                cas.append({"entree": entree, "erreur": str(e)})
    os.environ.clear()
    os.environ.update(anciennes)
    return cas



def _tente(fonction, erreurs=(ValueError,)) -> dict:
    """`{"sortie": …}` ou `{"erreur": message}` selon que `fonction()` rend ou refuse."""
    try:
        return {"sortie": fonction()}
    except erreurs as e:
        return {"erreur": str(e)}


TEXTES = [
    "348 ÷ 12 = ?",
    'l\'élève a dit "bonjour" \\ puis ]} et {"text":"op"} ; œufs à 12 €',
    "a\x00b\x1bc§ld\x7f",
    "ligne 1\nligne 2\tfin\r",
    "x" * 300,
    "",
    "Combien font trois fois douze et demi ?",
    "anticonstitutionnellement",
    "Écris « château » en anglais, en lettres ?",
    "un deux trois quatre cinq six sept huit neuf dix onze douze treize quatorze",
    "aaaaaaaaaaaaaaa bbbbbbbbbbbbbbb ccccccccccccccc ddddddddddddddd",
    "aaaaaaaaaaaaaaa bbbbbbbbbbbbbbb ccccccccccccccc ddddddddddddddd e",
    "espace insécable et fine",
    "émoji 🎉 et é décomposé",
    "  bords   multiples  ",
    "\x85\x9f fin",
]


def oracle_texte() -> list[dict]:
    from mine_de_rien.exercises.base import normaliser
    from mine_de_rien.minecraft import texte

    cas = []
    for t in TEXTES:
        cas.append({"entree": {"f": "nettoyer", "texte": t}, "sortie": texte.nettoyer(t)})
        cas.append({"entree": {"f": "nettoyer", "texte": t, "max": 5},
                    "sortie": texte.nettoyer(t, 5)})
        cas.append({"entree": {"f": "composant", "texte": t, "style": []},
                    "sortie": texte.composant(t)})
        cas.append({"entree": {"f": "composant", "texte": t,
                               "style": [["color", "gold"], ["bold", True], ["italic", False]]},
                    "sortie": texte.composant(t, color="gold", bold=True, italic=False)})
        cas.append({"entree": {"f": "lignes_panneau", "texte": t},
                    "sortie": texte.lignes_panneau(t)})
        cas.append({"entree": {"f": "normaliser", "texte": t}, "sortie": normaliser(t)})
    for pseudo in ["Steve_42", "@a", "@p[name=x]", "Steve op", "", "a" * 16, "a" * 17, "Stève",
                   "l'ami", "abc"]:
        cas.append({"entree": {"f": "tellraw", "joueur": pseudo, "texte": TEXTES[1]},
                    **_tente(lambda p=pseudo: texte.tellraw(p, TEXTES[1], color="aqua"))})
    for t in [" 12 R 3 ", "12r3", "Château", "ŒUF", "ﬁn", "ça va", "É t é", "x²", "½"]:
        cas.append({"entree": {"f": "normaliser", "texte": t}, "sortie": normaliser(t)})
    return cas


EXPRESSIONS = [
    "348 ÷ 12", "3,5 + 1,25", "(12 + 8) x 3", "348:12", "348 / 12 = ?", "12*7 =", "3.5 + 1.5",
    "0,1 + 0,2", "7 - 7", "2 + 3 × 4", "(2 + 3) × 4", "100 - 20 - 30", "100 ÷ 5 ÷ 2", "1 ÷ 4",
    "10 ÷ 3", "1 ÷ 8", "5 - 9", "2 ** 8", "__import__('os')", "2 ^ 8", "3", "(1 + 2", "1 + 2)",
    "1 ÷ 0", "1 ÷ (2 - 2)", "-5 + 9", "1 +", "", "1234567890 + 1",
    "1000+2000+3000+4000+5000+6000+7000+8000+9000", "  12 X 7  ", "12 x 7 =?", "12 x 7 = ? ",
    "12 x 7 ?", "12 x 7 = ??", "= ?", "=", "12 x 7 = 84", "1,5 x 2", "1,50 + 1,50", "0,5 ÷ 0,25",
    "1 ÷ 3 × 3", "2 ÷ 3 + 1 ÷ 3", "((1 + 2) × (3 + 4))", "()", "(1)", "(1) + 2", "1 + (2", ")1 + 2(",
    "1 2 + 3", "1,2,3 + 1", "1, + 2", ",5 + 2", "1. + 2", "12 + 3.", "3 + 4 + ", "007 + 3",
    "999999999 + 1", "999999999 × 999999999", "0 ÷ 5", "5 ÷ 5", "12,34 + 0,66", "12,345 + 1",
    "0,005 + 0,005", "1,005 × 2", "100 - 99,99", "10 - 0,01", "3 × 0,1", "1 ÷ 100", "1 ÷ 1000",
    "25 × 4 ÷ 10", "8 ÷ 2 × 4", "8 - 2 + 4", "2 × (3 + 4) - 5", "2 x 3 X 4 * 5", "48 : 6 / 2",
    "12 + trois", "12 € + 3", "12 + 3 €", "douze", "1 + 2 = 3 = ?", "1 = 2", "1\t+\t2", "1\n+\n2",
    "1 + 2\n= ?", "１ + 2", "٣ + 1", "1 − 2", "5 – 3", "1 + 2;", "3,5+1,25", "(3,5+1,25)x2",
    "1+" + "1+" * 20 + "1", "123456789 + 123456789 + 123456789", "9 ÷ 0,5", "0 × 0", "0 + 0",
    "1,10 + 2,20", "200 ÷ 8", "7 ÷ 2", "1 ÷ 16", "0,25 × 0,2", "12 345 + 1", "1 000 + 1",
]


def _exercice(ex) -> dict:
    return dataclasses.asdict(ex)


def oracle_expression() -> list[dict]:
    from mine_de_rien.exercises.expression import calculer

    return [{"entree": e, **_tente(lambda e=e: _exercice(calculer(e)))} for e in EXPRESSIONS]


ORACLES = {
    "config": oracle_config,
    "texte": oracle_texte,
    "expression": oracle_expression,
}


def main() -> None:
    SORTIE.mkdir(parents=True, exist_ok=True)
    for nom, fabrique in ORACLES.items():
        cas = fabrique()
        chemin = SORTIE / f"{nom}.json"
        chemin.write_text(
            json.dumps(cas, ensure_ascii=False, indent=1, sort_keys=True) + "\n", encoding="utf-8"
        )
        print(f"{chemin.relative_to(RACINE)} : {len(cas)} cas")


if __name__ == "__main__":
    main()
