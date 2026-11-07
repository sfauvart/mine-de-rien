# Un parent envoie un exercice depuis Telegram, l'enfant ouvre un coffre en le résolvant

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

## Pourquoi

Mine de rien n'a encore aucun chemin de bout en bout. Les trois pièces qui le
porteraient sont des coquilles vides :

- l'adaptateur de jeu lève `NotImplementedError` pour toute commande
  (`src/mine_de_rien/adapters/game/java_rcon.py:22` et `:25`) ;
- le canal Telegram aussi (`src/mine_de_rien/adapters/channel/telegram.py:20` et `:23`) ;
- le coffre du soir n'existe qu'en docstring (`src/mine_de_rien/games/coffre_du_soir.py:1-12`),
  et la CLI se contente d'annoncer que l'orchestrateur manque (`src/mine_de_rien/cli.py:24`).

Les commandes de verrouillage notées dans `docs/idees.md:22` et
`src/mine_de_rien/games/coffre_du_soir.py:6` utilisent le tag `Lock` sous forme de
chaîne. Il a très probablement été remplacé par un prédicat d'objet `lock` dans
les versions récentes de Java, et les composants texte des panneaux ont eux aussi
changé de format. Rien de ce qui est écrit n'a tourné sur un serveur.

Le seul exercice existant est la division (`src/mine_de_rien/exercises/maths/division.py`).
Il ne couvre pas « recopier l'exercice du cahier », qui est l'usage quotidien des
parents.

**Cible** : un parent tape `/coffre 348 ÷ 12` ou `/coffre division 3` dans
Telegram ; un coffre verrouillé apparaît à l'atelier, à côté de la base de
l'enfant, avec l'énoncé sur un panneau ; l'enfant renomme un objet « 29 » sur
l'enclume, ouvre le coffre, récupère sa récompense ; les deux parents reçoivent
« Coffre n°12 ouvert » et peuvent demander un rapport de la semaine.

## Décisions actées

| Sujet | Décision |
|---|---|
| Plateforme | Minecraft **Java Edition** sur PC. Bedrock n'est pas vérifié dans cette spec. |
| Version | `MC_VERSION` épinglée dans `.env` à la version sur laquelle `/essai-en-jeu` a validé les commandes, en phase 1. Le lanceur de l'enfant doit sélectionner la même version. |
| Saisie | Deux formes, une seule commande `/coffre`. Une expression (`348 ÷ 12`, `12 x 7`, `3,5 + 1,25`) est recalculée par Mine de rien. Sinon le texte est un nom de compétence suivi d'un nombre d'exercices (`division 3`). |
| Calcul d'une expression | Analyseur dédié, quatre opérations, entiers et décimaux, parenthèses. Jamais `eval`. Calcul exact en `Fraction`. |
| Réponses acceptées | Entier, ou décimal à deux chiffres après la virgule au plus, écrit **avec une virgule**. Une expression dont le résultat n'est pas de cette forme (`10 ÷ 3`) est refusée avec un message clair au parent. |
| Lieu | Un **atelier fixe**, construit par Mine de rien aux coordonnées données dans `config.toml`, près de la base de l'enfant. |
| Protection | Un petit datapack met l'enfant en mode **aventure** quand il entre à moins de 6 blocs de l'atelier, et le remet en survie quand il en sort. En aventure, il ne peut ni casser le coffre ni poser d'entonnoir dessous. |
| Coût d'un essai | Renommer sur une enclume coûte un niveau d'expérience. Mine de rien donne **3 niveaux** à chaque nouveau coffre. Au-delà, chaque essai au hasard se paie avec son expérience à lui : c'est la limite contre les réponses au hasard. |
| Un coffre à la fois | Un seul coffre actif. Les exercices suivants attendent dans une file et apparaissent quand le précédent est ouvert. |
| Énoncé | Panneau, quatre lignes courtes, énoncé neutre. Aucun LLM dans cette spec. |
| Récompense | Valeur par défaut dans `config.toml` (3 diamants), identique pour tous les coffres. |
| Détection | L'agent interroge le serveur par RCON toutes les 3 secondes quand l'enfant est connecté : présence dans `list`, puis présence dans son inventaire d'un papier « Preuve » marqué par un `custom_data` propre au coffre. |
| Canal | Telegram, réservé aux identifiants listés dans `canal.parents`. `/start` répond à n'importe qui avec son propre identifiant, et rien d'autre. |

