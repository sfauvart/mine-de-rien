# Idées fondatrices

Issues d'une séance de provocations latérales (opérateur PO, Edward de Bono, 1970),
le 7 novembre 2026. Toutes retenues.

## Ce qu'on a refusé d'emblée

Le PNJ professeur qui pose des questions dans le chat, le quiz contre diamants, la salle de
classe à panneaux, le bot « envoie la photo du cahier », le tableau de bord de scores.
Défaut commun : une barrière-quiz devant la récompense, qu'un enfant de 10 ans identifie
comme un devoir déguisé en moins d'une semaine. Les idées ci-dessous cherchent des formes où
le calcul est un moyen d'obtenir ce qu'il voulait déjà.

## 1. Coffre du soir (distorsion de lieu : le devoir est dans le butin)

Chaque soir, un coffre verrouillé apparaît près de la base. Un coffre portant le tag `Lock`
ne s'ouvre qu'en tenant un objet dont le nom est la clé, et la clé est la réponse. Un panneau
à côté porte l'énoncé envoyé depuis Telegram. L'enfant renomme un objet sur une enclume.

```
/setblock X Y Z oak_sign{front_text:{messages:['"Le fermier a 348 œufs"','"en boîtes de 12."','"Combien de boîtes ?"','""']}}
/data merge block X Y Z+1 {Lock:"29"}
/item replace block X Y Z+1 container.13 with diamond 3
```

Détection : le coffre contient un papier nommé « Preuve n°N » ; l'agent interroge l'inventaire
et notifie Telegram quand il apparaît. Limites : un coffre verrouillé se casse et lâche son
contenu, donc zone en mode aventure ou variante marchand dont l'échange exige l'objet renommé.
On ne voit pas les tentatives ratées, seulement le temps de résolution.

## 2. Arbre de progrès (souhait : le cahier de textes et l'arbre des progrès sont le même objet)

Datapack d'advancements, un nœud par attendu de fin de CM2, déclencheur `impossible`, accordé
par commande. Trois cadres : vu une fois, validé deux fois, revalidé en révision mélangée deux
semaines plus tard (répétition espacée gratuite). Le rapport parents est le JSON de progrès du
joueur, parsé et renvoyé sur Telegram : l'enfant et les parents regardent le même arbre.
Limite : la taxonomie est le vrai travail.

## 3. Bartholomé l'apprenti (renversement : l'enfant corrige l'IA)

Villageois nommé « Bartholomé, apprenti » dont les marchés reposent sur des affirmations,
vraies ou fausses. L'enfant gagne s'il attrape l'erreur **et** tape la correction dans le chat.
Les erreurs laissées passer, classées par compétence, sont le rapport « à retravailler ».
Ratio vrai/faux équilibré obligatoire. En anglais, Bartholomé ne comprend que l'anglais.

## 4. Bordure du monde (exagération : la récompense est le monde)

Bordure initiale de 100 × 100 ; chaque exercice validé l'agrandit de 20 blocs, animé sur
10 secondes. Le mur rouge est la liste de choses à faire. La bordure ne rétrécit jamais.
Limite : passé 2 000 blocs la valeur marginale s'effondre, donc un trimestre, pas une année.

## 5. Chantiers (échappement : l'agent ne pose jamais de question)

« Je veux un enclos rectangulaire de 24 blocs d'aire et je n'ai que 20 barrières. »
Validation par scan de la zone et remplissage pour mesurer aire et périmètre réels.
Plusieurs solutions valides sont un avantage. Limite : validation des formes non
rectangulaires et retour précis sur un enclos presque fermé.
