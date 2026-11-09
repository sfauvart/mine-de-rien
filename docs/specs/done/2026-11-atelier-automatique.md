# Un parent tape /atelier, et l'atelier se construit près de l'enfant, sur un sol valide

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

## Pourquoi

L'emplacement de l'atelier est aujourd'hui relevé à la main : aller dans le jeu, appuyer sur
F3, recopier x, y, z dans `config.toml`, redémarrer l'agent, lancer une commande
(`docs/installation.md:159-187`). Pour un parent qui ne connaît pas Minecraft, c'est l'étape
la plus fragile de l'installation.

Observé à l'essai du 2026-11-09, sur un serveur au relief normal :

- `config.toml` portait encore les coordonnées du monde plat d'essai (`0 -60 8`). Le coffre a
  été posé dans la roche, et Pixel y a emmuré le joueur qui demandait à y aller. Le correctif
  `9ab0246` empêche désormais de poser ou de téléporter, mais il faut toujours trouver les
  bonnes coordonnées.
- Après avoir corrigé `config.toml` et construit l'atelier, les coffres sont restés bloqués :
  `lancer` garde les coordonnées lues à son démarrage (`src/mine_de_rien/cli.py:95-98`).
  Le monde sait pourtant où est l'atelier : `construire` y écrit sa position
  (`src/mine_de_rien/games/atelier.py:174`).

**Cible** : le parent tape `/atelier` sur Telegram pendant que l'enfant est à sa base ; l'agent
trouve tout seul un emplacement valide autour de lui, y construit l'atelier, et plus aucune
coordonnée n'est à relever ni à recopier.

## Décisions actées

