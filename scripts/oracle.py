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
import sys
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



BASES = RACINE / "tests" / "donnees" / "bases"
MAINTENANT = "2026-11-14T18:00:00+00:00"


def _quand(jours: float = 0, minutes: float = 0, secondes: int = 0) -> str:
    from datetime import datetime, timedelta

    base = datetime.fromisoformat(MAINTENANT)
    instant = base - timedelta(days=jours) + timedelta(minutes=minutes, seconds=secondes)
    return instant.isoformat(timespec="seconds")


def _ecrire_bases() -> None:
    """Trois bases écrites par le Python, une par version du schéma, avec les mêmes coffres."""
    import sqlite3

    from mine_de_rien.storage.sqlite import MIGRATIONS, SCHEMA

    division, addition = "maths/division-posee", "maths/addition-posee"
    anglais = json.dumps(
        {"anglais": "august", "francais": "août", "famille": "mois", "sens": "vers-anglais"},
        ensure_ascii=False)
    # competence, enonce, reponse, donnees, source, statut, demande_par, cree, pose, ouvert
    lignes = [
        (division, "ancien", "1", "{}", "competence", "ouvert", "111",
         _quand(9, -1), _quand(9), _quand(9, 90)),
        (division, "72 ÷ 6 = ?", "12", '{"dividende": 72, "diviseur": 6, "quotient": 12, "reste": 0}',
         "competence", "ouvert", "111", _quand(4, -1), _quand(4), _quand(4, 10)),
        (division, "84 ÷ 7 = ?", "12", '{"expression": "84 ÷ 7"}', "expression", "ouvert", "111",
         _quand(3, -1), _quand(3), _quand(3, 45)),
        (division, "96 ÷ 3 = ?", "32", '{"expression": "96 ÷ 3"}', "expression", "ouvert", "cli",
         _quand(2, -1), _quand(2), _quand(2, 4)),
        (addition, "annulé", "1", "{}", "expression", "annule", "111",
         _quand(2, -1), _quand(2, 30), None),
        ("anglais/vocabulaire", "août en anglais ?", "august", anglais, "competence", "ouvert",
         "222", _quand(2, 50), _quand(2, 60), _quand(2, 100, 30)),
        (addition, "3,5 + 1,25 = ?", "4,75", '{"expression": "3,5 + 1,25"}', "expression", "pose",
         "111", _quand(3), _quand(0, -50 * 60), None),
        ("maths/calcul", "(12 + 8) × 3 = ?", "60", '{"expression": "(12 + 8) × 3"}', "expression",
         "en_file", "111", _quand(0, -30), None, None),
        ("anglais/vocabulaire", "Wednesday en français ?", "mercredi",
         '{"anglais": "wednesday", "francais": "mercredi", "famille": "jours", "sens": "vers-francais"}',
         "competence", "en_file", "222", _quand(0, -20), None, None),
    ]
    evenements = [(2, "indice", _quand(4, 2)), (2, "indice", _quand(4, 5)),
                  (4, "indice", _quand(2, 1)), (4, "indice", _quand(2, 2)),
                  (4, "indice", _quand(2, 3)), (6, "indice", _quand(2, 70)),
                  (6, "indice", _quand(2, 75)), (6, "indice", _quand(2, 80)),
                  (6, "indice", _quand(2, 85)), (7, "coffre_disparu", _quand(1)),
                  (None, "demarrage", _quand(1, 5)), (1, "indice", _quand(9, 10))]
    BASES.mkdir(parents=True, exist_ok=True)
    for version in (1, 2, 3):
        chemin = BASES / f"v{version}.sqlite"
        chemin.unlink(missing_ok=True)
        cx = sqlite3.connect(chemin)
        cx.executescript(SCHEMA)
        cx.executemany(
            """INSERT INTO exercices (competence, enonce, reponse, donnees, jeu, source, statut,
                   demande_par, recompense, recompense_nombre, cree_le, pose_le, ouvert_le)
               VALUES (?, ?, ?, ?, 'coffre-du-soir', ?, ?, ?, 'minecraft:diamond', 3, ?, ?, ?)""",
            lignes)
        if version >= 2:
            cx.executescript(MIGRATIONS[0])
            cx.execute("UPDATE exercices SET niveaux_donnes = 1 WHERE pose_le IS NOT NULL")
            cx.executemany("INSERT INTO evenements (exercice_id, genre, quand) VALUES (?, ?, ?)",
                           evenements)
        if version >= 3:
            cx.executescript(MIGRATIONS[1])
            cx.execute(
                """INSERT INTO exercices (competence, enonce, reponse, donnees, jeu, source,
                       statut, demande_par, recompense, recompense_nombre, cree_le)
                   VALUES ('maths/doubles-moities', 'Double de 17 = ?', '34',
                           '{"famille": "doubles", "nombre": "17"}', 'coffre-du-soir', 'photo',
                           'en_file', '111', 'minecraft:emerald', 5, ?)""", (_quand(0, -10),))
        cx.execute(f"PRAGMA user_version = {version}")
        cx.commit()
        cx.close()


def _stocke(s) -> dict | None:
    if s is None:
        return None
    d = dataclasses.asdict(s)
    for cle in ("cree_le", "pose_le", "ouvert_le"):
        d[cle] = d[cle].isoformat(timespec="seconds") if d[cle] else None
    d["duree"] = s.duree.total_seconds() if s.duree is not None else None
    return d


def _copie(version: int, dossier: str):
    import shutil

    from mine_de_rien.storage.sqlite import Depot

    copie = Path(dossier) / "copie.sqlite"
    shutil.copy(BASES / f"v{version}.sqlite", copie)
    return Depot.ouvrir(copie)


def oracle_stockage() -> list[dict]:
    from datetime import datetime

    _ecrire_bases()
    debut, fin = datetime.fromisoformat("2000-01-01T00:00:00+00:00"), datetime.fromisoformat(
        "2100-01-01T00:00:00+00:00")
    cas = []
    for version in (1, 2, 3):
        with tempfile.TemporaryDirectory() as dossier:
            depot = _copie(version, dossier)
            tous = depot.entre(debut, fin)
            cas.append({"entree": {"base": f"v{version}"}, "sortie": {
                "exercices": [_stocke(s) for s in tous],
                "en_file": [s.id for s in depot.en_file()],
                "suivant": _stocke(depot.suivant()),
                "actif": _stocke(depot.actif()),
                "dernier_ouvert": _stocke(depot.dernier_ouvert()),
                "poses": [s.id for s in depot.poses_entre(
                    datetime.fromisoformat(_quand(7)), datetime.fromisoformat(MAINTENANT))],
                "evenements": [
                    {"exercice_id": e.exercice_id, "genre": e.genre,
                     "quand": e.quand.isoformat(timespec="seconds")}
                    for e in depot.evenements_entre(debut, fin)],
                "indices": {str(s.id): depot.compter("indice", s.id) for s in tous},
            }})
            depot.fermer()
    return cas


