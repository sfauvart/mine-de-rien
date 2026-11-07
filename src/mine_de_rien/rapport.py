"""Rapport de la semaine pour les parents, demandé avec `/rapport`.

Sur les 7 derniers jours, par compétence : coffres posés, ouverts, durée médiane entre la
pose et l'ouverture. Puis une ligne « à retravailler » par exercice resté fermé plus de
48 heures ou ouvert en plus de 30 minutes. Les coffres annulés ne comptent pas.

Le rapport ne contient jamais le pseudo ni le prénom de l'enfant.
"""

from __future__ import annotations

import statistics
from collections import defaultdict
from datetime import datetime, timedelta

from mine_de_rien.games.coffre_du_soir import formater_duree
from mine_de_rien.storage.sqlite import Depot, ExerciceStocke

PERIODE = timedelta(days=7)
FERME_TROP_LONGTEMPS = timedelta(hours=48)
OUVERT_LENTEMENT = timedelta(minutes=30)


def a_retravailler(s: ExerciceStocke, quand: datetime) -> str | None:
    """La raison pour laquelle cet exercice est à retravailler, ou None."""
    if s.statut == "pose" and s.pose_le and quand - s.pose_le > FERME_TROP_LONGTEMPS:
        return f"toujours fermé après {formater_duree(quand - s.pose_le)}"
    if s.statut == "ouvert" and s.duree is not None and s.duree > OUVERT_LENTEMENT:
        return f"ouvert en {formater_duree(s.duree)}"
    return None


def rapport(depot: Depot, quand: datetime) -> str:
    debut = quand - PERIODE
    poses = [s for s in depot.poses_entre(debut, quand) if s.statut != "annule"]
    disparus = [e for e in depot.evenements_entre(debut, quand) if e.genre == "coffre_disparu"]
    en_file = len(depot.en_file())

    lignes = [f"Rapport des 7 derniers jours (du {debut:%d/%m} au {quand:%d/%m})"]
    if not poses:
        lignes.append("Aucun coffre posé.")
    par_competence: dict[str, list[ExerciceStocke]] = defaultdict(list)
    for s in poses:
        par_competence[s.exercice.competence].append(s)
    for competence, liste in sorted(par_competence.items()):
        ouverts = [s for s in liste if s.statut == "ouvert" and s.duree is not None]
        ligne = f"• {competence} : {len(liste)} posé(s), {len(ouverts)} ouvert(s)"
        if ouverts:
            mediane = statistics.median(s.duree.total_seconds() for s in ouverts)
            ligne += f", durée médiane {formater_duree(timedelta(seconds=mediane))}"
        lignes.append(ligne)

    retravailler = [(s, raison) for s in poses if (raison := a_retravailler(s, quand))]
    if retravailler:
        lignes.append("")
        lignes.append("À retravailler :")
        for s, raison in retravailler:
            lignes.append(f"• n°{s.id} {s.exercice.enonce} : {raison}")

    if disparus:
        lignes.append("")
        lignes.append(f"Coffres disparus puis reposés : {len(disparus)}")
    if en_file:
        lignes.append(f"En attente dans la file : {en_file}")
    return "\n".join(lignes)
