# Architecture et décisions

## Décisions prises au premier commit

1. **Minecraft Java Edition + serveur Paper auto-hébergé.** Seule combinaison sans compte
   supplémentaire ni abonnement, avec le jeu de commandes le plus riche (tag `Lock`,
   `data merge`, advancements, `worldborder`). Minecraft Education exclue (compte scolaire
   Microsoft obligatoire). Bedrock n'est pas pris en charge, voir plus bas.
2. **Pont sans plugin pour commencer.** Commandes via RCON, chat et connexions lus dans
   `logs/latest.log`. Un plugin Paper viendra seulement si un jeu exige des événements fins
   (ouverture de coffre, pose de bloc). Il est venu pour le son, pas pour les événements :
   voir « Un plugin pour la voix » plus bas.
3. **Le LLM n'est jamais la source de vérité.** Générateurs déterministes par compétence,
   réponse calculée. Le LLM habille l'énoncé et lit les photos du cahier. Un modèle local de
   7 à 8 milliards de paramètres suffit alors.
4. **Quatre adaptateurs.** Jeu, canal, LLM, et depuis la dictée audio
   ([spec](specs/done/2026-11-dictee-audio.md)) la voix. Chaque adaptateur externe a une
   alternative auto-hébergée : Telegram → Matrix ou page web locale ; Anthropic → API
   compatible OpenAI (llama.cpp, Ollama, LM Studio). La voix n'a que des fournisseurs
   locaux : Piper, dans son propre conteneur (licence GPL, jamais lié au binaire).
5. **Pseudonymisation vers tout LLM distant.** Seul l'alias configuré et le texte de
   l'exercice partent.
6. **SQLite local** pour compétences, tentatives et rapports. Licence MIT.

## Bedrock retiré (7 novembre 2026)

Le premier commit installait Geyser et Floodgate via `MODRINTH_PROJECTS` pour que les
clients Bedrock rejoignent le serveur Java. Retiré pour deux raisons :

- **Le serveur ne démarrait plus.** L'image `itzg/minecraft-server` télécharge les plugins
  sur Modrinth et s'arrête si l'un d'eux manque. Au 7 novembre 2026, Floodgate n'y est
  publié pour aucune version de Paper, et Geyser s'arrête à Minecraft 26.1.2 alors que
  `LATEST` sert la 26.3.
- **Personne n'en a besoin aujourd'hui.** L'enfant pour qui le projet est né joue en Java.

Pour le réactiver, il faudra télécharger Geyser et Floodgate depuis le site de GeyserMC
(variable `PLUGINS` de l'image plutôt que `MODRINTH_PROJECTS`), ouvrir le port
`19132/udp`, épingler une version de Minecraft que Geyser prend en charge, et remettre la
vérification Bedrock dans la skill `/nouveau-jeu`.

## Passage à Rust (10 novembre 2026)

L'agent, écrit en Python jusqu'à la version 0.1.2, est devenu un binaire Rust unique
([spec](specs/done/2026-11-agent-en-rust.md)). Rien ne change pour l'enfant ni pour les
parents : même `config.toml`, même base SQLite, mêmes commandes Telegram, mêmes commandes
envoyées à Minecraft.

- **Pourquoi.** Un service qui tourne en permanence à la maison n'a pas besoin d'un
  interpréteur, d'un gestionnaire de paquets et d'un environnement virtuel dans son image.
  Le mainteneur préfère Rust ; ce n'est pas une affaire de vitesse, l'agent attend le réseau.
- **Ce qui a été gardé tel quel.** Les six décisions ci-dessus, le découpage en modules, le
  client RCON écrit à la main, le schéma de la base et le format de ses dates.
- **La preuve.** Tant que le Python existait, il a écrit dans `tests/donnees/oracle/` ce
  qu'il produisait pour des entrées fixées : commandes Minecraft, messages, rapports,
  conversations entières. Les tests Rust exigent la même chose, au caractère près.
- **Ce que ça change pour qui installe.** L'image ne contient que le binaire (plus de
  `uv run` dans les commandes `docker compose exec`), le datapack est embarqué dedans, et
  elle se construit sans émulation pour amd64 et arm64.
- **Un seul fil.** Runtime `tokio` mono-fil : la base et le verrou du coffre sont partagés
  entre la boucle, Pixel et Telegram sans verrou de fil, comme avec `asyncio`.

## Un plugin pour la voix (11 novembre 2026)

La dictée audio faisait voyager ses sons dans un pack de ressources, que Minecraft ne
recharge qu'à la connexion : l'enfant devait se reconnecter, ou voir son jeu renvoyé vers
le serveur, à chaque nouvelle dictée. Rien ne pouvait être dit à la volée. Depuis la spec
[voix en direct](specs/open/2026-11-voix-en-direct.md) :

- le son passe par le chat vocal de **Simple Voice Chat**, auto-hébergé : un plugin sur le
  serveur Paper, un mod Fabric dans le jeu de l'enfant. L'enfant entend dans la seconde,
  sans écran de chargement ;
- le dépôt gagne un second programme, **en Java** : `plugin/`, le plugin Paper
  `mine-de-rien-voix`. Il joue à un seul joueur le son que l'agent lui désigne. Il ne
  décide rien, n'embarque aucune bibliothèque, n'ouvre aucun port et n'écoute aucun micro.
  L'agent reste un seul crate Rust, un seul binaire ;
- de l'agent au plugin : un fichier de son brut dans un dossier partagé avec le serveur,
  puis une commande RCON réservée à la console (`src/diffusion.rs`). Pas de réseau entre
  les deux ;
- le pack de sons, son service HTTP et le renvoi automatique du jeu
  ont disparu. Le datapack ne joue plus aucun son.

Ce que cela coûte : le jeu de l'enfant n'est plus « vanilla » pour les dictées, et le mod,
le plugin et Minecraft doivent changer de version ensemble
([rejoindre-le-serveur.md](rejoindre-le-serveur.md#quand-minecraft-change-de-version)).
Sans le mod, les coffres de calcul marchent toujours.

## Ordre de construction

| Version | Contenu | Dépendance technique |
|---|---|---|
| 0.1 | Coffre du soir, Telegram, saisie d'exercices, rapport simple ([spec](specs/done/2026-11-coffre-du-soir.md)) ; saisie par photo d'une fiche ([spec](specs/done/2026-11-photo-du-cahier.md)) ; Pixel dans le chat, retour au coffre et indices ([spec](specs/done/2026-11-aide-dans-le-chat.md)) ; mode paisible tant qu'il reste un coffre à faire ([spec](specs/done/2026-11-paisible-pendant-les-coffres.md)) | RCON, datapack, LLM local à vision, lecture du chat dans le log |
| 0.2 | Arbre de progrès comme format unique de rapport, bordure du monde | datapack |
| 0.3 | Bartholomé | lecture du chat dans le log (livrée avec Pixel, 0.1) |
| 0.4 | Chantiers | scan de zone, peut-être plugin Paper |
| 0.5 | Canal web local, adaptateur Bedrock natif si des contributeurs le veulent | |

## Flux d'un devoir

```
parent (Telegram) ──texte ou photo──▶ canal ──▶ orchestrateur
                                                   │ choisit une compétence + un générateur
                                                   │ calcule l'exercice et la réponse
                                                   │ demande au LLM un habillage (optionnel)
                                                   ▼
                                               jeu (RCON) ──▶ panneau + coffre + butin
                                                   ▲
                                               détection de réussite (inventaire, chat)
                                                   │
                                     stockage SQLite ──▶ rapport ──▶ canal ──▶ parents
```
