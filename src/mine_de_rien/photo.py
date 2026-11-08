"""Photo d'une fiche → exercices vérifiés, sans Telegram.

Le LLM local **recopie** les lignes imprimées : calculs et mots anglais. Il ne calcule rien,
et ce qu'il écrit après un `=` est jeté sans être lu. Chaque ligne devient un `Exercice` par
le code existant, ou elle est rejetée avec sa raison :

- « Double de N » et « Moitié de N » deviennent `N x 2` et `N ÷ 2`, recalculés par
  l'analyseur, avec l'énoncé de la fiche (« Double de 17 = ? ») ;
- tout autre calcul passe par l'analyseur d'expressions (`exercises/expression.py`) ;
- un mot passe par le lexique fermé du vocabulaire anglais.

Essais du 2026-11-08 (spec `2026-11-photo-du-cahier`) : une page droite est bien lue, mais
une double page envoyée d'un coup fait inventer des lignes (« Moitié de 3, 5, 7… ») ou en
fait sauter. Une image plus large que haute est donc coupée en deux moitiés qui se
recouvrent, lues séparément ; les doublons sont retirés. Le modèle ne sait pas dire dans quel
sens est une photo : le redressement est l'affaire du parent.

Les photos ne sont jamais écrites sur le disque, et ne partent que vers un LLM local.
"""

from __future__ import annotations

import io
import random
import re
from dataclasses import dataclass, field

from PIL import Image, ImageOps

from mine_de_rien.adapters.llm.base import ErreurLlm, Llm, lire_json
from mine_de_rien.exercises.anglais import vocabulaire
from mine_de_rien.exercises.base import Exercice, normaliser
from mine_de_rien.exercises.expression import ErreurExpression, calculer

COTE_MAX = 1536
MOITIES = ((0.0, 0.53), (0.47, 1.0))
COMPETENCE_DOUBLES = "maths/doubles-moities"
HORS_LEXIQUE = "hors lexique"

SYSTEME = (
    "Tu transcris fidèlement des fiches d'exercices photographiées. Tu ne calcules rien, tu "
    "ne prolonges aucune suite, tu ne remets rien dans un ordre logique, tu ignores "
    "l'écriture manuscrite. Tu ne recopies que ce qui est imprimé et visible."
)
# Aucun exemple chiffré ni mot d'exemple : sur une photo qu'il ne lit pas, le modèle recopie
# les exemples de la consigne comme s'ils étaient sur la fiche (essai du 2026-11-08).
CONSIGNE = (
    "Recopie chaque ligne de calcul imprimée (opérations, tables, lignes « Double de … » ou "
    "« Moitié de … ») et chaque mot anglais imprimé, dans l'ordre de la photo. Pour un "
    "calcul, recopie seulement ce qui est avant le signe =. S'il n'y a rien de tel, renvoie "
    "une liste vide. Réponds en JSON : "
    '{"lignes": [{"genre": "calcul" ou "mot", "texte": "..."}]}'
)

_DOUBLE = re.compile(r"(double|moitie)\s*(?:de|d')\s*(\d+(?:[.,]\d+)?)", re.IGNORECASE)
_MILLIERS = re.compile(r"(?<=\d)[   ](?=\d{3}(?!\d))")
_TIRETS = str.maketrans({"–": "-", "—": "-", "−": "-"})


class ErreurPhoto(ValueError):
    """Photo illisible ; le message s'adresse au parent."""


@dataclass(frozen=True)
class Rejet:
    texte: str
    raison: str


@dataclass
class Lecture:
    exercices: list[Exercice] = field(default_factory=list)
    rejets: list[Rejet] = field(default_factory=list)


def preparer(octets: bytes) -> list[bytes]:
    """Redresse selon l'EXIF, coupe une double page en deux, réduit et encode en JPEG."""
    try:
        image = ImageOps.exif_transpose(Image.open(io.BytesIO(octets))).convert("RGB")
    except (OSError, ValueError) as e:
        raise ErreurPhoto("Je n'arrive pas à ouvrir cette image.") from e
    largeur, hauteur = image.size
    morceaux = [image]
    if largeur > hauteur:
        morceaux = [
            image.crop((int(debut * largeur), 0, int(fin * largeur), hauteur))
            for debut, fin in MOITIES
        ]
    sorties = []
    for morceau in morceaux:
        morceau.thumbnail((COTE_MAX, COTE_MAX))
        tampon = io.BytesIO()
        morceau.save(tampon, format="JPEG", quality=85)
        sorties.append(tampon.getvalue())
    return sorties


def _simplifier(texte: str) -> str:
    """Accents retirés pour reconnaître « Moitié », espaces simples, sans toucher aux nombres."""
    sans_accents = texte.replace("é", "e").replace("É", "E")
    return re.sub(r"\s+", " ", sans_accents).strip()