def oracle_rapport() -> list[dict]:
    from datetime import datetime, timedelta

    from mine_de_rien.games.coffre_du_soir import formater_duree
    from mine_de_rien.rapport import a_retravailler, nombre_indices, rapport

    cas = []
    for version in (1, 2, 3):
        for jours in (0, 3, -5, 30):
            quand = _quand(-jours)
            with tempfile.TemporaryDirectory() as dossier:
                depot = _copie(version, dossier)
                instant = datetime.fromisoformat(quand)
                cas.append({"entree": {"f": "rapport", "base": f"v{version}", "quand": quand},
                            "sortie": rapport(depot, instant)})
                if version == 3:
                    for s in depot.entre(instant - timedelta(days=400), instant):
                        for indices in (0, 3):
                            cas.append({
                                "entree": {"f": "a_retravailler", "base": "v3", "quand": quand,
                                           "id": s.id, "indices": indices},
                                "sortie": a_retravailler(s, instant, indices)})
                depot.fermer()
    for secondes in [None, -30, 0, 59, 60, 61, 3599, 3600, 3661, 7200, 47 * 3600 + 59 * 60,
                     48 * 3600, 49 * 3600, 71 * 3600, 72 * 3600, 30 * 86400]:
        duree = None if secondes is None else timedelta(seconds=secondes)
        cas.append({"entree": {"f": "formater_duree", "secondes": secondes},
                    "sortie": formater_duree(duree)})
    for n in (0, 1, 2, 10):
        cas.append({"entree": {"f": "nombre_indices", "n": n}, "sortie": nombre_indices(n)})
    return cas



class Enregistreur:
    """Enveloppe un faux jeu des tests et note chaque échange, dans l'ordre.

    La version Rust rejoue la transcription : elle doit envoyer les mêmes commandes, dans le
    même ordre, et reçoit les mêmes réponses.
    """

    def __init__(self, jeu) -> None:
        self.jeu = jeu
        self.echanges: list[list] = []

    async def commande(self, commande: str) -> str:
        from mine_de_rien.adapters.game.rcon import ErreurRcon

        try:
            reponse = await self.jeu.commande(commande)
        except ErreurRcon as e:
            self.echanges.append(["panne", commande, str(e)])
            raise
        self.echanges.append(["commande", commande, reponse])
        return reponse

    async def dire(self, joueur: str, texte: str, **style) -> None:
        self.echanges.append(["dire", joueur, texte, style])
        await self.jeu.dire(joueur, texte, **style)


def _transcrire(entree: dict, jeu, action, erreurs) -> dict:
    """Joue `action(enregistreur)` et rend le cas d'oracle : échanges, puis sortie ou erreur."""
    import asyncio

    enregistreur = Enregistreur(jeu)
    cas = {"entree": entree}
    try:
        cas["sortie"] = asyncio.run(action(enregistreur))
    except erreurs as e:
        cas["erreur"] = str(e)
    cas["echanges"] = enregistreur.echanges
    return cas


def oracle_atelier() -> list[dict]:
    sys.path.insert(0, str(RACINE))
    from mine_de_rien.games import atelier
    from mine_de_rien.games.atelier import Atelier, ErreurAtelier
    from tests.test_atelier import FauxMonde

    construit = {
        (0, -60, 8): "minecraft:chest", (1, -60, 8): "minecraft:anvil",
        (-1, -60, 8): "minecraft:oak_sign", (2, -60, 6): "minecraft:lantern",
        **{(x, -61, z): "minecraft:smooth_stone" for x in range(-2, 3) for z in range(6, 11)},
    }
    mondes = {
        "libre": lambda: FauxMonde({}),
        "bloc qui gêne": lambda: FauxMonde({(1, -59, 9): "minecraft:oak_planks"}),
        "plancher construit": lambda: FauxMonde({(0, -61, 8): "minecraft:oak_planks"}),
        "coffre d'autrui": lambda: FauxMonde({(0, -60, 8): "minecraft:chest"}),
        "déjà construit": lambda: FauxMonde(dict(construit), pos_enregistree=(0, -60, 8)),
        "déjà construit, panneau retiré": lambda: FauxMonde(
            {k: v for k, v in construit.items() if v != "minecraft:oak_sign"},
            pos_enregistree=(0, -60, 8)),
        "atelier ailleurs": lambda: FauxMonde({}, pos_enregistree=(100, 64, 100)),
        "terre sous le plancher": lambda: FauxMonde({(0, -61, 8): "minecraft:dirt"}),
    }
    cas = []
    for nom, fabrique in mondes.items():
        for x, y, z in ((0, -60, 8), (-3, -60, -7)):
            a = Atelier(x, y, z)
            cas.append(_transcrire(
                {"f": "construire", "monde": nom, "atelier": [x, y, z]}, fabrique(),
                lambda jeu, a=a: atelier.construire(jeu, a), (ErreurAtelier,)))
        a = Atelier(0, -60, 8)

        async def lire(jeu):
            lu = await atelier.lire_atelier(jeu)
            return [lu.x, lu.y, lu.z] if lu else None

        async def libre(jeu, a=a):
            return await atelier.place_libre(jeu, a)

        async def demonter(jeu, a=a):
            await atelier.demonter(jeu, a)

        cas.append(_transcrire({"f": "lire_atelier", "monde": nom}, fabrique(), lire, ()))
        cas.append(_transcrire({"f": "place_libre", "monde": nom, "atelier": [0, -60, 8]},
                               fabrique(), libre, ()))
        cas.append(_transcrire({"f": "demonter", "monde": nom, "atelier": [0, -60, 8]},
                               fabrique(), demonter, (ErreurAtelier,)))
    return cas


