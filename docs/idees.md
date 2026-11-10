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

Chaque soir, un coffre verrouillé apparaît à l'atelier, près de la base. Un coffre portant un
verrou ne s'ouvre qu'en tenant un objet dont le nom est la clé, et la clé est la réponse. Un
panneau à côté porte l'énoncé envoyé depuis Telegram. L'enfant renomme sur une enclume la « Clé du
coffre » que Mine de rien lui donne, dont la description explique les gestes.

Commandes vérifiées sur Minecraft Java 26.3 : le verrou n'est plus la chaîne `Lock` des
anciennes versions, mais un prédicat d'objet `lock`, et les textes s'écrivent en SNBT.

```
/setblock X-1 Y Z oak_sign[rotation=0]{front_text:{messages:["348 ÷ 12 = ?","","",""]},is_waxed:1b}
/data modify block X Y Z lock set value {components:{"minecraft:custom_name":"29"}}
/item replace block X Y Z container.13 with minecraft:diamond 3
/item replace block X Y Z container.11 with minecraft:paper[minecraft:custom_name="Preuve n°12",minecraft:custom_data={mdr_coffre:12}]
```

Détection : le coffre contient un papier « Preuve n°N » marqué d'un `custom_data` ; l'agent
compte ce papier dans l'inventaire de l'enfant avec `clear <pseudo> <papier> 0` et notifie
Telegram quand il apparaît. Limites : un coffre verrouillé se casse et lâche son contenu, d'où
l'atelier fixe et la zone en mode aventure (le marchand a été écarté). On ne voit pas les
tentatives ratées, seulement le temps de résolution. Détails dans
`src/games/coffre_du_soir.rs`.

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

## 6. Dictée de la semaine (ajoutée le 10 novembre 2026 : la dictée se prépare en l'écoutant)

Venue après les cinq autres, pour un enfant dyslexique dont les dictées sont des dictées à
trous. Le parent envoie une fois le texte que la maîtresse donne à préparer ; chaque jour,
quelques coffres du soir attendent l'enfant, dont la clé est un mot du texte. Il entend la
phrase entière, lue par une voix de synthèse locale, la voit dans le chat avec `_____`, et
écrit le mot sur l'enclume. Un objet « Écouteur » relit la phrase d'un clic droit.

Ce n'est pas un nouveau jeu mais une clé de plus pour le coffre du soir (idée 1), qui perd
au passage sa limite « on ne voit pas les tentatives ratées » : l'agent relève ce que
l'enfant écrit sur la clé, et un mot raté revient deux jours plus tard. Règle propre à cette
idée : **jamais une graphie fausse sous ses yeux**, ni la sienne, ni celle d'un LLM, d'où
Bartholomé (idée 3) écarté pour l'orthographe. Détails dans
`src/games/dictee.rs` et [la spec](specs/open/2026-11-dictee-audio.md).