def _calcul(texte: str) -> Exercice:
    texte = texte.split("=", 1)[0].translate(_TIRETS).strip()
    texte = _MILLIERS.sub("", texte)
    m = _DOUBLE.fullmatch(_simplifier(texte))
    if m:
        operation = normaliser(m.group(1))
        nombre = m.group(2)
        ex = calculer(f"{nombre} x 2" if operation == "double" else f"{nombre} ÷ 2")
        mot = "Double" if operation == "double" else "Moitié"
        return Exercice(
            competence=COMPETENCE_DOUBLES,
            enonce=f"{mot} de {nombre} = ?",
            reponse=ex.reponse,
            donnees={"famille": "doubles" if operation == "double" else "moities",
                     "nombre": nombre},
        )
    ex = calculer(texte)
    return Exercice(ex.competence, ex.enonce, ex.reponse, {**ex.donnees, "famille": "calculs"})


def interpreter(reponses: list[dict], rng: random.Random) -> Lecture:
    """Réponses JSON du LLM (une par morceau d'image) → exercices validés et rejets."""
    lecture = Lecture()
    vus: set[str] = set()
    for reponse in reponses:
        lignes = reponse.get("lignes")
        if not isinstance(lignes, list):
            raise ErreurLlm("Le LLM n'a pas renvoyé de liste de lignes.")
        for ligne in lignes:
            if not isinstance(ligne, dict) or not isinstance(ligne.get("texte"), str):
                lecture.rejets.append(Rejet(str(ligne)[:60], "ligne mal formée"))
                continue
            texte = ligne["texte"].strip()[:80]
            if not texte:
                continue
            if ligne.get("genre") == "mot":
                ex = vocabulaire.exercice_pour(texte, rng)
                if ex is None:
                    lecture.rejets.append(Rejet(texte, HORS_LEXIQUE))
                    continue
                cle = "mot:" + ex.donnees["anglais"]
            else:
                try:
                    ex = _calcul(texte)
                except ErreurExpression as e:
                    lecture.rejets.append(Rejet(texte, str(e)))
                    continue
                cle = "calcul:" + ex.enonce
            if cle not in vus:
                vus.add(cle)
                lecture.exercices.append(ex)
    return lecture


async def lire(
    llm: Llm, octets: bytes, consigne_parent: str = "", rng: random.Random | None = None
) -> Lecture:
    """Lit une photo avec le LLM local. Refuse un LLM distant sans l'appeler."""
    if llm.distant:
        raise ErreurLlm(
            "La lecture de photo demande un LLM local ([llm] adaptateur = "
            "\"openai-compatible\") : une photo ne part jamais vers un LLM distant."
        )
    consigne = CONSIGNE
    if consigne_parent.strip():
        consigne += f" Ne regarde que cette partie de la page : {consigne_parent.strip()[:100]}."
    reponses = []
    for morceau in preparer(octets):
        texte = await llm.completer(SYSTEME, consigne, image=morceau, en_json=True)
        reponses.append(lire_json(texte))
    return interpreter(reponses, rng or random.Random())


def tirer(lecture: Lecture, n: int, rng: random.Random) -> list[Exercice]:
    """N exercices au hasard, sans remise, dans l'ordre de la fiche."""
    n = min(n, len(lecture.exercices))
    indices = sorted(rng.sample(range(len(lecture.exercices)), n))
    return [lecture.exercices[i] for i in indices]


_NOMS_FAMILLES = {
    "doubles": "Doubles de",
    "moities": "Moitiés de",
    "calculs": "Calculs",
    "jours": "Jours",
    "mois": "Mois",
    "nombres": "Nombres",
    "couleurs": "Couleurs",
    "salutations": "Salutations",
}


def _etiquette(ex: Exercice) -> str:
    if "nombre" in ex.donnees:
        return ex.donnees["nombre"]
    if "anglais" in ex.donnees:
        return ex.donnees["anglais"]
    return ex.donnees.get("expression", ex.enonce)


def resumer(lecture: Lecture) -> list[str]:
    """Ce qui a été lu, groupé par famille, puis les rejets : pour que le parent voie un trou."""
    groupes: dict[str, list[str]] = {}
    for ex in lecture.exercices:
        groupes.setdefault(ex.donnees.get("famille", "calculs"), []).append(_etiquette(ex))
    lignes = [f"J'ai lu {len(lecture.exercices)} exercice(s) :"]
    for famille, etiquettes in groupes.items():
        nom = _NOMS_FAMILLES.get(famille, famille.capitalize())
        lignes.append(f"• {nom} ({len(etiquettes)}) : {', '.join(etiquettes)}")
    # Un mot hors lexique est le plus souvent un titre ou une consigne : on le compte. Un calcul
    # écarté peut cacher une ligne mal lue : on le montre.
    mots = [r for r in lecture.rejets if r.raison == HORS_LEXIQUE]
    calculs = [r for r in lecture.rejets if r.raison != HORS_LEXIQUE]
    if calculs:
        lignes.append(f"Calculs écartés ({len(calculs)}) :")
        lignes += [f"• « {r.texte} » : {r.raison}" for r in calculs[:10]]
        if len(calculs) > 10:
            lignes.append(f"• … et {len(calculs) - 10} autre(s)")
    if mots:
        lignes.append(f"{len(mots)} texte(s) hors lexique ignoré(s) (titres, consignes…).")
    return lignes
