# Un parent photographie une fiche, Mine de rien en tire les coffres du soir

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

## Pourquoi

Aujourd'hui, un parent doit recopier chaque calcul à la main : `/coffre 348 ÷ 12`, un
coffre par message (`src/mine_de_rien/saisie.py:72-82`). Une fiche « Je mémorise » de
doubles et de moitiés en compte 73. Personne ne les tapera. Les compétences sans calcul,
comme le vocabulaire anglais, n'ont aucune entrée : le registre ne connaît que la division
(`src/mine_de_rien/exercises/registre.py:26-28`).

La lecture de photo est prévue depuis le premier commit (`docs/architecture.md`, décision 3),
mais rien n'existe :

- l'adaptateur LLM local lève `NotImplementedError`
  (`src/mine_de_rien/adapters/llm/openai_compatible.py:15-16`) ;
- le canal Telegram n'écoute que le texte (`src/mine_de_rien/adapters/channel/telegram.py:31`
  et `:60`), alors que `MessageParent` a déjà un champ `image`
  (`src/mine_de_rien/adapters/channel/base.py:14`) ;
- le garde-fou des générateurs n'accepte que des réponses numériques
  (`src/mine_de_rien/saisie.py:34-43`), donc aucun mot ne peut fermer un coffre ;
- la colonne `source` n'admet que `expression` et `competence`
  (`src/mine_de_rien/storage/sqlite.py:29`).

**Essais faits le 2026-11-08** avec le LLM de la maison (Gemma 4 E4B QAT, llama.cpp, vision
active), sur trois photos réelles réduites à 1280 px comme le fait Telegram :

| Photo | Résultat |
|---|---|
| Fiche d'anglais, jours et mois, ordre mélangé | 19 mots sur 19, dans l'ordre, si la consigne interdit de « remettre dans un ordre logique » ; sinon le modèle les remet dans l'ordre du calendrier. 15 à 25 s. |
| Double page « doubles et moitiés », 73 lignes | Page par page : **73 sur 73, aucune invention**, 73 s. Double page d'un coup : il **prolonge les suites** (« Moitié de 3, 5, 7… », 9 à 17 lignes inventées) ou saute des colonnes. |
| Problèmes additifs | Opérations justes, mais énoncés **reformulés** (« et une souris » perdu). |
| Six problèmes en texte | Opération juste 6 fois sur 6, pièges compris. |
| Orientation | Le modèle ne sait pas dire dans quel sens est la photo (1 sur 4). |
| Habillage d'un énoncé pour panneau | Contraintes de longueur non tenues, une fois en anglais, une fois trompeur. |

Le modèle lit bien une page droite, mais il complète ce qu'il croit deviner. Sa lecture ne
peut donc servir que si Python la vérifie ligne à ligne, et le parent avant la pose.

**Cible** : un parent envoie au bot la photo d'une fiche de calculs ou de vocabulaire
anglais avec « 5 » en légende ; Mine de rien répond en moins de deux minutes avec ce qu'il a
lu et les 5 exercices tirés au hasard, réponses calculées en Python ; le parent tape `/oui`
et les 5 coffres partent dans la file.

## Décisions actées