### Ce que ces décisions renversent

- `docs/architecture.md` place un « premier habillage par le LLM » dans la
  version 0.1. Il est repoussé : la 0.1 ne dépend d'aucun LLM.
- `docs/idees.md` décrit un coffre « près de la base » protégé par une zone en
  mode aventure ou un marchand. On tranche pour l'atelier fixe et le mode
  aventure piloté par datapack. Le marchand est écarté.
- La lecture du log serveur, prévue pour le chat en 0.3, n'est pas utilisée ici :
  `list` par RCON suffit à savoir si l'enfant est connecté.

## Hors périmètre

- Photo du cahier lue par un LLM, et tout habillage par un LLM.
- Autres générateurs que la division : chacun viendra par `/nouvel-exercice`.
- Division euclidienne avec reste dans le coffre (la réponse « 29 r 3 » n'a pas
  d'écriture unique assez simple pour un enfant).
- Compter les essais ratés. Piste notée : la baisse des niveaux d'expérience à
  l'atelier.
- Récompense réglable par exercice depuis Telegram.
- Rapport automatique programmé : le rapport est demandé avec `/rapport`.
- Clients Bedrock via Geyser, consoles.
- Arbre de progrès et bordure du monde (version 0.2).
- Plusieurs enfants sur le même serveur.

## Vie privée

- **Vers Telegram** : les énoncés, les réponses, les numéros de coffre et les
  durées. Jamais le pseudo ni le prénom de l'enfant : les messages disent
  « le coffre n°12 », les parents savent de qui il s'agit.
- **Vers un LLM** : rien.
- **Sur le réseau local** : le serveur Minecraft reste sur le réseau de la maison,
  avec une liste blanche contenant le seul pseudo de l'enfant.
- **Sur le disque** : SQLite local avec les exercices et leurs dates.

## Statut

**Branche** : `feat/coffre-du-soir`.

- [x] **Phase 1 : Parler au serveur** (Minecraft Java 26.3, Paper build 244)
- [x] **Phase 2 : Saisir un exercice**
- [x] **Phase 3 : L'atelier**
- [ ] **Phase 4 : Le coffre**
- [ ] **Phase 5 : Telegram et rapport**

Les phases 1 et 2 sont indépendantes. La phase 3 dépend de la 1, la phase 4 des
phases 2 et 3, la phase 5 de la 4.

## Conception

### Phase 1 : Parler au serveur

**Contexte.** Aucune commande ne part vers Minecraft aujourd'hui. Tout le reste
de la spec repose sur un adaptateur RCON fiable et sur un échappement de texte
sûr, parce que les énoncés viennent des parents.

**Fichiers.**
- `src/mine_de_rien/adapters/game/java_rcon.py` : `commande` et `dire`.
- `src/mine_de_rien/minecraft/texte.py` (nouveau) : construction des composants
  texte et échappement, au format de la version épinglée.
- `config.essai.toml` (nouveau, versionné) : config qui vise le serveur d'essai.
- `.env.example` : `MC_VERSION` épinglée.
- `tests/test_texte.py`, `tests/test_java_rcon.py`.

**Conception.**
- `commande` appelle le client RCON dans un thread (`asyncio.to_thread`) et se
  reconnecte une fois si la connexion est tombée. Un verrou évite deux commandes
  simultanées sur la même connexion.
- `dire(joueur, texte)` produit un `tellraw` dont le texte est sérialisé par
  `texte.py`, jamais concaténé.
- `texte.py` borne chaque chaîne (longueur maximale, caractères de contrôle
  retirés) et échappe guillemets et antislashs. Même fonction pour les panneaux
  en phase 4.
- Valider sur le serveur d'essai la dernière version stable de Java, puis
  l'épingler dans `.env.example`.

**Vérification automatique.**
- `uv run pytest` : un énoncé contenant `l'élève`, `"guillemets"`, `\`, des
  accents et `]}` produit un composant texte valide qui restitue exactement le
  texte d'origine.
- `/essai-en-jeu` : `list` répond ; un `tellraw` construit par `texte.py` avec ce
  même énoncé piégé est accepté sans erreur de syntaxe ; la version servie est
  relevée et recopiée dans `.env.example`.

**Vérification en jeu.** Se connecter au serveur d'essai sur `localhost:25566`
avec le lanceur réglé sur la version épinglée. Lancer
`uv run mine-de-rien --config config.essai.toml dire "<pseudo>" "l'élève a dit \"bonjour\""` :
le message s'affiche à l'identique dans le chat.

### Phase 2 : Saisir un exercice

**Contexte.** Les parents écrivent soit l'exercice exact, soit une compétence.
Cette phase transforme ce texte en `Exercice` avec une réponse calculée, et le
range dans une file. Elle ne touche pas au jeu.

**Fichiers.**
- `src/mine_de_rien/exercises/expression.py` (nouveau) : analyse et calcul d'une
  expression.
- `src/mine_de_rien/exercises/registre.py` (nouveau) : noms de compétences
  reconnus (`division` → `maths/division-posee`, niveau 1).
- `src/mine_de_rien/saisie.py` (nouveau) : `/coffre <texte>` → liste d'`Exercice`
  ou message d'erreur pour le parent.
- `src/mine_de_rien/storage/sqlite.py` : statut des exercices, file d'attente.
- `tests/test_expression.py`, `tests/test_saisie.py`, `tests/test_stockage.py`.

**Conception.**
- Analyseur par descente récursive sur les jetons : nombres (virgule ou point
  acceptés en entrée), `+ - × x * ÷ : /`, parenthèses. Tout autre caractère est
  refusé. Calcul en `Fraction`.
- Résultat accepté s'il est entier, ou décimal à deux chiffres au plus. Il est
  écrit avec une virgule, sans zéro inutile (`3,5`, pas `3,50`). Un résultat
  négatif est refusé : ce n'est pas du CM2.
- L'énoncé affiché est normalisé (`348 ÷ 12 = ?`), quelle que soit la saisie.
- `saisie.py` essaie d'abord l'expression, puis le registre. Un nom inconnu
  renvoie la liste des compétences disponibles. Nombre d'exercices borné à 10.
- Table `exercices` : ajout de `statut` (`en_file`, `pose`, `ouvert`, `annule`),
  `demande_par`, `recompense`, `source` (`expression` ou `competence`).
  `PRAGMA user_version` pour dater le schéma. Aucune donnée réelle n'existe
  encore, pas de migration.

**Vérification automatique.**
- `uv run pytest` : `348 ÷ 12` donne `29`, `3,5 + 1,25` donne `4,75`,
  `(12 + 8) x 3` donne `60` ; `10 ÷ 3`, `5 - 9`, `2 ** 8` et `__import__` sont
  refusés avec un message ; `division 3` donne trois exercices de division
  différents et vérifiés ; la file rend les exercices dans l'ordre et ignore les
  annulés.

### Phase 3 : L'atelier

**Contexte.** Le coffre a besoin d'un lieu sûr et reconnaissable. Mine de rien le
construit une fois, aux coordonnées choisies par le parent, et y protège
l'enfant contre lui-même avec le mode aventure.

**Fichiers.**
- `src/mine_de_rien/games/atelier.py` (nouveau) : construction et vérification.
- `datapacks/mine_de_rien/` (nouveau) : `pack.mcmeta` et une fonction `tick` pour
  la zone en mode aventure.
- `config.example.toml`, `src/mine_de_rien/config.py` : section `[atelier]` avec
  `x`, `y`, `z`.
- `src/mine_de_rien/cli.py` : sous-commande `atelier`.
- `docker-compose.yml`, `docker/essai.compose.yml` : montage du datapack.
- `tests/test_atelier.py`.

**Conception.**
- L'atelier est un plancher de 5 × 5 avec un coffre au centre, une enclume, un
  emplacement de panneau et une lanterne. Avant de construire, l'agent vérifie
  que le volume est de l'air ou de l'herbe ; sinon il refuse et dit quel bloc
  gêne, sans rien casser.
- Une entité `marker` portant le tag `mdr_atelier` marque le centre. Le datapack
  ne contient aucune coordonnée : il met en aventure les joueurs en survie à
  moins de 6 blocs d'un marqueur, en les marquant `mdr_aventure`, et remet en
  survie ceux qui portent ce tag et sont plus loin. 6 blocs dépassent la portée
  d'un joueur, qui ne peut donc pas casser le coffre depuis l'extérieur.
- Un joueur en créatif ou spectateur n'est jamais touché.
- Commande idempotente : relancée, elle répare sans dupliquer.

**Vérification automatique.**
- `uv run pytest` : les commandes produites pour des coordonnées fixées ; refus
  si le volume n'est pas libre.
- `/essai-en-jeu` : après `atelier`, le coffre, l'enclume et le marqueur existent
  aux bonnes coordonnées ; une seconde exécution ne crée pas de second marqueur ;
  le datapack apparaît dans `datapack list`.

**Vérification en jeu.** Sur le serveur d'essai, se mettre en survie, marcher
vers l'atelier : à quelques blocs, le mode passe en aventure et le coffre ne se
casse pas. S'éloigner : retour en survie. Se mettre en créatif : le mode ne change
plus.

### Phase 4 : Le coffre

**Contexte.** Le cœur du jeu : poser un exercice de la file dans l'atelier, voir
quand l'enfant l'ouvre, récompenser, ranger, passer au suivant. Une sous-commande
de la CLI permet de tout jouer sans Telegram.

**Fichiers.**
- `src/mine_de_rien/games/coffre_du_soir.py`.
- `src/mine_de_rien/orchestrateur.py` (nouveau) : boucle de surveillance.
- `src/mine_de_rien/cli.py` : sous-commandes `coffre <texte>` et `lancer`.
- `config.example.toml` : `[coffre]` avec `recompense` et `niveaux_offerts`.
- `docs/idees.md` : commandes du coffre corrigées pour la version épinglée.
- `tests/test_coffre_du_soir.py`.

**Conception.**
- **Poser** : panneau avec l'énoncé (`texte.py`, quatre lignes de 15 caractères au
  plus, coupure sur les espaces) ; coffre verrouillé par la réponse ; dans le
  coffre, la récompense et un papier « Preuve n°N » portant
  `custom_data={mdr_coffre:N}` ; enclume neuve (une enclume s'abîme à l'usage) ;
  `xp add <pseudo> 3 levels` ; message dans le chat et son de cloche si l'enfant
  est connecté.
- **Surveiller**, toutes les 3 s quand l'enfant est dans `list` : compter les
  papiers de preuve dans son inventaire avec `clear <pseudo> <objet> 0`, qui
  compte sans retirer. Vérifier aussi que le coffre est toujours là.
- **Ouvert** : statut `ouvert`, durée depuis la pose, retrait de la preuve de son
  inventaire, panneau et coffre vidés, puis exercice suivant après 10 secondes.
- **Coffre disparu** sans être ouvert (explosion, créatif) : on le repose, et
  l'événement est noté pour le rapport.
- **À la connexion** de l'enfant avec un coffre actif : rappel dans le chat.
- Les notifications vers les parents passent par une interface vide en phase 4
  (affichage dans la console), branchée sur Telegram en phase 5.

**Vérification automatique.**
- `uv run pytest` : commandes produites pour un exercice fixé, découpage du
  panneau, énoncé piégé échappé.
- `/essai-en-jeu` : après `coffre "348 ÷ 12"`, le panneau porte l'énoncé, le
  coffre porte le verrou attendu et contient la récompense et la preuve, le tout
  vérifié avec `data get block`.

**Vérification en jeu.** Sur le serveur d'essai, en survie : lancer
`uv run mine-de-rien --config config.essai.toml lancer`, puis dans un autre
terminal `uv run mine-de-rien --config config.essai.toml coffre "division 2"`.
Un message et une cloche annoncent le coffre. Le coffre refuse de s'ouvrir.
Renommer un papier avec une mauvaise réponse : toujours fermé, un niveau
dépensé. Renommer avec la bonne réponse : il s'ouvre, la récompense est là, la
console annonce « Coffre n°1 ouvert », et le second coffre apparaît. Se
déconnecter puis se reconnecter : un rappel s'affiche.

### Phase 5 : Telegram et rapport

**Contexte.** Rendre tout cela utilisable par les parents depuis leur téléphone,
et installable à la maison par quelqu'un qui n'a pas suivi le développement.

**Fichiers.**
- `src/mine_de_rien/adapters/channel/telegram.py`.
- `src/mine_de_rien/rapport.py` (nouveau).
- `src/mine_de_rien/orchestrateur.py` : branchement du canal.
- `docker-compose.yml` : liste blanche, aucun opérateur, montage du datapack.
- `docs/installation.md` (nouveau), `README.md`.
- `tests/test_rapport.py`, `tests/test_telegram.py`.

**Conception.**
- Commandes : `/start` (répond l'identifiant Telegram de l'expéditeur, à
  n'importe qui), puis, pour les parents autorisés seulement, `/coffre <texte>`,
  `/file` (exercices en attente et coffre actif), `/annuler <n>` et `/rapport`.
  Tout message d'un inconnu est ignoré sans réponse.
- Notifications aux deux parents : coffre posé, coffre ouvert avec sa durée,
  coffre disparu et reposé.
- `/rapport` sur les 7 derniers jours, par compétence : posés, ouverts, durée
  médiane, et une ligne « à retravailler » pour les exercices non ouverts après
  48 h ou ouverts en plus de 30 minutes.
- `docs/installation.md` : créer le bot avec BotFather, remplir `.env`, accepter
  l'EULA, choisir `ONLINE_MODE` en renvoyant à `docs/rejoindre-le-serveur.md`,
  relever ses identifiants avec `/start`, choisir les coordonnées de
  l'atelier avec F3, importer un monde existant en copiant son dossier dans
  `data/minecraft/world`, régler le lanceur sur la version épinglée, vérifier que
  l'enfant n'est pas opérateur.

**Vérification automatique.**
- `uv run pytest` : un message d'un identifiant non autorisé ne produit aucun
  exercice ; le rapport, sur une base remplie à la main, classe correctement les
  exercices à retravailler ; aucun message envoyé ne contient le pseudo de
  l'enfant.

**Vérification en jeu.** Sur le serveur réel, installé en suivant uniquement
`docs/installation.md`. Depuis le téléphone d'un parent, `/coffre 348 ÷ 12` : les
deux parents reçoivent « coffre posé ». L'enfant se connecte, trouve le coffre,
l'ouvre. Les deux parents reçoivent « Coffre n°1 ouvert ». `/rapport` montre
l'exercice. Depuis un troisième compte Telegram, `/coffre 1 + 1` ne fait rien.
