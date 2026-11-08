"""Indices par paliers pour un coffre, calculés en Python, sans jamais la réponse.

Pixel (spec `2026-11-aide-dans-le-chat`) donne le palier suivant à chaque demande, puis
répète le dernier. Un palier aide à trouver la méthode, de plus en plus précisément ; le
dernier peut aller jusqu'aux calculs intermédiaires, mais **jamais jusqu'au résultat**.

Chaque palier est construit à partir de l'`Exercice` stocké (énoncé, réponse, `donnees`),
puis passé au filtre `contient_la_reponse` : un palier qui contiendrait la réponse, sous une
forme que l'enclume accepterait ou que l'enfant n'aurait plus qu'à recopier, est retiré.
S'il ne reste rien, l'indice de secours ne contient aucun nombre.

Le même filtre sert à Pixel sur les phrases du LLM.

Familles :
- doubles et moitiés (`maths/doubles-moities`) : définition, décomposition, morceaux ;
- calculs à deux nombres (+, −, ×, ÷), saisis par un parent, lus sur une photo ou tirés
  par le générateur de division : méthode, décomposition, calculs intermédiaires ;
- calcul avec plusieurs opérations : l'ordre des priorités ;
- vocabulaire anglais : sens, première lettre et longueur, repère (rang, objet, moment) ;
- tout le reste : relire le panneau.
"""

from __future__ import annotations

import re
import tomllib
import unicodedata
from fractions import Fraction
from functools import cache
from importlib import resources

from mine_de_rien.exercises.anglais import vocabulaire
from mine_de_rien.exercises.base import Exercice
from mine_de_rien.exercises.expression import ErreurExpression, ecrire_reponse

LONGUEUR_MAX = 150
SECOURS = "Relis bien le panneau, puis essaie sur l'enclume : chaque essai coûte de l'expérience."

_NOMBRE = r"\d+(?:[.,]\d+)?"
_BINAIRE = re.compile(rf"^\s*({_NOMBRE})\s*([-+×÷])\s*({_NOMBRE})\s*$")
_MILLIERS = re.compile(r"(?<=\d)[   ](?=\d{3}(?!\d))")
_NOMBRES_TEXTE = re.compile(_NOMBRE)


# --- Le filtre -------------------------------------------------------------------------


def _mots(texte: str) -> list[str]:
    """Minuscules, sans accents, découpé en mots et en nombres (virgule décimale gardée)."""
    sans_accents = unicodedata.normalize("NFKD", texte).encode("ascii", "ignore").decode()
    sans_milliers = _MILLIERS.sub("", sans_accents.lower())
    return re.findall(r"\d+(?:[.,]\d+)?|[a-z]+", sans_milliers)


def _contient_suite(mots: list[str], suite: list[str]) -> bool:
    n = len(suite)
    return n > 0 and any(mots[i : i + n] == suite for i in range(len(mots) - n + 1))


def _valeur(texte: str) -> Fraction | None:
    if not re.fullmatch(_NOMBRE, texte.strip()):
        return None
    return Fraction(texte.strip().replace(",", "."))


_UNITES = [
    "zero", "un", "deux", "trois", "quatre", "cinq", "six", "sept", "huit", "neuf", "dix",
    "onze", "douze", "treize", "quatorze", "quinze", "seize", "dix-sept", "dix-huit",
    "dix-neuf",
]
_DIZAINES = {2: "vingt", 3: "trente", 4: "quarante", 5: "cinquante", 6: "soixante"}


def _moins_de_cent(n: int) -> str:
    if n < 20:
        return _UNITES[n]
    if n < 70:
        d, u = divmod(n, 10)
        if u == 0:
            return _DIZAINES[d]
        return _DIZAINES[d] + ("-et-un" if u == 1 else "-" + _UNITES[u])
    if n < 80:
        return "soixante-et-onze" if n == 71 else "soixante-" + _UNITES[n - 60]
    if n == 80:
        return "quatre-vingts"
    return "quatre-vingt-" + _UNITES[n - 80]


def _moins_de_mille(n: int) -> str:
    c, reste = divmod(n, 100)
    if c == 0:
        return _moins_de_cent(reste)
    tete = "cent" if c == 1 else _UNITES[c] + "-cent" + ("s" if reste == 0 else "")
    return tete if reste == 0 else f"{tete}-{_moins_de_cent(reste)}"


def en_lettres(n: int) -> str:
    """Un entier de 0 à 999 999 en lettres, avec des tirets partout (rectifications de 1990).
    Sert seulement à reconnaître la réponse écrite en lettres : la comparaison se fait mot à
    mot, les tirets et le « et » de « vingt et un » ne comptent donc pas."""
    if not 0 <= n < 1_000_000:
        raise ValueError(n)
    milliers, reste = divmod(n, 1000)
    if milliers == 0:
        return _moins_de_mille(reste)
    tete = "mille" if milliers == 1 else _moins_de_mille(milliers) + "-mille"
    return tete if reste == 0 else f"{tete}-{_moins_de_mille(reste)}"