| Sujet | Décision |
|---|---|
| Déclencheur | La commande Telegram `/atelier`, réservée aux parents, pendant que l'enfant est connecté dans l'Overworld. |
| Point de départ | La position de l'enfant au moment de la commande. |
| Source de vérité | **Le monde.** `lancer` relit à chaque tour la position enregistrée dans `mine_de_rien:atelier`. Plus de redémarrage après un changement. |
| Coordonnées fixes | `[atelier]` reste facultatif, seulement pour la commande `mine-de-rien atelier` (serveur d'essai, emplacement précis voulu). `lancer` ne le lit plus. |
| Déplacement | `/atelier` relancé déplace l'atelier, seulement sans coffre actif et avec un coffre vide. |
| Terrain (hypothèse) | 5 × 5 colonnes **exactement au même niveau**. Aucun terrassement : la règle « rien n'est cassé » de `atelier.py` ne change pas, et `verifications_de_place` est réutilisée telle quelle. |
| Recherche (hypothèse) | Centres testés par couronnes, du plus proche au plus loin, de 8 à 32 blocs de l'enfant (distance de Tchebychev). Le premier centre valide gagne : ordre déterministe, pas de hasard. |
| Distance minimale (hypothèse) | 8 blocs : le 5 × 5 ne tombe pas sur l'enfant, et il reste hors de la zone en mode aventure (6 blocs autour du marqueur), qui l'empêcherait sinon de casser des blocs là où il se trouve. |
| Écart vertical (hypothèse) | Surface à 16 blocs au plus de l'enfant : au fond d'une mine, l'agent refuse au lieu de construire loin au-dessus de sa tête. |
| Échec | Rien n'est construit. Les parents reçoivent la raison et le geste à faire (enfant hors ligne, dans une autre dimension, sous terre, pas de terrain plat assez proche). |
| Ce que voit l'enfant | Une ligne dans le chat, à la construction : à combien de blocs et dans quelle direction, calculé en Python. |
| Ce que voient les parents | Un accusé (« Je cherche un emplacement… »), puis le résultat avec les coordonnées et la distance, jamais le pseudo. |

### Ce que ces décisions renversent

- `docs/specs/done/2026-11-coffre-du-soir.md:42` : l'atelier n'est plus construit « aux
  coordonnées données dans `config.toml` », sauf choix explicite.
- `src/mine_de_rien/games/atelier.py:214` : « Le déplacer n'est pas prévu » reste vrai pour la
  commande en ligne, plus pour `/atelier`.
- `src/mine_de_rien/cli.py:96-103` : sans section `[atelier]`, `lancer` ne pose aucun coffre.
  Il en posera dès qu'un atelier existe dans le monde.
- Le second message de `probleme_atelier` (« l'agent attend… redémarrer l'agent »), ajouté par
  le correctif, disparaît : ce décalage ne peut plus exister.

## Hors périmètre

- Aplanir ou combler le terrain : écarté, cela casserait des blocs naturels sans l'accord de personne.
- Reconnaître une construction posée sur du sol naturel : une terrasse en terre de l'enfant passe pour du sol, comme aujourd'hui.
- Laisser l'enfant déclencher ou déplacer l'atelier : commande réservée aux parents.
- Proposer plusieurs emplacements au choix : un seul, et `/atelier` relancé si on veut changer.
- Rendre au sol ses blocs d'origine après un déplacement : ils ne sont pas connus.
- Lancer la recherche sans joueur connecté (point d'apparition) : écarté, potentiellement loin de la base.
- Lire « not loaded » autrement que « coffre absent » (`coffre_du_soir.py:305`) : avec un atelier construit, le `forceload` garde le chunk chargé.

## Vie privée

Rien de nouveau ne sort de la maison. La position de l'enfant reste entre l'agent et le serveur
Minecraft, et aucun LLM n'intervient. Telegram reçoit les coordonnées de l'atelier et la distance
à l'enfant, jamais son pseudo ni sa position.

## Statut

**Branche** : `feat/atelier-automatique`.

- [x] **Phase 1 : Le monde fait foi**
- [x] **Phase 2 : Trouver un emplacement**
- [x] **Phase 3 : /atelier sur Telegram**
- [x] **Phase 4 : Déplacer l'atelier**

Close le 2026-11-09.

Chaque phase dépend de la précédente. La phase 1 est utile seule : elle supprime le blocage après
un changement de coordonnées.

## Conception

Les commandes de cette spec ont été exécutées sur Paper 26.3 avec `/essai-en-jeu`. Sans joueur
sur le serveur d'essai, `data get entity <pseudo> Pos` et le sélecteur `@a[name=…,distance=0..]`
n'ont été vérifiés que sur un marqueur : le test en jeu de la phase 3 les confirme avec un joueur.

### Phase 1 : Le monde fait foi

**Contexte.** `CoffreDuSoir` et `Pixel` reçoivent un `Atelier` figé à leur création
(`coffre_du_soir.py:211`, `pixel.py:199`), construit depuis `config.toml` au démarrage de
`lancer`. `position_enregistree` (`atelier.py:174`) sait déjà lire la position du monde.

**Fichiers.** `src/mine_de_rien/games/coffre_du_soir.py`, `games/pixel.py`, `games/atelier.py`,
`orchestrateur.py`, `cli.py`, `config.example.toml`, `tests/test_coffre_du_soir.py`,
`tests/test_pixel.py`, `tests/test_rapport.py`.

**Conception.**
- `CoffreDuSoir` ne reçoit plus d'atelier. Une méthode `actualiser_atelier()` lit
  `position_enregistree` et garde un `Atelier` ou `None`, sans lever d'erreur.
- `Orchestrateur.tour` l'appelle au début de chaque tour, `/annuler` avant de retirer un coffre.
  Toute méthode qui a besoin de la position lève `AtelierAbsent` quand il n'y en a pas.
- `Pixel` lit la position au moment de téléporter.
- `lancer` crée toujours le coffre du soir et Pixel, avec ou sans `[atelier]`.
- `mine-de-rien atelier` continue de lire `[atelier]`, et dit quoi faire si la section manque.
- `probleme_atelier` se réduit au cas « pas construit ».

**Vérification automatique.**
- `uv run pytest` et `uv run ruff check` passent.
- Nouveaux tests : position changée entre deux tours, le coffre suivant est posé au nouvel
  endroit sans recréer l'orchestrateur ; aucune position dans le monde, parents prévenus une fois.
- `/essai-en-jeu` : `lancer` avec un `config.essai.toml` sans `[atelier]`, sur le monde d'essai où
  l'atelier existe, pose bien le coffre en `0 -60 8` (état vérifié avec `execute if block`).

**Vérification en jeu.**
1. Sur un monde où l'atelier existe, retirer la section `[atelier]` de `config.toml`, puis
   `docker compose restart agent`.
2. Sur Telegram : `/coffre 12 x 7`. Le coffre apparaît à l'atelier, comme avant.
3. Dans le chat du jeu : « Pixel, ramène-moi au coffre ». On arrive sur le plancher, devant le coffre.

### Phase 2 : Trouver un emplacement

**Contexte.** Il faut connaître la surface d'une colonne sans relevé manuel. La sous-commande
`positioned over` d'`execute` place l'exécution juste au-dessus du sol selon une carte de hauteur.
L'agent peut enchaîner beaucoup de commandes : à l'essai du 2026-11-09, `construire` a fait
environ 125 tests de blocs en 32 ms.

**Fichiers.** `src/mine_de_rien/games/emplacement.py` (nouveau), `tests/test_emplacement.py`
(nouveau).

**Conception.** Fonction `chercher(jeu, x, y, z) -> Atelier | None`, sans rien construire
(commandes vérifiées sur Paper 26.3) :

```
summon minecraft:marker <x> <y> <z> {Tags:["mdr_sonde"]}
execute positioned X 0 Z positioned over motion_blocking_no_leaves
    run tp @e[type=minecraft:marker,tag=mdr_sonde,limit=1] ~ ~ ~
kill @e[type=minecraft:marker,tag=mdr_sonde]
```

- La réponse du `tp` donne le Y de la colonne : c'est le niveau du coffre, le plancher étant
  dessous. La sonde, distincte de `mdr_atelier`, n'active pas la zone du datapack. Elle est
  retirée même en cas d'erreur.
- Hauteurs gardées en cache par colonne : une couronne réutilise celles de la précédente.
- Un centre est retenu si ses 25 colonnes sont au même niveau, à 16 blocs au plus de l'enfant,
  et si `verifications_de_place(a, False)` passe : l'eau, les arbres et les constructions sont
  écartés par les listes `LIBRE` et `SOL` existantes.
- Un chunk non chargé (« not loaded ») écarte le centre : autour d'un joueur connecté la zone est
  chargée, la recherche n'a donc pas besoin de `forceload`.
- Vers un chunk non chargé, le serveur répond « That position is not loaded » et la sonde ne
  bouge pas. Pire cas mesuré à l'essai (aucun centre valide) : 1 seconde.

**Vérification automatique.**
- `uv run pytest` : monde simulé avec plat, bosses d'un bloc, eau, arbre, construction, zone trop
  proche, pente à plus de 16 blocs, aucun centre valide. Ordre déterministe, sonde toujours retirée.
- `/essai-en-jeu` : le monde d'essai étant plat, y construire avec `fill` une bosse, un bassin et
  un tronc, puis appeler `chercher` et vérifier que le centre rendu les évite, et qu'aucune entité
  `mdr_sonde` ne reste.

### Phase 3 : /atelier sur Telegram

**Contexte.** `CommandesParents.traiter` (`orchestrateur.py`) distribue les commandes ;
`accuse_reception` envoie un premier message avant un traitement long. `construire` écrit la
position en dernier : la boucle de surveillance ne voit donc pas d'atelier à moitié construit.

**Fichiers.** `src/mine_de_rien/orchestrateur.py`, `games/emplacement.py`, `docs/installation.md`
(étape 7 réécrite), `config.example.toml`, `README.md`, `tests/test_orchestrateur.py`.

**Conception.**
- Position de l'enfant : `data get entity <pseudo> Pos`. Dimension :
  `execute in minecraft:overworld if entity @a[name=<pseudo>,distance=0..]`, le champ `Dimension`
  n'étant pas vérifiable sans joueur connecté, alors que `distance` a été validé sur un marqueur.
- Si un atelier existe déjà, réponse « déjà construit en X Y Z » (remplacé en phase 4).
- Sinon : accusé, `chercher`, `construire`, réponse aux parents, ligne dans le chat de l'enfant
  avec la distance et la direction (nord, sud, est, ouest, calculées en Python).
- Réponses d'échec, chacune avec son geste : enfant hors ligne, autre dimension, sous terre, aucun
  terrain plat à moins de 32 blocs (« emmener l'enfant vers un terrain dégagé, puis relancer »).
- `/atelier` est ajouté à `AIDE`. Le message « pas construit » renvoie vers `/atelier`.
- `pseudo_minecraft` ne figure dans aucune réponse aux parents (même contrôle que `tests/test_rapport.py`).

**Vérification automatique.**
- `uv run pytest` : chaque échec a sa réponse ; un expéditeur inconnu est ignoré ; le pseudo
  n'apparaît dans aucune réponse ; la direction est juste pour les quatre côtés.
- `/essai-en-jeu` : les lectures `data get entity` demandent un joueur, donc elles relèvent de la
  vérification en jeu ; la suite (`chercher` puis `construire` à partir d'une position donnée) est
  jouée sur le serveur d'essai.

**Vérification en jeu.**
1. Repartir d'un monde sans atelier et d'un `config.toml` sans `[atelier]`. Se connecter et
   marcher jusqu'à un endroit dégagé.
2. Sur Telegram : `/atelier`. L'accusé arrive, puis le résultat. Dans le chat du jeu, une ligne
   indique la distance et la direction.
3. Marcher dans cette direction : l'atelier est là, posé sur le sol, avec le coffre, l'enclume,
   le panneau et la lanterne.
4. Sur Telegram : `/coffre 12 x 7`. Dans le chat : « Pixel, ramène-moi au coffre ». On arrive
   devant le coffre, sans être coincé dans un bloc.
5. Se déconnecter, puis taper `/atelier` : réponse « enfant hors ligne », rien n'est construit.

### Phase 4 : Déplacer l'atelier

**Contexte.** Sans cette phase, un emplacement qui déplaît ne se corrige qu'en RCON. Le datapack
remet en survie un joueur qui n'est plus dans une zone (`tick.mcfunction`) : retirer le marqueur suffit.

**Fichiers.** `src/mine_de_rien/games/atelier.py`, `orchestrateur.py`, `docs/installation.md`,
`tests/test_atelier.py`, `tests/test_orchestrateur.py`.

**Conception.** `/atelier` relancé quand un atelier existe :
1. Refus si un coffre est actif (« annuler avec /annuler ou attendre qu'il soit ouvert »), ou si
   le coffre de l'atelier contient encore des objets : on ne détruit jamais une récompense oubliée.
2. `chercher` d'abord : si rien ne convient, l'ancien atelier ne bouge pas.
3. Retrait, dans cet ordre : coffre, panneau, enclume et lanterne remplacés par de l'air, plancher
   remplacé par de la terre (hypothèse), marqueur, `forceload remove`, puis
   `data remove storage mine_de_rien:atelier pos`.
4. `construire` au nouvel endroit, qui écrit la position en dernier. En cas d'interruption après
   l'étape 3, le monde n'a plus d'atelier et `/atelier` en reconstruit un.

Un verrou (`CoffreDuSoir.verrou`), tenu pendant chaque tour de surveillance et pendant `/atelier`,
empêche de poser un coffre à l'ancien endroit en plein déplacement.

Les coffres en file seront posés au nouvel endroit, sans autre geste (phase 1).

**Vérification automatique.**
- `uv run pytest` : refus avec coffre actif, refus avec coffre plein, ancien atelier intact quand
  `chercher` échoue, ordre de retrait.
- `/essai-en-jeu` : après déplacement, plus de coffre ni d'enclume à l'ancien endroit, un seul
  marqueur `mdr_atelier`, l'ancien chunk n'est plus en `forceload` (`forceload query`), et la
  position enregistrée est la nouvelle.

**Vérification en jeu.**
1. Atelier construit, aucun coffre actif, coffre vide. Marcher une cinquantaine de blocs plus loin.
2. Sur Telegram : `/atelier`. Une ligne dans le chat annonce le nouvel atelier.
3. Y aller, puis `/coffre 12 x 7` : le coffre apparaît au nouvel endroit.
4. Revenir à l'ancien emplacement : il ne reste qu'un carré de terre, et on peut y casser des blocs
   (plus de mode aventure).
5. Coffre actif : `/atelier` répond qu'il faut d'abord l'annuler ou l'ouvrir, et rien ne bouge.