| Sujet | Décision |
|---|---|
| Ce qui est lu | Fiches **imprimées** : lignes de calcul (opérations, « Double de N », « Moitié de N », tables) et mots anglais. Les problèmes à énoncé et l'écriture manuscrite ne sont pas visés. |
| Rôle du LLM | Il **recopie** des lignes, rien d'autre. Tout ce qui suit un `=` dans sa réponse est jeté sans être lu. La réponse d'un calcul vient de l'analyseur (`exercises/expression.py`), la traduction d'un mot vient du lexique. |
| Validation | Chaque ligne lue doit devenir un `Exercice` par l'analyseur, par les formes « Double de N » (`N x 2`) et « Moitié de N » (`N ÷ 2`), ou par le lexique anglais. Sinon elle est rejetée et listée au parent. |
| Découpage | Une photo plus large que haute est une double page : Python la coupe en deux moitiés qui se recouvrent un peu, et envoie chaque moitié séparément. Les doublons sont retirés. |
| Orientation | À la charge du parent, qui peut redresser la photo dans Telegram. Si aucune ligne n'est valide, Mine de rien demande si la photo est à l'endroit. Une photo de travers peut aussi donner des suites **inventées mais justes** (« Double de 21… 30 »), que Python ne peut pas écarter : chaque réponse au parent rappelle de comparer la liste à la fiche, et de répondre `/non` puis de renvoyer la photo à l'endroit si elle ne correspond pas. |
| Consigne au LLM | Sans aucun exemple chiffré ni mot d'exemple : sur une photo qu'il ne lit pas, le modèle recopie les exemples de la consigne comme s'ils étaient sur la fiche. |
| Résolution | La photo Telegram ordinaire suffit (essayée à 1280 px), pas besoin de l'envoyer comme fichier. |
| Saisie | Une photo, avec en légende un nombre N (3 par défaut, 10 au plus, comme `NOMBRE_MAX`) suivi éventuellement d'un texte libre qui précise la partie de la page (« 5 exercice 7 »). Ce texte est ajouté à la consigne du LLM. |
| Tirage | **Immédiat et sans remise** : N exercices au hasard parmi les lignes valides. La lecture est oubliée ensuite ; pour en tirer d'autres, on renvoie la photo. |
| Confirmation | Mine de rien répond avec ce qu'il a lu (groupé : « Doubles : 1, 2, 3… »), les lignes rejetées, puis les N exercices tirés avec leur réponse. `/oui` les met en file, `/non` abandonne. La lecture en attente est gardée en mémoire, une par parent, 30 minutes. |
| Attente | Message « Je lis la photo… » tout de suite. Pendant une lecture (jusqu'à deux minutes), les autres commandes de ce parent attendent leur tour. |
| Photos et LLM distant | Une photo n'est **jamais** envoyée à l'adaptateur `anthropic` : il refuse toute image, quelle que soit la config. Une photo ne se pseudonymise pas, le prénom de l'enfant y figure souvent. Avec un LLM distant, Mine de rien répond au parent que la lecture de photo demande un LLM local. |
| Photos sur le disque | Jamais écrites. Elles restent en mémoire le temps de la lecture. Seuls les exercices confirmés sont stockés, avec la source `photo`. |
| Vocabulaire anglais | Lexique fermé, dans un fichier de données du générateur : jours, mois, nombres de 0 à 20, couleurs, salutations. |
| Sens de traduction | **Les deux, tirés au hasard** pour chaque coffre : « mercredi en anglais ? » ou « Wednesday en français ? ». Un sens n'est proposé que si sa réponse est **unique** dans le lexique : « bonjour » donne *hello* ou *good morning*, donc ces deux mots ne sont demandés que de l'anglais vers le français. Les mots identiques dans les deux langues (« orange ») sont exclus. Python le vérifie à l'import du lexique. |
| Écriture des réponses | Toujours en minuscules, avec l'orthographe scolaire : accents français, anglais britannique (« grey »), masculin pour les couleurs (« vert »), tiret dans « dix-sept », espace dans « good night ». Le mode d'emploi de la clé et l'annonce le rappellent : « en minuscules ». |
| Accents sur l'enclume | Hypothèse à vérifier en jeu en phase 2 : l'enclume accepte « août » tapé au clavier. **Repli** si elle ne l'accepte pas : le lexique marque ces mots, et seul le sens qui ne demande pas d'accent est proposé pour eux. |
| `/coffre anglais 3` | Le générateur de vocabulaire entre aussi au registre : trois mots au hasard dans tout le lexique, sans photo. |
| Énoncé affiché | Celui de la fiche quand il tient sur le panneau (« Double de 17 = ? »), sinon l'énoncé normalisé de l'analyseur. Aucun habillage. |
| Contournement | Une fiche « Je mémorise » porte les réponses imprimées : l'enfant peut la consulter. C'est de la mémorisation, le parent garde la fiche s'il le souhaite. Les essais au hasard restent limités par l'expérience, comme pour tout coffre. |

### Ce que ces décisions renversent

- La spec du coffre du soir disait « Vers un LLM : rien ». Les photos partent maintenant
  vers le LLM **local**, et seulement lui.
- `docs/architecture.md`, décision 5, prévoit la pseudonymisation vers tout LLM distant. Une
  photo ne se pseudonymise pas : elle est donc réservée au LLM local, sans exception.
- `README.md` et `docs/architecture.md`, décision 3, disent que le LLM « habille l'énoncé ».
  L'habillage reste écarté : les essais montrent qu'un modèle de 4 milliards de paramètres
  ne tient pas les contraintes d'un panneau.

## Hors périmètre

- Problèmes à énoncé : énoncé mal recopié, trop long pour un panneau, et c'est le LLM qui
  choisirait l'opération. Seconde spec à part.
- Habillage des énoncés par le LLM.
- Réserve nommée et réutilisable (`/coffre doubles-moities 3` un autre soir) : écartée au
  profit du tirage immédiat.
- Détection automatique de l'orientation : le modèle en est incapable, et un OCR local
  ajouterait une dépendance lourde.
- Écriture manuscrite (exercice recopié par l'enfant dans son cahier) : non essayée.
- Accepter majuscules et variantes d'accents en lisant le nom de la clé au lieu du verrou :
  relève du chantier « essais ratés », qui lira déjà ce nom.
- Retirer au sort sans renvoyer la photo, choisir les exercices à la main dans la liste lue.
- Vocabulaire anglais au-delà des cinq familles du lexique, autres matières.

## Vie privée

- **Vers Telegram** : la photo elle-même, envoyée par le parent au bot. C'est nouveau : une
  photo de fiche passe par les serveurs de Telegram et peut montrer le prénom de l'enfant.
  `docs/installation.md` le dira, et conseillera de cadrer sans le nom. Dans l'autre sens,
  Mine de rien ne renvoie que les lignes lues et les exercices.
- **Vers un LLM** : la photo, au seul LLM local configuré (`[llm] adaptateur =
  "openai-compatible"`), sur le réseau de la maison. Jamais à un LLM distant.
- **Sur le disque** : pas de photo. Les exercices confirmés, comme aujourd'hui, avec
  `source = 'photo'`.

## Statut

**Branche** : `feat/photo-du-cahier`, pas encore créée, `/next` l'ouvre à la première phase.

- [x] **Phase 1 : Parler au LLM local**
- [x] **Phase 2 : Le vocabulaire anglais**
- [x] **Phase 3 : Lire une photo**
- [x] **Phase 4 : La photo depuis Telegram**

Les phases 1 et 2 sont indépendantes. La phase 3 dépend des deux, la phase 4 de la 3.

## Retours de jeu

- 2026-11-08, phase 3, sur le serveur de la maison, photos réduites à 1280 px :
  - double page doubles et moitiés **redressée** : 72 des 73 lignes, aucune inventée, 70 à
    80 s (deux appels) ;
  - même fiche **de travers**, telle que sortie du téléphone : suites inventées
    (« Double de 21 à 30 », « Moitié de 1 à 30 »), toutes justes. D'où le rappel au parent ;
  - avec des exemples chiffrés dans la consigne, la photo de travers les recopiait
    (« 348 ÷ 12 », « 7 × 8 ») : consigne réécrite sans exemple ;
  - fiche d'anglais avec « exercice 7 » : 19 mots sur 19, dans l'ordre, 21 s. Sans
    consigne : 22 mots, dont les salutations manuscrites de l'exercice 6, et un jour
    oublié, 35 s ;
  - problèmes additifs : seules les deux opérations imprimées sont retenues, réponses
    recalculées (39 et 25,5), 36 s.

## Conception

### Phase 1 : Parler au LLM local

**Contexte.** Aucun appel LLM n'existe. Le serveur de la maison est un `llama-server`
(llama.cpp) qui expose `/v1/chat/completions` et accepte des images en `image_url` base64.
Il ignore le champ `model` quand il ne sert qu'un modèle. Ollama, lui, exige l'identifiant
exact (`GET /v1/models`).

**Fichiers.**
- `src/mine_de_rien/adapters/llm/base.py` : le contrat gagne `json: bool` (réponse JSON
  demandée) et une erreur `ErreurLlm`.
- `src/mine_de_rien/adapters/llm/openai_compatible.py` : `completer` par `httpx`.
- `src/mine_de_rien/adapters/llm/anthropic.py` : refuse toute image avec un message clair.
  Le reste reste `NotImplementedError`.
- `src/mine_de_rien/cli.py` : sous-commande `llm "<consigne>" [--image fichier]` pour
  essayer le serveur à la main.
- `config.example.toml` : commentaire sur l'identifiant exact du modèle.
- `tests/test_llm.py`.

**Conception.**
- Requête `POST {url}/chat/completions` : message système, message utilisateur avec le texte
  et l'image en `data:image/jpeg;base64,…`, `temperature: 0`, et
  `response_format: {"type": "json_object"}` si `json`. En-tête `Authorization` seulement si
  `cle` est renseignée. Délai : 180 s.
- Réponse JSON : retirer d'éventuelles balises de code autour avant `json.loads`. Une
  réponse illisible lève `ErreurLlm`, sans nouvel essai.
- Aucune journalisation du contenu des images ni des réponses.

**Vérification automatique.**
- `uv run pytest` : avec `httpx.MockTransport`, la requête porte bien l'image en base64, la
  température 0 et le `response_format` ; une réponse entourée de balises de code est lue ;
  une réponse non JSON lève `ErreurLlm` ; l'adaptateur `anthropic` refuse une image.
- À la main, sur le serveur de la maison :
  `uv run mine-de-rien llm "Que vois-tu ?" --image <photo>` répond en moins d'une minute.

### Phase 2 : Le vocabulaire anglais

**Contexte.** Le coffre n'accepte aujourd'hui que des réponses numériques
(`saisie.py:34-43`). Le vocabulaire est la première compétence dont la clé est un mot. Suivre
`/nouvel-exercice`.

**Fichiers.**
- `src/mine_de_rien/exercises/anglais/vocabulaire.py` (nouveau) : générateur.
- `src/mine_de_rien/exercises/anglais/donnees/lexique.toml` (nouveau) : les paires, par
  famille (emplacement des données de langue fixé par `/nouvel-exercice`).
- `src/mine_de_rien/exercises/registre.py` : entrée `anglais`.
- `src/mine_de_rien/saisie.py` : `_verifier_cle` accepte aussi une clé en mots.
- `src/mine_de_rien/games/coffre_du_soir.py` : « en minuscules » dans le mode d'emploi de
  la clé et dans l'annonce.
- `tests/test_anglais_vocabulaire.py`.

**Conception.**
- Une paire : `anglais`, `francais`, `famille`, et `accent` calculé à l'import. Chargement
  par `tomllib`.
- À l'import, Python calcule pour chaque paire les sens autorisés : la réponse doit être
  unique pour cette question dans tout le lexique (comparaison par `normaliser`), et les
  deux mots doivent être différents.
- `generer(rng, niveau)` : tire une paire et un sens parmi ceux qui sont autorisés. Énoncé
  « mercredi en anglais ? » ou « Wednesday en français ? » ; clé en minuscules. Une fonction
  `exercice_pour(mot)` sert la phase 3 : à partir d'un mot lu, elle produit l'exercice dans
  un sens autorisé tiré au hasard.
- Clé acceptée par `_verifier_cle` : 50 caractères au plus (limite de l'enclume), lettres
  minuscules, accents, espace et tiret ; jamais de majuscule.
- Si le repli sur les accents est déclenché (voir « Vérification en jeu »), les paires
  `accent` perdent le sens qui demande de taper l'accent. Une constante suffit.

**Vérification automatique.**
- `uv run pytest`, sur 500 graines : chaque énoncé tient sur le panneau ; chaque clé passe
  `_verifier_cle` ; « bonjour » n'est jamais demandé vers l'anglais ; « orange » n'est
  jamais tiré ; les deux sens apparaissent ; les cinq familles apparaissent ;
  `/coffre anglais 3` donne trois exercices différents. Une entrée du lexique peut
  interdire le sens vers l'anglais (`vers_anglais = false`) quand l'anglais a plusieurs
  écritures admises : « goodbye » ou « good bye ».
- `/essai-en-jeu` : un coffre verrouillé par « août », puis par « good night », est accepté
  par le serveur et `data get block` restitue la clé à l'identique. Validé sur Minecraft
  26.3 le 2026-11-08, avec aussi « bon après-midi ».

**Vérification en jeu.** Sur le serveur d'essai, envoyer `/coffre anglais 3` jusqu'à obtenir
un mot à accent, sinon poser à la main un coffre de clé « août » par `rcon-cli`. À
l'atelier, poser la clé sur l'enclume, taper « août » au clavier : l'accent circonflexe doit
s'afficher dans le champ, et le coffre doit s'ouvrir. Si l'accent ne s'affiche pas, activer
le repli et le noter dans « Retours de jeu ». Vérifier aussi qu'une clé renommée
« Mercredi » n'ouvre pas un coffre « mercredi », et que le mode d'emploi dit « en
minuscules ».

### Phase 3 : Lire une photo

**Contexte.** Le cœur du chantier : d'une photo à une liste d'exercices validés, sans
Telegram. Les essais du 2026-11-08 fixent la consigne et le découpage (voir « Pourquoi »).
Le travail se fait sur des réponses du LLM enregistrées, jamais sur les photos : elles
montrent le prénom de l'enfant et ne doivent pas entrer dans le dépôt.

**Fichiers.**
- `src/mine_de_rien/photo.py` (nouveau) : préparation de l'image, consigne, interprétation.
- `src/mine_de_rien/cli.py` : sous-commande `lire-photo <fichier> [--consigne "exercice 7"]`
  qui affiche la lecture, les rejets et un tirage.
- `pyproject.toml` : dépendance `pillow`.
- `tests/test_photo.py`, `tests/donnees/photo/*.json` : réponses du LLM, sans nom, y
  compris celle d'une photo de travers, qui garde la trace de la limite.

**Conception.**
- Préparation : redressement EXIF, réduction à 1536 px de côté au plus, JPEG. Plus large que
  haute : deux moitiés `[0 ; 0,53]` et `[0,47 ; 1]` en largeur, un appel par moitié.
- Consigne système : « Tu transcris fidèlement des fiches photographiées. Tu ne calcules
  rien, tu ne prolonges aucune suite, tu ne remets rien dans un ordre logique, tu ignores
  l'écriture manuscrite. » Consigne utilisateur : lignes de calcul et mots anglais
  imprimés, plus le texte libre du parent. Réponse JSON
  `{"lignes": [{"genre": "calcul" | "mot", "texte": "…"}]}`.
- Interprétation, ligne par ligne :
  - `calcul` : couper au premier `=` et jeter la suite. « Double de N » et « Moitié de N »
    deviennent `N x 2` et `N ÷ 2`, avec l'énoncé « Double de N = ? ». Sinon, l'analyseur
    d'expressions (`calculer`). Les espaces de milliers (« 1 000 ») sont retirées avant.
  - `mot` : `vocabulaire.exercice_pour(mot)`, sinon rejet.
  - Rejet listé avec sa raison (« hors lexique », « calcul illisible », « ne tombe pas
    juste »). Doublons retirés par énoncé.
- Résultat : `Lecture(exercices, rejets)`, et `tirer(lecture, n, rng)` sans remise.

**Vérification automatique.**
- `uv run pytest` : à partir des réponses enregistrées pour les deux fiches essayées, 72
  des 73 doubles et moitiés, aucun inventé, 19 mots dans l'ordre de la fiche ; une réponse qui contient « Double de 17 = 35 »
  donne quand même la clé « 34 » ; « Moitié de 3 » donne « 1,5 », valide ; « Moitié de 1,25 »
  (trois décimales), ou « 10 ÷ 3 », est rejeté ; un mot hors lexique est rejeté ; une
  image paysage produit deux appels ; une image portrait, un seul ; le tirage ne dépasse
  jamais le nombre de lignes valides.
- À la main, sur le serveur de la maison, avec les trois photos du 2026-11-08 :
  `uv run mine-de-rien lire-photo <photo>` retrouve au moins 70 des 73 doubles et moitiés
  sur la double page redressée, et les 19 jours et mois de la fiche d'anglais ; la photo des problèmes ne donne
  aucun exercice faux. Noter les durées dans « Retours de jeu ».

### Phase 4 : La photo depuis Telegram

**Contexte.** Brancher la phase 3 sur le canal, avec confirmation. Le bot ne connaît que le
texte aujourd'hui (`telegram.py:60`).

**Fichiers.**
- `src/mine_de_rien/adapters/channel/telegram.py` : écoute aussi les photos ; télécharge la
  plus grande taille en mémoire et la place dans `MessageParent.image`, avec la légende
  comme `texte`.
- `src/mine_de_rien/orchestrateur.py` : traitement d'une photo, `/oui`, `/non`, aide mise à
  jour.
- `src/mine_de_rien/storage/sqlite.py` : migration 3, `source` admet `photo`.
- `docs/installation.md` : la photo passe par Telegram, cadrer sans le prénom ; la lecture
  demande un LLM local.
- `tests/test_orchestrateur.py`, `tests/test_telegram.py`, `tests/test_stockage.py`.

**Conception.**
- Photo d'un parent autorisé : réponse immédiate « Je lis la photo… », puis lecture (phase 3)
  avec le LLM configuré. Si l'adaptateur est distant : refus avec explication, sans appel.
- Légende : `^\s*(\d+)?\s*(.*)$`. N vaut 3 par défaut et 10 au plus ; le reste est la
  consigne libre.
- Réponse au parent : nombre de lignes lues par famille, liste compacte (« Doubles : 1, 2,
  3… »), rejets, puis les N exercices tirés avec leur réponse, et « /oui pour les mettre en
  file, /non pour abandonner ». Rien de valide : « Je n'ai rien reconnu. La photo est-elle à
  l'endroit ? »
- Lecture en attente : un dictionnaire en mémoire, une par parent, 30 minutes. `/oui` la
  met en file par le chemin existant (`mettre_en_file`, source `photo`), `/non` l'oublie.
  Une nouvelle photo remplace la précédente.
- Migration 3 : SQLite ne modifie pas une contrainte `CHECK`, donc la table `exercices`
  est recréée et ses lignes recopiées, dans une transaction.

**Vérification automatique.**
- `uv run pytest` : une photo d'un inconnu est ignorée ; une photo avec un adaptateur
  `anthropic` est refusée sans appel au LLM ; légende « 5 exercice 7 » → N = 5 et consigne
  « exercice 7 » ; `/oui` met en file avec `source = 'photo'` ; `/oui` sans lecture en
  attente, ou après 30 minutes, répond qu'il n'y a rien à confirmer ; une base en version 2
  migre sans perdre ses exercices.

**Vérification en jeu.** Sur le serveur d'essai, avec le bot d'essai : envoyer la photo de la
fiche doubles et moitiés avec « 3 » en légende. Vérifier le message d'attente, puis la
lecture (73 lignes) et les trois exercices tirés. Taper `/oui` : `/file` montre les trois
coffres. À l'atelier, ouvrir le premier, panneau « Double de N = ? ». Recommencer avec la
fiche d'anglais et « 3 exercice 7 », taper `/non`, et vérifier que `/file` n'a pas changé.