def _suite_lettres(n: int) -> list[str]:
    return [m for m in _mots(en_lettres(n).replace("-", " ")) if m != "et"]


def _valeur_nombre_anglais(ex: Exercice) -> int | None:
    """Pour « seven en français ? », la valeur 7 : l'écrire en chiffres donnerait la réponse."""
    if ex.donnees.get("famille") != "nombres":
        return None
    nombres = [p.anglais for p in vocabulaire.lexique() if p.famille == "nombres"]
    anglais = ex.donnees.get("anglais")
    return nombres.index(anglais) if anglais in nombres else None


def contient_la_reponse(texte: str, ex: Exercice) -> bool:
    """Vrai si `texte` donne la réponse : en chiffres (virgule ou point, espaces de milliers),
    en lettres pour un entier, ou le mot lui-même, en mot entier, sans accents ni casse."""
    mots = _mots(texte)
    valeur = _valeur(ex.reponse)
    if valeur is not None:
        if any(Fraction(m.replace(",", ".")) == valeur for m in mots if m[0].isdigit()):
            return True
        if valeur.denominator == 1 and valeur < 1_000_000:
            sans_et = [m for m in mots if m != "et"]
            return _contient_suite(sans_et, _suite_lettres(int(valeur)))
        return False
    if _contient_suite(mots, _mots(ex.reponse)):
        return True
    nombre = _valeur_nombre_anglais(ex)
    if nombre is not None:
        return any(m == str(nombre) for m in mots) or _contient_suite(
            [m for m in mots if m != "et"], _suite_lettres(nombre)
        )
    return False


# --- Les paliers -----------------------------------------------------------------------


def _e(x: Fraction) -> str:
    return ecrire_reponse(x)


def _decomposer(n: Fraction) -> tuple[Fraction, Fraction] | None:
    """17 → (10, 7), 1200 → (1000, 200), 2,5 → (2, 0,5). None si rien à décomposer."""
    entier = n.numerator // n.denominator
    if entier != n:
        if entier == 0:
            return None
        return Fraction(entier), n - entier
    if entier < 10:
        return None
    puissance = 10 ** (len(str(entier)) - 1)
    tete = entier // puissance * puissance
    if tete == entier:
        return None
    return Fraction(tete), Fraction(entier - tete)


