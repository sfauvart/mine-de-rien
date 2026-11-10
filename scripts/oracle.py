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



def oracle_vocabulaire() -> list[dict]:
    from mine_de_rien.exercises.anglais import vocabulaire as v

    cas = []
    for p in v.lexique():
        cas.append({
            "entree": {"anglais": p.anglais},
            "sortie": {
                "francais": p.francais, "famille": p.famille, "sens": list(p.sens),
                "accent": p.accent,
                "exercices": {s: _exercice(v.exercice(p, s)) for s in p.sens},
            },
        })
    return cas


def _exercices_pour_indices() -> list:
    """Un échantillon fixe de chaque famille d'exercices, tiré par le Python."""
    import random

    from mine_de_rien import photo
    from mine_de_rien.exercises import registre
    from mine_de_rien.exercises.anglais import vocabulaire
    from mine_de_rien.exercises.base import Exercice
    from mine_de_rien.exercises.expression import ErreurExpression, calculer

    exercices = [vocabulaire.exercice(p, s) for p in vocabulaire.lexique() for s in p.sens]
    for entree in registre.ENTREES:
        exercices += [entree.generer(random.Random(g), 1) for g in range(60)]
    from mine_de_rien.exercises.maths import division
    exercices += [division.generer(random.Random(g), 2) for g in range(40)]

    def nombre(rng) -> str:
        if rng.random() < 0.2:
            return f"{rng.randint(0, 99)},{rng.randint(1, 99)}"
        return str(rng.choice([rng.randint(1, 9), rng.randint(10, 999), rng.randint(1000, 99999)]))

    for graine in range(1500):
        rng = random.Random(graine)
        try:
            exercices.append(calculer(f"{nombre(rng)} {rng.choice('+-×÷')} {nombre(rng)}"))
        except ErreurExpression:
            pass
    for e in EXPRESSIONS:
        try:
            exercices.append(calculer(e))
        except ErreurExpression:
            pass
    textes = [f"{mot} de {n}" for mot in ("Double", "Moitié")
              for n in list(range(1, 131)) + list(range(190, 2001, 37)) + [1000, 1200, 2000]]
    textes += [f"Double de {n},{d}" for n in range(0, 30, 3) for d in range(1, 10, 2)]
    textes += [f"Moitié de {n},{d}" for n in range(0, 30, 3) for d in (2, 4, 6, 8)]
    textes += ["Moitié de 7", "Moitié de 0,5", "Double de 0,05", "Moitié de 1,01"]
    for t in textes:
        try:
            exercices.append(photo._calcul(t))
        except ErreurExpression:
            pass
    exercices += [
        Exercice("francais/conjugaison", "Il (manger) ?", "mange"),
        Exercice("maths/division-posee", "17 ÷ 5 : quotient et reste ?", "3 r 2",
                 {"dividende": 17, "diviseur": 5, "quotient": 3, "reste": 2}),
        Exercice("maths/division-posee", "? ", "6", {"dividende": "36", "diviseur": "6"}),
        Exercice("maths/division-posee", "? ", "6", {"dividende": 36, "diviseur": 0}),
        Exercice("maths/doubles-moities", "?", "x", {"famille": "doubles", "nombre": "abc"}),
        Exercice("anglais/vocabulaire", "?", "lundi", {"famille": "jours", "anglais": "nope"}),
        Exercice("anglais/vocabulaire", "?", "bleu", {"famille": "couleurs"}),
        Exercice("anglais/vocabulaire", "?", "château fort", {}),
    ]
    return exercices


