---
name: nouveau-jeu
description: Ajoute une mise en scène Mine de rien dans Minecraft (comme le coffre du soir ou Bartholomé) dans src/mine_de_rien/games/ : placement, commandes RCON échappées, détection de réussite, récompense, nettoyage, garde-fous contre le contournement par l'enfant, rapport aux parents, compatibilité Bedrock via Geyser. À utiliser dès qu'on ajoute ou modifie un jeu, y compris pendant une phase de /next.
---

# Ajouter un jeu

Un jeu prend un `Exercice` et le rend jouable dans le monde de l'enfant : il
pose des blocs, des panneaux, des entités, il attend la réussite, il récompense,
il range. Le défaut à éviter est le même qu'au premier jour du projet : une
barrière-quiz devant une récompense. Le calcul doit être un **moyen** d'obtenir
quelque chose que l'enfant voulait déjà. Relire `docs/idees.md` avant de
commencer.

## Règles qui ne se discutent pas

- **Aucun texte libre interpolé brut dans une commande.** Énoncé, habillage du
  LLM, message d'un parent : tout passe par une fonction d'échappement unique
  (composant texte sérialisé, guillemets et antislashs échappés, longueur
  bornée). Une apostrophe dans « l'élève » ne doit pas casser la commande, et un
  texte malveillant ne doit pas en fabriquer une autre.
- **Le jeu reçoit un `Exercice`, il ne le fabrique pas.** Aucune réponse calculée
  dans `games/`. Le jeu ne connaît la réponse que pour la poser comme clé ou la
  comparer.
- **Il ne touche que sa zone.** Coordonnées fournies par la configuration ou
  choisies près de la base, jamais sur une construction de l'enfant. Vérifier
  que les blocs visés sont de l'air avant de poser (`execute if block`).
- **Il range derrière lui.** Après réussite ou expiration, panneaux, coffres et
  entités du jeu disparaissent ou deviennent décoratifs. Un monde jonché de vieux
  coffres d'exercice, c'est un monde que l'enfant n'aime plus.
- **Rien ne sort de la maison** au-delà de ce que la spec a prévu.

## 1. Écrire la mécanique sur papier d'abord

Dans la docstring du module `src/mine_de_rien/games/<slug>.py`, avant le code :

- **Le désir.** Ce que l'enfant obtient et pourquoi il le veut.
- **Les commandes**, dans l'ordre, avec la version de Minecraft sur laquelle
  elles ont été vérifiées.
- **La détection de réussite** : quel état du monde ou quel message prouve que
  c'est résolu, et à quelle fréquence on l'interroge. Sans plugin, les sources
  possibles sont la sortie des commandes RCON (`data get`, `execute if`,
  `clear` avec un compte de 0) et le log serveur (chat, connexions).
- **La récompense**, proportionnée : quelques diamants ou émeraudes, un
  déblocage, de la bordure de monde. Jamais de quoi rendre le reste du jeu
  inutile, sinon la récompense suivante ne vaut plus rien.
- **Ce qui part dans le rapport aux parents** : réussite, temps, nombre
  d'essais si on peut le voir, compétence concernée.

## 2. Passer la liste du contournement

Pour chaque ligne, écrire dans la docstring comment le jeu résiste, ou pourquoi
ce n'est pas grave :

- **Casser** le bloc ou tuer l'entité qui porte la récompense (zone en mode
  aventure, entité invulnérable, ou récompense donnée seulement à la réussite).
- **Les droits d'opérateur et le mode créatif.** L'enfant ne doit pas être
  opérateur sur le serveur réel : `/give`, `/data get` ou le créatif rendent
  tout jeu inutile. Le dire dans le rapport si la configuration ne le garantit
  pas.
- **Répondre au hasard** : borner les essais, ou ralentir après plusieurs
  échecs, quand la mécanique permet de les compter.
- **Lire la réponse** quelque part : écran de débogage, données du bloc, chat
  de l'agent, fichier sur le disque.
- **Faire résoudre par quelqu'un d'autre** sur un serveur multijoueur : pas
  grave en soi, mais le rapport ne doit pas prétendre le contraire.

## 3. Bedrock via Geyser

Un enfant sur tablette joue par Geyser. Vérifier que la mécanique y existe :
renommage sur enclume, panneaux, livres, échanges avec les villageois et
messages `tellraw` passent ; les composants Java très récents ou les
interfaces d'inventaire exotiques ne passent pas toujours. Si un point n'est pas
vérifiable sans client Bedrock, le dire dans le rapport au lieu de le supposer.

## 4. Coder et vérifier

- Le jeu n'appelle le serveur que par l'adaptateur de jeu (`adapters/game/`),
  jamais par un client RCON direct : c'est ce qui permettra un jour un autre
  adaptateur.
- Les tests unitaires portent sur les commandes produites (une liste de chaînes
  à partir d'un `Exercice` fixé) : échappement d'un énoncé avec apostrophes,
  guillemets et accents, coordonnées, nettoyage.
- Chaque commande est ensuite exécutée sur le serveur d'essai avec
  `/essai-en-jeu`, sur la version de Minecraft épinglée dans `.env`. Une
  commande qui n'a tourné que dans un test unitaire n'est pas vérifiée.
- Mettre à jour la ligne du jeu dans le tableau du `README.md` et sa section
  dans `docs/idees.md` si la mécanique a changé en route.