def _rond(n: Fraction) -> tuple[int, str] | None:
    """40 → (4, "dizaines"), 3000 → (3, "milliers") ; None si ce n'est pas un nombre rond."""
    if n.denominator != 1 or n < 10:
        return None
    for puissance, nom in ((1000, "milliers"), (100, "centaines"), (10, "dizaines")):
        if n % puissance == 0:
            return int(n // puissance), nom
    return None


def _doubles_moities(ex: Exercice) -> list[str]:
    n = Fraction(ex.donnees["nombre"].replace(",", "."))
    double = ex.donnees.get("famille") == "doubles"
    mot = "le double" if double else "la moitié"
    paliers = [
        f"Le double de {_e(n)}, c'est {_e(n)} ajouté à lui-même."
        if double
        else f"La moitié de {_e(n)}, c'est le nombre qui, ajouté à lui-même, donne {_e(n)}."
    ]
    morceaux = _decomposer(n)
    rond = _rond(n)
    if morceaux:
        a, b = morceaux
        paliers.append(
            f"Découpe : {_e(n)} = {_e(a)} + {_e(b)}. Cherche {mot} de {_e(a)}, "
            f"puis {mot} de {_e(b)}."
        )
        fa, fb = (a * 2, b * 2) if double else (a / 2, b / 2)
        try:
            paliers.append(
                f"{mot.capitalize()} de {_e(a)} = {_e(fa)} et {mot} de {_e(b)} = {_e(fb)} : "
                f"il reste à les ajouter."
            )
        except ErreurExpression:
            pass
    elif rond:
        combien, nom = rond
        paliers.append(f"Pense en {nom} : {_e(n)}, c'est {combien} {nom}.")
        paliers.append(f"Cherche {mot} de {combien}, puis compte-le en {nom}.")
    elif double:
        paliers.append(f"Sur tes doigts : {_e(n)}, et encore {_e(n)} de plus.")
    else:
        paliers.append(
            f"Imagine {_e(n)} jetons partagés équitablement entre toi et ton ami : "
            f"combien chacun ?"
        )
    return paliers


def _par_rangs(n: Fraction) -> list[Fraction]:
    """348 → [300, 40, 8] ; 3,25 → [3, 0,25]. Les morceaux nuls sont omis."""
    entier = n.numerator // n.denominator
    if entier != n:
        return [x for x in (Fraction(entier), n - entier) if x]
    chiffres = str(entier)
    return [
        Fraction(int(c) * 10 ** (len(chiffres) - i - 1)) for i, c in enumerate(chiffres) if c != "0"
    ] or [Fraction(0)]


def _addition(a: Fraction, b: Fraction) -> list[str]:
    decimaux = a.denominator != 1 or b.denominator != 1
    paliers = [
        "Pose l'addition en colonnes : "
        + ("virgule sous virgule." if decimaux else "unités sous unités, dizaines sous dizaines.")
    ]
    ra, rb = _par_rangs(a), _par_rangs(b)
    if decimaux:
        ea, da = Fraction(a.numerator // a.denominator), a - a.numerator // a.denominator
        eb, db = Fraction(b.numerator // b.denominator), b - b.numerator // b.denominator
        paliers.append(
            f"Ajoute les parties entières ({_e(ea)} + {_e(eb)}), puis les parties décimales "
            f"({_e(da)} + {_e(db)})."
        )
        paliers.append(
            f"{_e(ea)} + {_e(eb)} = {_e(ea + eb)} et {_e(da)} + {_e(db)} = {_e(da + db)} : "
            "il reste à les ajouter."
        )
    elif len(ra) > 1 or len(rb) > 1:
        grand, petit = (a, b) if a >= b else (b, a)
        morceaux = _par_rangs(petit)
        etapes = " puis ".join(f"+ {_e(m)}" for m in morceaux)
        paliers.append(f"Pars de {_e(grand)} et ajoute en plusieurs bonds : {etapes}.")
        if len(morceaux) > 1:
            paliers.append(
                f"{_e(grand)} + {_e(morceaux[0])} = {_e(grand + morceaux[0])} : il reste à "
                f"ajouter {_e(petit - morceaux[0])}."
            )
    else:
        grand, petit = (a, b) if a >= b else (b, a)
        paliers.append(f"Pars de {_e(grand)} et avance de {_e(petit)} sur tes doigts.")
    return paliers


def _soustraction(a: Fraction, b: Fraction) -> list[str]:
    paliers = ["Pose la soustraction en colonnes, le plus grand nombre en haut."]
    morceaux = _par_rangs(b)
    if len(morceaux) > 1:
        etapes = " puis ".join(f"− {_e(m)}" for m in morceaux)
        paliers.append(f"Pars de {_e(a)} et retire en plusieurs bonds : {etapes}.")
        paliers.append(
            f"{_e(a)} − {_e(morceaux[0])} = {_e(a - morceaux[0])} : il reste à retirer "
            f"{_e(b - morceaux[0])}."
        )
    elif b.denominator == 1 and b < 10:
        paliers.append(f"Compte à rebours depuis {_e(a)}, de {_e(b)} pas.")
    elif b.denominator == 1:
        rang = {1: "dizaines", 2: "centaines", 3: "milliers"}.get(len(str(int(b))) - 1)
        if rang:
            paliers.append(f"Retire {_e(b)} : commence par le chiffre des {rang} de {_e(a)}.")
    return paliers


def _multiplication(a: Fraction, b: Fraction) -> list[str]:
    if a.denominator != 1 or b.denominator != 1:
        return [
            (
                "Calcule d'abord sans les virgules, puis remets la virgule : autant de "
                "chiffres après elle qu'en tout dans les deux nombres."
            )
        ]
    grand, petit = (a, b) if a >= b else (b, a)
    paliers = [
        f"C'est dans la table de {_e(petit)}." if petit <= 10 else "Pose la multiplication."
    ]
    morceaux = _decomposer(grand)
    if morceaux:
        x, y = morceaux
        paliers.append(
            f"Découpe : {_e(grand)} × {_e(petit)} = {_e(x)} × {_e(petit)} + {_e(y)} × {_e(petit)}."
        )
        paliers.append(
            f"{_e(x)} × {_e(petit)} = {_e(x * petit)} et {_e(y)} × {_e(petit)} = "
            f"{_e(y * petit)} : il reste à les ajouter."
        )
    elif grand < 10 and grand > 5:
        paliers.append(
            f"{_e(petit)} × {_e(grand)}, c'est {_e(petit)} × 10 moins {_e(petit)} × "
            f"{_e(10 - grand)}."
        )
        paliers.append(
            f"{_e(petit)} × 10 = {_e(petit * 10)} et {_e(petit)} × {_e(10 - grand)} = "
            f"{_e(petit * (10 - grand))} : il reste à soustraire."
        )
    return paliers


def _division(a: Fraction, b: Fraction) -> list[str]:
    paliers = [f"Cherche combien de fois {_e(b)} entre dans {_e(a)}."]
    if a.denominator != 1 or b.denominator != 1 or b == 0:
        return paliers
    dividende, diviseur = int(a), int(b)
    multiples = ", ".join(str(diviseur * k) for k in range(1, 6))
    paliers.append(f"Les multiples de {diviseur} : {multiples}… Lequel s'approche ?")
    chiffres = str(dividende)
    for i in range(1, len(chiffres)):
        tete = int(chiffres[:i])
        if tete >= diviseur:
            q, r = divmod(tete, diviseur)
            paliers.append(
                f"Première étape : dans {tete}, {diviseur} va {q} fois, il reste {r}. "
                f"Abaisse le chiffre suivant."
            )
            break
    return paliers


def _calcul(expression: str) -> list[str]:
    m = _BINAIRE.match(expression)
    if not m:
        return [
            (
                "Commence par ce qui est entre parenthèses, puis les × et les ÷, et termine "
                "par les + et les −."
            )
        ]
    a = Fraction(m.group(1).replace(",", "."))
    b = Fraction(m.group(3).replace(",", "."))
    operation = {"+": _addition, "-": _soustraction, "×": _multiplication, "÷": _division}
    return operation[m.group(2)](a, b)


_FAMILLES = {
    "jours": "C'est un jour de la semaine.",
    "mois": "C'est un mois de l'année.",
    "nombres": "Ce mot désigne une quantité, écrite en lettres.",
    "couleurs": "C'est une couleur.",
    "salutations": "C'est une façon de saluer quelqu'un.",
}


@cache
def _reperes() -> dict[str, dict[str, str]]:
    texte = resources.files(vocabulaire.__package__).joinpath("donnees/indices.toml")
    return tomllib.loads(texte.read_text("utf-8"))


def _rang(ex: Exercice) -> str:
    famille = ex.donnees["famille"]
    mots = [p.anglais for p in vocabulaire.lexique() if p.famille == famille]
    rang = mots.index(ex.donnees["anglais"]) + 1
    return "1er" if rang == 1 else f"{rang}e"


def _anglais(ex: Exercice) -> list[str]:
    famille = ex.donnees.get("famille", "")
    vers_francais = ex.donnees.get("sens") == vocabulaire.VERS_FRANCAIS
    reponse = ex.reponse
    langue = "français" if vers_francais else "anglais"
    lettres = sum(c.isalpha() for c in reponse)
    forme = ""
    if " " in reponse:
        forme = f", en {len(reponse.split())} mots"
    elif "-" in reponse:
        forme = ", avec un tiret"
    paliers = [
        _FAMILLES.get(famille, "Pense au sens du mot."),
        f"Le mot {langue} commence par « {reponse[0]} » et a {lettres} lettres{forme}.",
    ]
    if famille == "jours":
        paliers.append(f"C'est le {_rang(ex)} jour de la semaine, en comptant à la française.")
    elif famille == "mois":
        paliers.append(f"C'est le {_rang(ex)} mois de l'année.")
    elif famille == "nombres":
        paliers.append("Compte en anglais : one, two, three… jusqu'à retrouver ce nombre.")
    elif famille in ("couleurs", "salutations"):
        repere = _reperes().get(famille, {}).get(ex.donnees["anglais"])
        if repere:
            paliers.append(repere[0].upper() + repere[1:] + ".")
    if lettres >= 6:
        paliers.append(
            f"Le mot {langue} commence par « {reponse[:2]} » et finit par « {reponse[-1]} »."
        )
    return paliers


def _candidats(ex: Exercice) -> list[str]:
    d = ex.donnees
    if ex.competence == vocabulaire.competence:
        return _anglais(ex)
    if d.get("famille") in ("doubles", "moities") and "nombre" in d:
        return _doubles_moities(ex)
    if "expression" in d:
        return _calcul(d["expression"])
    if "dividende" in d and "diviseur" in d and not d.get("reste"):
        return _division(Fraction(d["dividende"]), Fraction(d["diviseur"]))
    return []


def indices(ex: Exercice) -> tuple[str, ...]:
    """Les paliers d'indices de `ex`, du plus vague au plus précis. Jamais vide."""
    try:
        candidats = _candidats(ex)
    except (ErreurExpression, KeyError, ValueError, ZeroDivisionError):
        candidats = []
    paliers = tuple(
        p for p in candidats if len(p) <= LONGUEUR_MAX and not contient_la_reponse(p, ex)
    )
    return paliers[:4] or (SECOURS,)