def oracle_indices() -> list[dict]:
    from mine_de_rien.exercises.anglais import vocabulaire
    from mine_de_rien.exercises.base import Exercice
    from mine_de_rien.exercises.indices import contient_la_reponse, en_lettres, indices

    cas = [{"entree": {"f": "indices", "exercice": _exercice(ex)}, "sortie": list(indices(ex))}
           for ex in _exercices_pour_indices()]
    for n in list(range(130)) + [199, 200, 201, 280, 300, 380, 999, 1000, 1001, 1080, 1999,
                                    2000, 2026, 10000, 80000, 80080, 100000, 200000, 999999]:
        cas.append({"entree": {"f": "en_lettres", "n": n}, "sortie": en_lettres(n)})

    def mot(anglais: str, sens: str) -> Exercice:
        return vocabulaire.exercice(
            next(p for p in vocabulaire.lexique() if p.anglais == anglais), sens)

    cibles = [
        Exercice("maths/x", "17 x 2 = ?", "34"), Exercice("maths/x", "?", "13545"),
        Exercice("maths/x", "?", "4,75"), Exercice("maths/x", "?", "21"),
        Exercice("maths/x", "?", "0"), Exercice("maths/x", "?", "1000000"),
        Exercice("maths/x", "?", "80"), Exercice("maths/x", "?", "12 r 3"),
        mot("august", "vers-francais"), mot("good night", "vers-francais"),
        mot("seven", "vers-francais"), mot("seventeen", "vers-francais"),
        mot("wednesday", "vers-anglais"), mot("twenty", "vers-francais"),
    ]
    textes = [
        "c'est 34", "34,0 !", "34.00", "trente-quatre", "Trente quatre", "la réponse: 34.", "341",
        "134", "3,4", "17 + 17", "trente", "quatre", "ça fait 13 545", "13 545", "13 545",
        "4.75", "4,7 et 5", "4,75", "vingt et un", "vingt-et-un", "vingt un", "AOÛT",
        "c'est aout", "Août.", "le mois d'aoûtat", "on dit Bonne Nuit", "une bonne idée, la nuit",
        "c'est 7", "sept", "17", "dix-sept", "dix sept", "DIX-SEPT !", "zéro", "0", "0,0",
        "un million", "1 000 000", "quatre-vingts", "quatre vingt", "80", "12 r 3", "12 R 3",
        "12r3", "Wednesday", "wednesdays", "mercredi", "vingt", "20", "", "  ", "1 234 567",
        "le 12 345 et 678", "034", "34e", "x34", "3 4",
    ]
    for cible in cibles:
        for t in textes:
            cas.append({"entree": {"f": "contient", "texte": t, "exercice": _exercice(cible)},
                        "sortie": contient_la_reponse(t, cible)})
    return cas


CLES = ["29", "4,75", "mercredi", "août", "dix-sept", "good night", "bon après-midi", "Mercredi",
        " lundi", "lundi ", "good  night", "3.5", "3,50", "a" * 50, "a" * 51, "", "0", "00", "007",
        "0,5", "0,05", "0,50", "3,", ",5", "3,005", "1/2", "1e3", "-5", "+5", "1_000", " 12", "12 ",
        "٣", "é", "œuf", "ÿ", "÷", "a-b", "a--b", "-a", "a-", "a b c", "12 r 3", "3,5", "10", "1,10",
        "100,01"]

SAISIES = EXPRESSIONS + ["division 0", "division 11", "division 500", "fractions 2",
                         "division 3 fois", "division3", "division x", "anglais 99999999999999999999",
                         "  ", "é", "division -1", "anglais 0"]


def oracle_saisie() -> list[dict]:
    from mine_de_rien.saisie import ErreurSaisie, cle_valide, interpreter

    cas = [{"entree": {"f": "cle_valide", "cle": c}, "sortie": cle_valide(c)} for c in CLES]

    def lire(texte: str) -> dict:
        s = interpreter(texte)
        return {"source": s.source, "exercices": [_exercice(ex) for ex in s.exercices]}

    for s in SAISIES:
        cas.append({"entree": {"f": "interpreter", "texte": s},
                    **_tente(lambda s=s: lire(s), (ErreurSaisie,))})
    for s in ["division", "  Division ", "division 3", "DIVISION 10", "anglais", "Anglais 4",
              "anglais 10", "division\t2"]:
        saisie = interpreter(s)
        cas.append({"entree": {"f": "competence", "texte": s},
                    "sortie": {"source": saisie.source, "nombre": len(saisie.exercices)}})
    return cas


ORACLES = {
    "config": oracle_config,
    "texte": oracle_texte,
    "expression": oracle_expression,
    "vocabulaire": oracle_vocabulaire,
    "indices": oracle_indices,
    "saisie": oracle_saisie,
}


def main() -> None:
    SORTIE.mkdir(parents=True, exist_ok=True)
    for nom, fabrique in ORACLES.items():
        cas = fabrique()
        chemin = SORTIE / f"{nom}.json"
        # Un cas par ligne : fichiers compacts, et un diff git qui montre le cas qui bouge.
        lignes = ",\n".join(json.dumps(c, ensure_ascii=False, sort_keys=True) for c in cas)
        chemin.write_text(f"[\n{lignes}\n]\n", encoding="utf-8")
        print(f"{chemin.relative_to(RACINE)} : {len(cas)} cas")


if __name__ == "__main__":
    main()