def oracle_emplacement() -> list[dict]:
    sys.path.insert(0, str(RACINE))
    from mine_de_rien.games import emplacement
    from mine_de_rien.games.atelier import Atelier, ErreurAtelier
    from tests.test_emplacement import PREMIER, PSEUDO, Terrain, X, Y, Z

    px, pz = PREMIER

    class Mine(Terrain):
        def sol(self, x, z):
            return Y + emplacement.ECART_VERTICAL + 1

    class Colline(Terrain):
        def sol(self, x, z):
            return Y + emplacement.ECART_VERTICAL

    class Pente(Terrain):
        """Une marche tous les 12 blocs vers l'est : il faut chercher un peu."""

        def sol(self, x, z):
            return Y + (x // 3) % 2 if x < X + 4 else Y

    terrains = {
        "plat": lambda: Terrain(),
        "bosse": lambda: Terrain(sols={(px, pz): Y + 1}),
        "eau": lambda: Terrain(blocs={(px, Y - 1, pz): "minecraft:water"}),
        "construction": lambda: Terrain(blocs={(px, Y + 1, pz): "minecraft:oak_planks"}),
        "mine": lambda: Mine(),
        "colline": lambda: Colline(),
        "pente": lambda: Pente(),
        "non chargé à l'ouest": lambda: Terrain(charge=lambda x, z: x > px + 2),
        "sonde refusée": lambda: Terrain(summon="Unable to summon entity"),
        "enfant absent": lambda: Terrain(joueur=None),
        "autre dimension": lambda: Terrain(dimension=False),
        "enfant en négatif": lambda: Terrain(joueur=(-27.5, 68.0, -0.25)),
        "atelier existant": lambda: Terrain(atelier=[X + 40, Y, Z + 40]),
    }

    def atelier(a):
        return [a.x, a.y, a.z] if a else None

    def construction(c):
        return {"atelier": atelier(c.atelier), "distance": c.distance, "direction": c.direction}

    cas = []
    for nom, fabrique in terrains.items():
        async def chercher(jeu):
            return atelier(await emplacement.chercher(jeu, X, Y, Z))

        async def position(jeu):
            return list(await emplacement.position_de(jeu, PSEUDO))

        async def construire(jeu):
            return construction(await emplacement.construire_pres_de(jeu, PSEUDO))

        async def deplacer(jeu):
            ancien = Atelier(X + 40, Y, Z + 40)
            return construction(await emplacement.deplacer_pres_de(jeu, PSEUDO, ancien))

        actions = [("chercher", chercher), ("position_de", position)]
        # La recherche est la même dans les trois fonctions : on ne la transcrit qu'une fois,
        # sauf là où la suite (construction, déplacement, refus) diffère.
        if nom in ("plat", "mine", "enfant absent", "autre dimension", "enfant en négatif",
                   "atelier existant", "sonde refusée"):
            actions += [("construire_pres_de", construire), ("deplacer_pres_de", deplacer)]
        for f, action in actions:
            cas.append(_transcrire({"f": f, "terrain": nom, "pseudo": PSEUDO, "ici": [X, Y, Z]},
                                   fabrique(), action, (ErreurAtelier,)))
    for dx, dz in ((10, 3), (-10, 3), (2, 9), (2, -9), (5, 5), (-5, 5), (0, 0), (0, -1)):
        cas.append({"entree": {"f": "direction", "dx": dx, "dz": dz},
                    "sortie": emplacement.direction(dx, dz), "echanges": []})
    return cas


def oracle_paisible() -> list[dict]:
    sys.path.insert(0, str(RACINE))
    from mine_de_rien.games.coffre_du_soir import ErreurCoffre
    from mine_de_rien.games.paisible import ModePaisible
    from tests.test_paisible import PSEUDO, FauxServeur

    class Refus(FauxServeur):
        async def commande(self, c):
            if c.startswith("difficulty "):
                return "Unknown or incomplete command"
            return await super().commande(c)

    class Muet(FauxServeur):
        async def commande(self, c):
            return ""

    serveurs = {"Easy": lambda: FauxServeur("Easy"), "Peaceful": lambda: FauxServeur("Peaceful"),
                "Hard": lambda: FauxServeur("Hard"), "refus": lambda: Refus("Easy"),
                "muet": lambda: Muet()}
    cas = []
    for nom, fabrique in serveurs.items():
        for normale in ("easy", "hard"):
            for suite in ([[True, True]], [[False, True]], [[True, False], [False, False]],
                          [[True, True], [True, True], [False, True], [False, True]]):
                async def jouer(jeu, normale=normale, suite=suite):
                    mode = ModePaisible(jeu, PSEUDO, normale)
                    return [await mode.appliquer(a_faire, en_ligne) for a_faire, en_ligne in suite]

                cas.append(_transcrire(
                    {"serveur": nom, "pseudo": PSEUDO, "normale": normale, "suite": suite},
                    fabrique(), jouer, (ErreurCoffre,)))
    return cas



PSEUDO_ESSAI = "Zorglub42"
T0 = "2026-11-07T18:00:00+00:00"


class Serveur:
    """Un serveur simulé, réglable d'un tour à l'autre, pour les scénarios du coffre."""

    def __init__(self, **etat) -> None:
        self.atelier = [0, -60, 8]
        self.en_ligne = True
        self.cle = False
        self.preuve = False
        self.present = True
        self.plein = False
        self.pose_ok = True
        self.refus = None
        self.difficulte = "Easy"
        self.panne = None
        for cle, valeur in etat.items():
            setattr(self, cle, valeur)

    async def commande(self, c: str) -> str:
        from mine_de_rien.adapters.game.rcon import ErreurRcon

        if self.panne and c.startswith(self.panne):
            raise ErreurRcon("serveur RCON injoignable sur essai:25575 : simulé")
        if self.refus and c.startswith(self.refus):
            return "Unknown or incomplete command, see below for error<--[HERE]"
        if c == "list":
            noms = f"{PSEUDO_ESSAI}, Autre" if self.en_ligne else "Autre"
            return f"There are 2 of a max of 20 players online: {noms}"
        if c == "difficulty":
            return f"The difficulty is {self.difficulte}"
        if c.startswith("difficulty "):
            demandee = c.split()[1].capitalize()
            if demandee == self.difficulte:
                return f"The difficulty did not change; it is already set to {demandee}"
            self.difficulte = demandee
            return f"The difficulty has been set to {demandee}"
        if c.startswith("data get storage"):
            if self.atelier is None:
                return "Found no elements matching pos"
            return f"Storage mine_de_rien:atelier has the following contents: {self.atelier}"
        if c.startswith(f"clear {PSEUDO_ESSAI} minecraft:trial_key") and c.endswith(" 0"):
            return (f"Found 1 matching item(s) on player {PSEUDO_ESSAI}" if self.cle
                    else f"No items were found on player {PSEUDO_ESSAI}")
        if c.startswith(f"clear {PSEUDO_ESSAI} minecraft:paper") and c.endswith(" 0"):
            return (f"Found 1 matching item(s) on player {PSEUDO_ESSAI}" if self.preuve
                    else f"No items were found on player {PSEUDO_ESSAI}")
        if c.startswith("give "):
            self.cle = True
            return f"Gave 1 [Clé du coffre] to {PSEUDO_ESSAI}"
        if "Items[0]" in c:
            return "Test passed" if self.plein else "Test failed"
        if c.startswith(("execute if data block", "execute if items block")):
            return "Test passed" if self.pose_ok else "Test failed"
        if c.startswith("execute if block"):
            return "Test passed" if self.present else "Test failed"
        return ""

    async def dire(self, joueur, texte, **style) -> None:
        pass


EXERCICES_COFFRE = [
    {"competence": "maths/division-posee", "enonce": "348 ÷ 12 = ?", "reponse": "29",
     "donnees": {"expression": "348 ÷ 12"}},
    {"competence": "anglais/vocabulaire", "enonce": "bon après-midi en anglais ?",
     "reponse": "good afternoon", "donnees": {}},
    {"competence": "maths/addition-posee", "enonce": "3,5 + 1,25 = ?", "reponse": "4,75",
     "donnees": {}},
    {"competence": "piege", "enonce": 'l\'élève "a" \\ ]}', "reponse": 'o\'clock "x" \\',
     "donnees": {}},
    {"competence": "piege", "enonce": "Le fermier a 348 œufs en boîtes de 12. Combien ?",
     "reponse": "29", "donnees": {}},
    {"competence": "piege", "enonce": " ".join(["abcdefgh"] * 5), "reponse": "1", "donnees": {}},
    {"competence": "piege", "enonce": "tab\tet\nsaut §6 de ligne", "reponse": "é", "donnees": {}},
]


def _exercice_stocke(d: dict, ident: int = 12):
    from datetime import datetime

    from mine_de_rien.exercises.base import Exercice
    from mine_de_rien.storage.sqlite import ExerciceStocke

    return ExerciceStocke(
        id=ident, exercice=Exercice(**d), jeu="coffre-du-soir", source="expression",
        statut="en_file", demande_par="cli", recompense="minecraft:diamond", recompense_nombre=3,
        cree_le=datetime.fromisoformat(T0), pose_le=None, ouvert_le=None)


SCENARIOS_COFFRE = {
    "vie d'un coffre, enfant absent puis connecté": {
        "exercices": [0, 1], "paisible": True, "assistant": "Pixel", "monde": {"en_ligne": False},
        "etapes": [
            {}, {"avance": 60, "monde": {"en_ligne": True}}, {"avance": 3},
            {"avance": 240, "monde": {"preuve": True, "plein": True}, "indices": 2},
            {"avance": 11, "monde": {"preuve": False}}, {"avance": 3},
            {"avance": 3, "monde": {"plein": False}}, {"avance": 3},
            {"avance": 600, "monde": {"preuve": True}}, {"avance": 30, "monde": {"preuve": False}},
            {"avance": 3},
        ]},
    "le suivant attend dix secondes": {
        "exercices": [0, 2], "paisible": False, "assistant": None, "monde": {},
        "etapes": [{}, {"monde": {"preuve": True}}, {"avance": 5, "monde": {"preuve": False}},
                   {"avance": 6}]},
    "coffre disparu, reposé": {
        "exercices": [0], "paisible": True, "assistant": None, "monde": {"en_ligne": False},
        "etapes": [{}, {"avance": 3, "monde": {"present": False}},
                   {"avance": 3, "monde": {"present": True}}]},
    "sans atelier, puis construit": {
        "exercices": [0], "paisible": True, "assistant": "Pixel",
        "monde": {"atelier": None, "difficulte": "Peaceful"},
        "etapes": [{}, {"avance": 3}, {"avance": 3, "monde": {"atelier": [120, 64, -35]}},
                   {"avance": 3, "monde": {"atelier": None}}, {"avance": 3}]},
    "pose refusée par le serveur": {
        "exercices": [0], "paisible": True, "assistant": None, "monde": {"refus": "item replace"},
        "etapes": [{}, {"avance": 3, "monde": {"refus": None}}]},
    "pose non vérifiée": {
        "exercices": [0], "paisible": False, "assistant": None, "monde": {"pose_ok": False},
        "etapes": [{}, {"avance": 3, "monde": {"pose_ok": True}}]},
    "énoncé trop long dans la file": {
        "exercices": [5, 0], "paisible": False, "assistant": None, "monde": {},
        "etapes": [{}, {"avance": 3}]},
    "serveur en panne un tour": {
        "exercices": [0], "paisible": True, "assistant": None, "monde": {"panne": "list"},
        "etapes": [{}, {"avance": 3, "monde": {"panne": None}},
                   {"avance": 3, "monde": {"panne": "difficulty"}},
                   {"avance": 3, "monde": {"panne": None}}]},
    "coffre plein, on demande une fois de le vider": {
        "exercices": [0], "paisible": True, "assistant": None, "monde": {"plein": True},
        "etapes": [{}, {"avance": 3}, {"avance": 3, "monde": {"en_ligne": False}},
                   {"avance": 3, "monde": {"en_ligne": True, "plein": False, "cle": True}}]},
    "rien à faire, difficulté rendue": {
        "exercices": [], "paisible": True, "assistant": None, "monde": {"difficulte": "Peaceful"},
        "etapes": [{}, {"avance": 3}]},
    "clé déjà en poche, niveaux à zéro": {
        "exercices": [0], "paisible": False, "assistant": None, "niveaux": 0,
        "monde": {"cle": True}, "etapes": [{}, {"avance": 3}]},
}


def oracle_coffre_du_soir() -> list[dict]:
    from datetime import datetime, timedelta

    from mine_de_rien.adapters.game.rcon import ErreurRcon
    from mine_de_rien.exercises.base import Exercice
    from mine_de_rien.games import coffre_du_soir as cds
    from mine_de_rien.games.atelier import Atelier
    from mine_de_rien.games.paisible import ModePaisible
    from mine_de_rien.orchestrateur import Orchestrateur
    from mine_de_rien.storage.sqlite import Depot, ErreurStockage

    cas = [{"entree": {"f": "exercices"}, "sortie": EXERCICES_COFFRE, "echanges": []}]
    for x, y, z in ((0, -60, 8), (-27, 68, -16)):
        a = Atelier(x, y, z)
        for i, ex in enumerate(EXERCICES_COFFRE):
            cas.append({"entree": {"f": "commandes_pose", "atelier": [x, y, z], "exercice": i},
                        **_tente(lambda a=a, ex=ex: cds.commandes_pose(a, _exercice_stocke(ex)),
                                 (cds.ErreurCoffre,)), "echanges": []})
        cas.append({"entree": {"f": "commandes_retrait", "atelier": [x, y, z]},
                    "sortie": cds.commandes_retrait(a), "echanges": []})
        for pseudo in ("Steve", "@a"):
            cas.append({"entree": {"f": "commandes_rangement", "atelier": [x, y, z],
                                   "pseudo": pseudo},
                        **_tente(lambda a=a, p=pseudo: cds.commandes_rangement(a, p, 12)),
                        "echanges": []})
    for pseudo in ("Steve", "@a", "Zorglub42"):
        cas.append({"entree": {"f": "commande_cle", "pseudo": pseudo},
                    **_tente(lambda p=pseudo: cds.commande_cle(p)), "echanges": []})
    cas.append({"entree": {"f": "predicats"},
                "sortie": [cds.predicat_cle(), cds.predicat_preuve(12)], "echanges": []})

    methodes = ("joueur_en_ligne", "actualiser_atelier", "poser", "annoncer", "equiper",
                "rappeler", "demander_de_vider", "preuve_trouvee", "coffre_present",
                "coffre_vide", "ranger", "retirer", "a_une_cle", "donner_cle")
    mondes = {"neuf": {}, "hors ligne": {"en_ligne": False}, "clé en poche": {"cle": True},
              "preuve en poche": {"preuve": True}, "coffre absent": {"present": False},
              "coffre plein": {"plein": True}, "sans atelier": {"atelier": None},
              "give refusé": {"refus": "give"}, "pose non vérifiée": {"pose_ok": False},
              "atelier non chargé": {"refus": "setblock"}}
    for nom, etat in mondes.items():
        for assistant in (None, "Pixel"):
            for methode in methodes:
                async def appeler(jeu, methode=methode, assistant=assistant):
                    coffre = cds.CoffreDuSoir(jeu, PSEUDO_ESSAI, 3, assistant)
                    if methode != "actualiser_atelier":
                        await coffre.actualiser_atelier()
                    s = _exercice_stocke(EXERCICES_COFFRE[3])
                    fonction = getattr(coffre, methode)
                    avec_coffre = methode in ("poser", "annoncer", "rappeler", "preuve_trouvee",
                                              "ranger")
                    resultat = await (fonction(s) if avec_coffre else fonction())
                    a = coffre.atelier
                    return {"resultat": resultat, "atelier": [a.x, a.y, a.z] if a else None}

                cas.append(_transcrire(
                    {"f": "methode", "methode": methode, "monde": nom, "assistant": assistant},
                    Serveur(**etat), appeler, (cds.ErreurCoffre,)))

    for nom, scenario in SCENARIOS_COFFRE.items():
        with tempfile.TemporaryDirectory() as dossier:
            depot = Depot.ouvrir(Path(dossier) / "mdr.sqlite")
            t0 = datetime.fromisoformat(T0)
            for i in scenario["exercices"]:
                depot.ajouter(Exercice(**EXERCICES_COFFRE[i]), source="expression",
                              demande_par="cli", recompense="minecraft:diamond",
                              recompense_nombre=3, quand=t0)
            serveur = Serveur(**scenario["monde"])
            notifications: list[str] = []
            horloge = [t0]

            class Notif:
                async def notifier(self, message, notifications=notifications):
                    notifications.append(message)

            async def jouer(jeu, scenario=scenario, serveur=serveur, depot=depot,
                            horloge=horloge, notifications=notifications, t0=t0):
                coffre = cds.CoffreDuSoir(jeu, PSEUDO_ESSAI, scenario.get("niveaux", 3),
                                          scenario["assistant"])
                mode = ModePaisible(jeu, PSEUDO_ESSAI, "easy") if scenario["paisible"] else None
                orch = Orchestrateur(depot, coffre, Notif(), lambda: horloge[0], paisible=mode)
                for etape in scenario["etapes"]:
                    for cle, valeur in etape.get("monde", {}).items():
                        setattr(serveur, cle, valeur)
                    horloge[0] += timedelta(seconds=etape.get("avance", 0))
                    for _ in range(etape.get("indices", 0)):
                        depot.noter("indice", depot.actif().id, horloge[0])
                    try:
                        await orch.tour()
                    except (ErreurRcon, cds.ErreurCoffre, ErreurStockage) as e:
                        notifications.append(f"ERREUR {e}")
                tous = depot.entre(t0 - timedelta(days=1), t0 + timedelta(days=1))
                return {
                    "notifications": notifications,
                    "coffres": [[s.id, s.statut, s.niveaux_donnes, cds.formater_duree(s.duree)]
                                for s in tous],
                    "evenements": [[e.exercice_id, e.genre] for e in depot.evenements_entre(
                        t0 - timedelta(days=1), t0 + timedelta(days=1))],
                }

            entree = {"f": "scenario", "nom": nom,
                      **{k: scenario[k] for k in ("exercices", "paisible", "assistant", "etapes")},
                      "niveaux": scenario.get("niveaux", 3)}
            cas.append(_transcrire(entree, serveur, jouer, ()))
            depot.fermer()
    return cas



CALCULS_PHOTO = [
    "Double de 17", "Double de 17 = 35", "348 ÷ 12 = 30", "Moitié de 3", "moitie  de 1 000",
    "Double de 2,5", "Moitié de 1,25", "10 ÷ 3", "799", "799 – 760", "799 — 760", "799 − 760",
    "1 200 + 12 345", "1 200 + 12 345", "1 200 + 12 345", "MOITIÉ DE 8", "Double d'8",
    "double de8", "Double  de\t12", "Le double de 4", "Double de 4 et 5", "Double de", "Moitie de 7",
    "Double de 1000", "Moitié de 1 200", "Double de 0,5", "Double de 12.5", "75,5 - 50 =",
    "= 12", "", "3 x 4 = 12 = 13", "Doublé de 3", "Double de 3 ", " (12 + 8) x 3", "2 + 3 × (4 + 1)",
]

LECTURES_PHOTO = {
    "résultat du LLM jamais lu": [{"lignes": [
        {"genre": "calcul", "texte": "Double de 17 = 35"},
        {"genre": "calcul", "texte": "348 ÷ 12 = 30"}]}],
    "rejets": [{"lignes": [
        {"genre": "calcul", "texte": "Moitié de 1,25"}, {"genre": "calcul", "texte": "10 ÷ 3"},
        {"genre": "calcul", "texte": "799"}, {"genre": "mot", "texte": "Greetings"},
        {"genre": "mot", "texte": "book"}, {"genre": "mot", "texte": "orange"}]}],
    "doublons": [{"lignes": [
        {"genre": "calcul", "texte": "Double de 20"}, {"genre": "calcul", "texte": "Double de 20 = 40"},
        {"genre": "mot", "texte": "May"}, {"genre": "mot", "texte": "may"}]},
        {"lignes": [{"genre": "calcul", "texte": "Double de 20"},
                    {"genre": "calcul", "texte": "3 x 4"}]}],
    "lignes mal formées": [{"lignes": [
        "texte nu", {"genre": "mot"}, None, 12, True, [1, "a"], {"genre": "calcul", "texte": 3},
        {"texte": "   "}, {"texte": "Double de 2"}, {"genre": "autre", "texte": "3 + 4"},
        {"genre": "mot", "texte": "x" * 100}, {"genre": "calcul", "texte": "9" * 100},
        {"l'élève": "a \"dit\"", "liste": [None, False, 1.5]}]}],
    "pas de liste": [{"autre": []}],
    "liste qui n'en est pas une": [{"lignes": "Double de 2"}],
    "vide": [{"lignes": []}],
    "onze calculs écartés": [{"lignes": [{"genre": "calcul", "texte": f"{n} ÷ 0"}
                                         for n in range(1, 12)]}],
    "familles mêlées": [{"lignes": [
        {"genre": "mot", "texte": "Wednesday"}, {"genre": "calcul", "texte": "12 x 7"},
        {"genre": "mot", "texte": "red"}, {"genre": "mot", "texte": "seven"},
        {"genre": "calcul", "texte": "Moitié de 8"}, {"genre": "mot", "texte": "good night"},
        {"genre": "mot", "texte": "february"}, {"genre": "calcul", "texte": "Double de 9"}]}],
}

TAILLES_PHOTO = [(1280, 885), (885, 1280), (4000, 3000), (3000, 4000), (1536, 1536), (1537, 1536),
                 (1536, 1537), (100, 100), (2000, 2000), (5000, 100), (100, 5000), (3264, 2448),
                 (2448, 3264), (1601, 1200), (1, 1), (2, 1), (3073, 1000)]


def _lecture(photo, reponses) -> dict:
    import random

    from mine_de_rien.exercises.anglais import vocabulaire

    lecture = photo.interpreter(reponses, random.Random(0))
    # Le sens d'un mot est tiré au hasard : on ne compare que ce qui n'en dépend pas.
    exercices = [
        {"anglais": ex.donnees["anglais"], "famille": ex.donnees["famille"]}
        if ex.competence == vocabulaire.competence else _exercice(ex)
        for ex in lecture.exercices
    ]
    return {"exercices": exercices, "rejets": [[r.texte, r.raison] for r in lecture.rejets],
            "resume": photo.resumer(lecture)}


def oracle_photo() -> list[dict]:
    import io

    from PIL import Image

    from mine_de_rien import photo
    from mine_de_rien.adapters.llm.base import ErreurLlm, lire_json
    from mine_de_rien.exercises.expression import ErreurExpression

    cas = [{"entree": {"f": "constantes"},
            "sortie": {"systeme": photo.SYSTEME, "consigne": photo.CONSIGNE}}]
    for texte in CALCULS_PHOTO:
        cas.append({"entree": {"f": "calcul", "texte": texte},
                    **_tente(lambda t=texte: _exercice(photo._calcul(t)), (ErreurExpression,))})
    donnees = RACINE / "tests" / "donnees" / "photo"
    for fichier in sorted(donnees.glob("*.json")):
        reponses = json.loads(fichier.read_text("utf-8"))
        cas.append({"entree": {"f": "interpreter", "fichier": fichier.name},
                    "sortie": _lecture(photo, reponses)})
    for nom, reponses in LECTURES_PHOTO.items():
        cas.append({"entree": {"f": "interpreter", "nom": nom, "reponses": reponses},
                    **_tente(lambda r=reponses: _lecture(photo, r), (ErreurLlm,))})
    for largeur, hauteur in TAILLES_PHOTO:
        tampon = io.BytesIO()
        Image.new("RGB", (largeur, hauteur), "white").save(tampon, format="JPEG")
        morceaux = photo.preparer(tampon.getvalue())
        cas.append({"entree": {"f": "preparer", "taille": [largeur, hauteur]},
                    "sortie": [list(Image.open(io.BytesIO(m)).size) for m in morceaux]})

    # Une image de synthèse, sans rien de personnel : moitié gauche rouge, moitié droite bleue,
    # 60 × 20, à tourner d'un quart de tour selon l'EXIF (orientation 6).
    image = Image.new("RGB", (60, 20), (220, 30, 30))
    image.paste((30, 30, 220), (30, 0, 60, 20))
    exif = Image.Exif()
    exif[0x0112] = 6
    chemin = donnees / "orientation-6.jpg"
    image.save(chemin, format="JPEG", quality=95, exif=exif)
    (morceau,) = photo.preparer(chemin.read_bytes())
    redressee = Image.open(io.BytesIO(morceau))
    haut, bas = redressee.getpixel((10, 5)), redressee.getpixel((10, 55))
    cas.append({"entree": {"f": "orientation", "fichier": chemin.name},
                "sortie": {"taille": list(redressee.size),
                           "haut": "rouge" if haut[0] > haut[2] else "bleu",
                           "bas": "rouge" if bas[0] > bas[2] else "bleu"}})

    for texte in ['{"a": 1}', '```json\n{"lignes": []}\n```', '  ```\n{"b": 2}\n```  ',
                  "Voici les lignes : Double de 2", "[1, 2]", "", "```json\n[1]\n```",
                  '{"a": "é"} ', '```\n{"a": {"b": [1, 2]}}```', "``` {} ```", '"texte"', "12",
                  'Voici : ```json\n{"a": 1}\n```']:
        cas.append({"entree": {"f": "lire_json", "texte": texte},
                    **_tente(lambda t=texte: lire_json(t), (ErreurLlm,))})
    return cas



MESSAGES_PIXEL = [
    "Pixel, aide-moi", "pixel: tp", "PIXEL ramène-moi", "  Pïxel,  aide ", "Pixel", "pixel,",
    "Pixels, aide", "bonjour Pixel", "aide-moi", "Pix, aide", "Pixel\taide", "Pixel!aide",
    "Pixel , : aide", "PIXEL,ramène-moi au coffre", "pixel aide", "Pixèl, aide", "Pixel,,, ",
    "pixel:\t\ttp", "Pixel ", " Pixel, aide", "Pixel aide", "Píxel aide", "pi xel aide", "",
    "Pixel, " + "a" * 300, "Zoé, aide-moi", "zoe aide", "ZOÉ: tp",
]

DEMANDES_PIXEL = [
    "ramène-moi au coffre", "téléporte moi", "tp", "aide-moi", "je suis bloqué", "je sais pas",
    "donne un indice", "c'est combien ?", "ça veut dire quoi", "je comprends pas",
    "tu aimes les creepers ?", "", "RAMÈNE", "un retour au coffre", "htp", "stp aide", "j'aide",
    "laide", "il signifie quoi", "quelle est la réponse", "help", "comment on fait",
    "je trouve pas", "emmène-moi", "amène moi", "le teleporteur", "c'est quoi", "sais  pas",
    "je comprend pas", "aide\tmoi", "à l'aide !", "AIDE", "indices", "combien font 2 + 2",
    "raconte une blague", "tape", "tpx", "x tp", "bloqué !!", "blo que",
]

BRUTS_PIXEL = [
    "Salut ! 😊✨ ⛏️", "Courage !", "", "   ", "Deux   espaces\net\tun saut", "a\x00b§c",
    "x" * 300, "Bravo ★ ☆ ♥ → ✓ © ® ™ ° ± × ÷ € $ + = < >", "é̀ composé", " privé",
    "Voilà 34 ! ٣", "1️⃣ 2️⃣", "👍🏽 famille 👨‍👩‍👧", "« guillemets » – tiret — long… fin",
]

SCENARIOS_PIXEL = {
    "indices sans LLM jusqu'au dernier répété": {
        "exercice": "Double de 17", "llm": None,
        "messages": [["Pixel, aide-moi", 0]] * 5},
    "autres joueurs et messages sans le nom": {
        "exercice": "Double de 17", "llm": {},
        "messages": [["aide-moi", 0], ["Copain|Pixel, aide-moi", 0], ["Pixels, aide", 0]]},
    "téléportation, patience, puis de nouveau": {
        "exercice": "Double de 17", "llm": {"intention": "teleporter"},
        "messages": [["Pixel, ramène-moi au coffre", 0], ["Pixel, ramène-moi au coffre", 4],
                     ["Pixel, ramène-moi au coffre", 2], ["Pixel, emmène-moi", 0]]},
    "sans coffre": {
        "exercice": None, "llm": None,
        "messages": [["Pixel, tp", 0], ["Pixel, aide", 0], ["Pixel, salut", 0], ["Pixel,", 0]]},
    "atelier jamais construit": {
        "exercice": "Double de 17", "llm": None, "jeu": {"atelier": None},
        "messages": [["Pixel, ramène-moi au coffre", 0], ["Pixel, aide", 0]]},
    "atelier ailleurs": {
        "exercice": "Double de 17", "llm": None, "jeu": {"atelier": [0, -60, 8]},
        "messages": [["Pixel, ramène-moi au coffre", 0]]},
    "enfant déconnecté": {
        "exercice": "Double de 17", "llm": None, "jeu": {"reponse_tp": "No entity was found"},
        "messages": [["Pixel, tp", 0], ["Pixel, tp", 0]]},
    "reformulation acceptée": {
        "exercice": "Double de 17",
        "llm": {"reformulation": "Astuce : ajoute 17 à lui-même, tu vas y arriver !"},
        "messages": [["Pixel, aide-moi", 0]]},
    "reformulation qui donne la réponse": {
        "exercice": "Double de 17", "llm": {"reformulation": "Ça fait 34 !"},
        "messages": [["Pixel, aide-moi", 0]]},
    "reformulation en lettres": {
        "exercice": "Double de 17", "llm": {"reformulation": "C'est trente-quatre."},
        "messages": [["Pixel, aide-moi", 0]]},
    "reformulation avec un calcul inventé": {
        "exercice": "Double de 17", "llm": {"reformulation": "Le double de 7 = 15, ajoute 10."},
        "messages": [["Pixel, aide-moi", 0], ["Pixel, aide-moi", 0], ["Pixel, aide-moi", 0]]},
    "reformulation vide": {
        "exercice": "Double de 17", "llm": {"reformulation": ""},
        "messages": [["Pixel, aide-moi", 0]]},
    "reformulation trop longue": {
        "exercice": "Double de 17", "llm": {"reformulation": "x" * 300},
        "messages": [["Pixel, aide-moi", 0]]},
    "reformulation avec émojis et milliers": {
        "exercice": "Moitié de 1 200",
        "llm": {"reformulation": "Pense en centaines 😊 : 1 200, c'est 12 centaines !"},
        "messages": [["Pixel, aide-moi", 0], ["Pixel, aide-moi", 0]]},
    "bavardage sans chiffre": {
        "exercice": None, "llm": {"intention": "bavardage", "bavardage": "J'adore les moutons !"},
        "messages": [["Pixel, tu aimes quoi ?", 0]]},
    "bavardage avec chiffre": {
        "exercice": None,
        "llm": {"intention": "bavardage", "bavardage": "348 divisé par 12, ça fait 29."},
        "messages": [["Pixel, et 348 divisé par 12 ?", 0],
                     ["Pixel, combien font 348 divisé par 12 ?", 0]]},
    "bavardage qui donne la réponse en lettres": {
        "exercice": "Double de 17",
        "llm": {"intention": "bavardage", "bavardage": "Peut-être trente-quatre ?"},
        "messages": [["Pixel, devine", 0]]},
    "bavardage avec émojis": {
        "exercice": None, "llm": {"intention": "bavardage", "bavardage": "Salut ! 😊✨ ⛏️"},
        "messages": [["Pixel, salut", 0]]},
    "LLM distant": {
        "exercice": "Double de 17", "llm": {"distant": True},
        "messages": [["Pixel, aide-moi", 0], ["Pixel, raconte une blague", 0]]},
    "LLM en erreur": {
        "exercice": "Double de 17", "llm": {"erreur": True},
        "messages": [["Pixel, ramène-moi", 0], ["Pixel, aide", 0], ["Pixel, salut", 0]]},
    "intention hors liste": {
        "exercice": None, "llm": {"intention": "donner_diamants", "bavardage": "Bonne idée ?"},
        "messages": [["Pixel, donne-moi des diamants", 0]]},
    "le LLM classe ce que les mots-clés ne reconnaissent pas": {
        "exercice": "348 ÷ 12", "llm": {"intention": "indice", "reformulation": "Courage !"},
        "messages": [["Pixel, je n'y arrive vraiment plus", 0],
                     ["Pixel, " + "très " * 60 + "long", 0]]},
    "le LLM classe vers la téléportation": {
        "exercice": "348 ÷ 12", "llm": {"intention": "teleporter"},
        "messages": [["Pixel, je veux voir le coffre", 0]]},
    "mot anglais": {
        "exercice": "mot:august", "llm": None, "nom": "Zoé",
        "messages": [["Zoé, aide-moi", 0]] * 4 + [["zoe tp", 0], ["Pixel, aide", 0]]},
}


def oracle_pixel() -> list[dict]:
    from datetime import datetime, timedelta

    sys.path.insert(0, str(RACINE))
    from mine_de_rien import photo
    from mine_de_rien.adapters.game.base import MessageChat
    from mine_de_rien.exercises.anglais import vocabulaire
    from mine_de_rien.games import pixel as px
    from mine_de_rien.games.atelier import Atelier
    from mine_de_rien.storage.sqlite import Depot
    from tests.test_pixel import PSEUDO, FauxJeu, FauxLlm

    cas = [{"entree": {"f": "constantes"}, "echanges": [], "sortie": {
        "classement": [px.SYSTEME_CLASSEMENT, px.CONSIGNE_CLASSEMENT.format(message="M")],
        "reformulation": px.SYSTEME_REFORMULATION, "bavardage": px.SYSTEME_BAVARDAGE}}]
    for nom in ("Pixel", "Zoé"):
        for message in MESSAGES_PIXEL:
            cas.append({"entree": {"f": "adresse", "message": message, "nom": nom},
                        "sortie": px.adresse(message, nom), "echanges": []})
    for demande in DEMANDES_PIXEL:
        cas.append({"entree": {"f": "classer", "demande": demande},
                    "sortie": px.classer_par_mots_cles(demande), "echanges": []})
        cas.append({"entree": {"f": "normaliser_espaces", "demande": demande},
                    "sortie": px.normaliser_espaces(demande), "echanges": []})
    for brut in BRUTS_PIXEL:
        cas.append({"entree": {"f": "propre", "brut": brut}, "sortie": px.propre(brut),
                    "echanges": []})
    for nom in ("Pixel", "Zoé", "P", "@a tp", "x" * 17, "l'ami", "Pixel2", ""):
        cas.append({"entree": {"f": "verifier_nom", "nom": nom},
                    **_tente(lambda n=nom: px.verifier_nom(n)), "echanges": []})
    for pseudo in ("Steve_42", "@a"):
        cas.append({"entree": {"f": "commande_teleportation", "pseudo": pseudo},
                    **_tente(lambda p=pseudo: px.commande_teleportation(Atelier(10, 64, -5), p)),
                    "echanges": []})

    debut = datetime.fromisoformat("2026-11-08T18:00:00+00:00")
    for nom, scenario in SCENARIOS_PIXEL.items():
        with tempfile.TemporaryDirectory() as dossier:
            depot = Depot.ouvrir(Path(dossier) / "mdr.sqlite")
            exercice = None
            if scenario["exercice"] is not None:
                if scenario["exercice"].startswith("mot:"):
                    mot = scenario["exercice"].removeprefix("mot:")
                    paire = next(p for p in vocabulaire.lexique() if p.anglais == mot)
                    exercice = vocabulaire.exercice(paire, vocabulaire.VERS_FRANCAIS)
                else:
                    exercice = photo._calcul(scenario["exercice"])
                ident = depot.ajouter(exercice, source="photo", demande_par="1",
                                      recompense="minecraft:diamond", recompense_nombre=3,
                                      quand=debut)
                depot.marquer_pose(ident, debut)
            llm = FauxLlm(**scenario["llm"]) if scenario["llm"] is not None else None
            jeu = FauxJeu(**{k: (tuple(v) if isinstance(v, list) else v)
                             for k, v in scenario.get("jeu", {}).items()})
            temps = [debut + timedelta(minutes=1)]

            async def jouer(enregistreur, scenario=scenario, depot=depot, llm=llm, temps=temps,
                            debut=debut):
                pixel = px.Pixel(enregistreur, depot, PSEUDO, scenario.get("nom", "Pixel"), llm,
                                 lambda: temps[0])
                for texte, avance in scenario["messages"]:
                    joueur = PSEUDO
                    if "|" in texte:
                        joueur, texte = texte.split("|", 1)
                    temps[0] += timedelta(minutes=avance)
                    await pixel.recevoir(MessageChat(debut, joueur, texte))
                    if pixel._tache is not None:
                        await pixel._tache
                return {
                    "evenements": [[e.exercice_id, e.genre, e.quand.isoformat(timespec="seconds")]
                                   for e in depot.evenements_entre(
                                       debut - timedelta(days=1), debut + timedelta(days=1))],
                    "appels": [list(a) for a in llm.appels] if llm is not None else None,
                }

            entree = {"f": "scenario", "nom": nom, "pseudo": PSEUDO,
                      "assistant": scenario.get("nom", "Pixel"),
                      "exercice": _exercice(exercice) if exercice else None,
                      "llm": scenario["llm"], "messages": scenario["messages"]}
            cas.append(_transcrire(entree, jeu, jouer, ()))
            depot.fermer()
    return cas


ORACLES = {
    "config": oracle_config,
    "texte": oracle_texte,
    "expression": oracle_expression,
    "vocabulaire": oracle_vocabulaire,
    "indices": oracle_indices,
    "saisie": oracle_saisie,
    "stockage": oracle_stockage,
    "rapport": oracle_rapport,
    "atelier": oracle_atelier,
    "emplacement": oracle_emplacement,
    "paisible": oracle_paisible,
    "coffre_du_soir": oracle_coffre_du_soir,
    "photo": oracle_photo,
    "pixel": oracle_pixel,
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
