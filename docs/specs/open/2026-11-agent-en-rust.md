# L'agent devient un binaire Rust unique, sans rien changer pour l'enfant ni pour les parents

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

## Pourquoi

L'agent est écrit en Python : 4 524 lignes dans `src/mine_de_rien/`, 3 526 lignes et
261 tests dans `tests/`. Le mainteneur préfère Rust et ne juge pas Python adapté à un
service qui tourne en permanence à la maison. Ce que le dépôt montre :

- l'image embarque un interpréteur, `uv` et un environnement virtuel pour lancer une seule
  commande (`docker/agent.Dockerfile:3-15`) ;
- le datapack est retrouvé par le chemin du fichier source, ce qui impose une installation
  en mode éditable dans l'image (`src/mine_de_rien/datapack.py:18`,
  `docker/agent.Dockerfile:11-12`) ;
- les états sont des chaînes vérifiées à l'exécution : statuts et sources de coffre
  (`src/mine_de_rien/storage/sqlite.py:28-31`), adaptateurs choisis par nom
  (`src/mine_de_rien/config.py:26-45`) ;
- une dépendance déclarée ne sert pas : `anthropic` (`pyproject.toml:11`), dont
  l'adaptateur est une ébauche (`src/mine_de_rien/adapters/llm/anthropic.py:31`).

Ce que Rust n'apportera pas : de la vitesse. L'agent attend le réseau et tourne toutes les
3 secondes (`src/mine_de_rien/orchestrateur.py:57`). Et ce chantier n'apporte rien à
l'enfant : son seul critère de réussite, côté jeu, est que rien ne se voie.

Le moment est le moins coûteux possible : la dictée audio (7 phases,
`docs/specs/draft/2026-11-dictee-audio.md`) n'a pas commencé. Chaque phase écrite en Python
d'ici là serait à écrire deux fois.

**Cible** : `mine-de-rien` est un binaire Rust statique, datapack compris, dans une image de
quelques dizaines de Mo ; même `config.toml`, même base SQLite, mêmes commandes Telegram et
même comportement en jeu qu'en 0.1.2 ; plus une ligne de Python dans le dépôt.

## Décisions actées

| Sujet | Décision |
|---|---|
| Ordre | La conversion passe **avant** la dictée audio, qui s'écrira ensuite directement en Rust. |
| Périmètre | Iso-fonctionnel strict. Aucune nouveauté, aucune correction de comportement : un écart avec le Python est un défaut du port, sauf s'il est noté dans « Écarts assumés » (section à ouvrir dans ce fichier au premier cas). |
| Fidélité | Traduction module par module, mêmes noms de modules, de fonctions et de tests. Les chaînes fermées (statut, source, adaptateur) deviennent des `enum`, sans redessiner le reste. |
| Emplacement | Un seul crate à la racine : `Cargo.toml`, `src/lib.rs`, `src/main.rs`, puis `src/adapters/`, `src/exercises/`, `src/games/`, `src/storage/`, `src/minecraft/`. L'arborescence de `CLAUDE.md` reste vraie, sans l'étage `mine_de_rien/`. |
| Cohabitation | Le Python reste en place et **fait foi** jusqu'à la dernière phase, qui le supprime. Tout se passe sur la branche : `main` reste en Python et publiable. |
| Oracle | Tant que le Python existe, il produit des fichiers de référence (`tests/donnees/oracle/*.json`) : pour des entrées fixées, les commandes Minecraft, les messages, les textes de rapport. Les tests Rust exigent la même sortie, au caractère près. Ces fichiers restent après la suppression du Python. |
| Tests | Chaque `tests/test_x.py` devient `tests/x.rs`, chaque test garde son nom. Un test Python sans équivalent Rust est listé avec sa raison dans « Écarts assumés ». |
| Aléa | `rand`, avec un générateur initialisé par graine dans les tests. Les suites tirées ne sont pas celles de Python : aucune graine n'est stockée, les tests vérifient des propriétés (`tests/test_division.py:8-13`), pas des valeurs. |
| Asynchrone | `tokio`, runtime **mono-fil** (`current_thread`) : c'est le modèle d'`asyncio`. Le dépôt SQLite et le verrou du coffre restent partagés entre trois tâches sans `Mutex` de fil ni contrainte `Send`. |
| Telegram | `teloxide`, limité au client `Bot` et à l'interrogation longue. Pas de webhook. |
| HTTP | `reqwest` pour le LLM local, la même version que celle de `teloxide`. TLS par `rustls` avec les racines embarquées : aucune dépendance à OpenSSL ni aux certificats du système. |
| SQLite | `rusqlite` avec SQLite embarqué (`bundled`). Le schéma, `PRAGMA user_version` et le format des dates ne changent pas : la base de la maison est rouverte telle quelle. |
| Images | Crate `image` à la place de Pillow : redressement EXIF, coupe en deux, réduction à 1 536 px, JPEG qualité 85. |
| LLM distant | L'adaptateur Anthropic reste l'ébauche qu'il est : il refuse les photos et ne fait aucun appel. Aucun SDK. |
| Binaire | Statique (cibles `musl`), datapack embarqué à la compilation. `mine-de-rien datapack <dossier>` l'écrit sans dossier source ; `--source` reste possible. |
| Image Docker | `FROM scratch` : le binaire seul. Construite par compilation croisée depuis la plateforme de la CI, plus sous QEMU. amd64 et arm64, comme aujourd'hui. |
| Arrêt | Le binaire traite `SIGTERM` et `SIGINT` : seul processus de son conteneur, il ne recevrait sinon aucun arrêt propre. |
| Identifiants | Règle de `CLAUDE.md` inchangée : français pour le métier, anglais pour la plomberie. |
| Qualité | `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` remplacent `ruff` et `pytest`. Pas de `unsafe`. |
| Version | Aucun changement de numéro dans ce chantier. La version publiée ensuite se décide par la procédure habituelle (`docs/publier-une-version.md`). |

Hypothèses prises sans poser la question, à corriger dans ce fichier si elles sont fausses :

- personne d'autre ne dépend de l'API Python du paquet : seul le binaire `mine-de-rien` compte ;
- aucun correctif n'est attendu sur `main` pendant le port ; s'il en vient un, il est
  reporté sur la branche dans les deux langages tant que le Python y vit ;
- la recette finale se joue sur le serveur d'essai, puis une fois sur le vrai serveur après
  fusion, avec une copie de `data/agent` gardée de côté.

### Ce que ces décisions renversent

- `CLAUDE.md` : « la réponse est calculée en Python » devient « calculée par le générateur,
  jamais par le LLM ». Les commandes `uv` et les chemins `src/mine_de_rien/…` changent.
- `README.md:41` : « Python 3.12+, `uv`, SQLite ».
- Spec `2026-11-image-publiee` (close) : l'image partait de `python:3.12-slim` et se
  construisait sous QEMU ; le projet y était installé en mode éditable pour trouver le
  datapack.
- `scripts/verifier_version.py` lisait `pyproject.toml` et `__init__.py` : la version n'a
  plus qu'une source, `Cargo.toml`.
- Spec `2026-11-dictee-audio` (en brouillon dans `docs/specs/draft/`, non commencée) : tous ses chemins `.py`, ses
  commandes `uv run pytest` et son choix d'une « bibliothèque Python qui écrit du Vorbis »
  sont à réécrire avant sa première phase. Ce n'est pas fait ici.
- Skills `/next`, `/nouvel-exercice`, `/nouveau-jeu`, `/essai-en-jeu` : commandes et chemins.

## Hors périmètre

- La dictée audio, et toute autre feature : gelées jusqu'à `/close` de cette spec.
- Réécrire la spec de la dictée pour Rust : un passage par `/feature` après la clôture.
- Un adaptateur Anthropic qui fonctionne, un canal Matrix ou web, un plugin Paper.
- Redessiner l'orchestrateur, le schéma SQLite ou le format de `config.toml`.
- Corriger au passage un défaut connu du Python : le noter dans « Retours de jeu », pas plus.
- Découper en plusieurs crates (workspace) : un seul suffit à cette taille.
- Client Telegram écrit à la main avec `reqwest` : envisagé, écarté au profit de `teloxide`.
- Faire parler Rust et Python ensemble (FFI, deux processus) pendant la transition.
- Reproduire les suites aléatoires de Python (Mersenne Twister) : rien n'en dépend.

## Vie privée

Rien de nouveau ne sort de la maison. Les mêmes échanges qu'en 0.1.2 : Telegram (énoncés,
réponses, numéros, durées, jamais le pseudo), le LLM local (photo, message adressé à Pixel).

Deux points à tenir pendant le port :

- les garde-fous existants sont des comportements à porter et à tester comme les autres :
  refus d'une photo vers un LLM distant, photo jamais écrite sur le disque, chat ni
  journalisé ni enregistré, aucune réponse à un expéditeur non autorisé ;
- aucune dépendance Rust ne doit ouvrir de connexion de son propre chef (télémétrie,
  vérification de mise à jour). À contrôler à chaque ajout dans `Cargo.toml`. Les crates se
  téléchargent à la compilation, jamais à l'exécution.

## Statut

**Branche** : `feat/agent-en-rust`, pas encore créée, `/next` l'ouvre à la première phase.

- [x] **Phase 1 : Le crate, la configuration et le datapack embarqué**
- [x] **Phase 2 : Texte Minecraft, expressions et division**
- [x] **Phase 3 : Anglais, indices, registre et saisie**
- [x] **Phase 4 : La base SQLite et le rapport**
- [x] **Phase 5 : Parler au serveur : RCON et log**
- [x] **Phase 6 : Atelier, emplacement et mode paisible**
- [x] **Phase 7 : Le coffre du soir et la boucle de surveillance**
- [x] **Phase 8 : LLM local et lecture de photo**
- [ ] **Phase 9 : Pixel**
- [ ] **Phase 10 : Telegram et les commandes des parents**
- [ ] **Phase 11 : La bascule**

Chaque phase s'appuie sur les précédentes, sauf la 8, qui ne dépend que des phases 1 à 3.
Les phases 1 à 4 ne touchent ni au jeu ni au réseau. La phase 11 est la seule qui retire
quelque chose : jusqu'à elle, abandonner le chantier ne coûte que la branche.

## Conception

Aucune commande Minecraft nouvelle dans ce chantier : celles du Python, déjà vérifiées sur
la 26.3, sont reproduites à l'identique et contrôlées par l'oracle.

### Règles communes à toutes les phases

**Vérification automatique, toujours.**

    cargo fmt --check
    cargo clippy --all-targets -- -D warnings
    cargo test
    uv run pytest        # jusqu'à la phase 10 : le Python doit rester vert

**L'oracle.** `scripts/oracle.py` (Python, temporaire, supprimé en phase 11) importe le
paquet Python et écrit `tests/donnees/oracle/<module>.json` : une liste de cas
`{"entree": …, "sortie": …}`. Chaque phase y ajoute les cas de ses modules, relance le
script, et commite les fichiers produits. Le test Rust lit le fichier et compare.

**Pièges de traduction, à relire à chaque phase.**

- **Caractères, pas octets.** `len(texte)` et `texte[:n]` comptent en caractères ; en Rust,
  `len()` compte des octets et couper au milieu d'un « é » ou d'un « ÷ » fait paniquer.
  Partout où le Python mesure ou coupe : `chars()`. Concerne la largeur du panneau
  (`minecraft/texte.py`), la coupe des messages Telegram
  (`adapters/channel/telegram.py:53-72`), `LONGUEUR_MAX` (`exercises/expression.py:27`), les
  morceaux RCON (`adapters/game/rcon.py:28-31`, où il faut relire si la limite est en
  octets ou en caractères).
- **Expressions régulières.** Le crate `regex` n'a ni regard avant ni regard arrière.
  `_MILLIERS` en utilise (`photo.py:56`, `exercises/indices.py:41`) : réécrire à la main
  plutôt qu'ajouter `fancy-regex`. `fullmatch` et `match` de Python n'ont pas d'équivalent
  direct : ancrer avec `^…$`. `\d`, `\w`, `\s` sont Unicode des deux côtés.
- **Unicode.** `unicodedata.normalize("NFKD", …).encode("ascii", "ignore")`
  (`exercises/base.py:12`) : crate `unicode-normalization`, puis filtre ASCII.
  `unicodedata.category` (`games/pixel.py:177`) : crate `unicode-general-category`.
  `str.lower()`, `isdigit()` : vérifier sur l'oracle avec « É », « œ », « ² ».
- **Dates.** Le Python écrit `datetime.isoformat()` en UTC
  (`2026-11-08T18:45:26.123456+00:00`). Le Rust doit lire ce format, avec ou sans
  microsecondes, et écrire le même.
- **Calcul exact.** `Fraction` (`exercises/expression.py:21`) : `num-rational` sur grands
  entiers, ou `i128` avec opérations vérifiées. Jamais de flottant.
- **Mise en forme.** Les `f"{x:.0f}"`, la médiane de `statistics` (`rapport.py`) et les
  arrondis : comparer sur l'oracle, pas de mémoire.
- **Erreurs.** Une exception Python attrapée devient une variante d'erreur. Un `panic` dans
  une tâche tue l'agent : pas de `unwrap()` hors des tests et des constantes.

### Phase 1 : Le crate, la configuration et le datapack embarqué

**Contexte.** Le socle, sans jeu ni réseau. À la fin, `cargo run -- datapack <dossier>` et
`cargo run -- --version` marchent, à côté du Python intact.

**Fichiers.**
- `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` (version de Rust figée), `src/lib.rs`,
  `src/main.rs`, `src/cli.rs`.
- `src/config.rs` : `src/mine_de_rien/config.py`.
- `src/datapack.rs` : `src/mine_de_rien/datapack.py`.
- `tests/config.rs`, `tests/datapack.rs`.
- `scripts/oracle.py`, `tests/donnees/oracle/` (premiers cas : configurations lues).
- `.gitignore`, `.dockerignore` : `target/`.

**Conception.**
- `clap` pour les sous-commandes, mêmes noms et mêmes options qu'en Python
  (`cli.py:143-169`). Les sous-commandes pas encore portées répondent « pas encore portée »
  et sortent en erreur.
- Configuration : `serde` et `toml`, substitution des `${VARIABLES}` avant analyse, comme
  aujourd'hui. Les valeurs par défaut et les messages de refus sont ceux du Python.
  `adaptateur` devient un `enum` par section.
- Datapack : `datapacks/mine_de_rien` embarqué dans le binaire (`include_dir`).
  `recopier` garde sa copie atomique par dossier temporaire voisin et ses refus
  (destination qui recouvre la source).
- La version affichée vient de `Cargo.toml`, alignée sur `pyproject.toml` (0.1.2).
- Runtime `tokio` mono-fil posé dès maintenant dans `main`, même s'il ne sert pas encore.

**Vérification automatique.** Les commandes communes, plus :
- `config.example.toml` et `config.essai.example.toml` se lisent ; une variable absente,
  un nom d'assistant ou un objet de récompense invalide sont refusés avec le message Python ;
- `cargo run -- datapack /tmp/essai-datapack` écrit un dossier identique à
  `datapacks/mine_de_rien` (`diff -r` vide), y compris lancé depuis un autre répertoire.

### Phase 2 : Texte Minecraft, expressions et division

**Contexte.** Le cœur déterministe, première moitié. C'est ici que la règle « le LLM ne
calcule jamais » vit : la réponse d'un coffre sort de ces modules.

**Fichiers.**
- `src/minecraft/texte.rs`, `src/exercises/base.rs`, `src/exercises/expression.rs`,
  `src/exercises/maths/division.rs`.
- `tests/texte.rs`, `tests/expression.rs`, `tests/division.rs`.
- `scripts/oracle.py`, `tests/donnees/oracle/`.

**Conception.**
- `Exercice` : `competence`, `enonce`, `reponse`, `donnees` (`serde_json::Value`, objet).
- `Generateur` devient un type de fonction `fn(&mut impl Rng, niveau) -> Exercice`.
- L'analyseur d'expressions reste une descente récursive sur jetons, calcul exact.
- Oracle : une centaine d'expressions (valides, refusées, avec virgule, parenthèses,
  « = ? » final, résultat non décimal, négatif, trop long) avec réponse ou message de refus ;
  l'échappement SNBT et le découpage en lignes de panneau sur des textes accentués.

**Vérification automatique.** Les commandes communes. Les propriétés de
`tests/test_division.py` tiennent sur 200 graines aux deux niveaux ; l'oracle des
expressions et du texte passe en entier.

### Phase 3 : Anglais, indices, registre et saisie

**Contexte.** Seconde moitié du cœur déterministe. `indices.py` est le plus gros module du
dépôt (426 lignes) et le plus sensible aux pièges Unicode.

**Fichiers.**
- `src/exercises/anglais/vocabulaire.rs`, `src/exercises/anglais/donnees/*.toml` (déplacés
  tels quels à la phase 11 ; lus par chemin relatif d'ici là), `src/exercises/indices.rs`,
  `src/exercises/registre.rs`, `src/saisie.rs`.
- `tests/anglais_vocabulaire.rs`, `tests/indices.rs`, `tests/saisie.rs`.
- `scripts/oracle.py`, `tests/donnees/oracle/`.

**Conception.**
- Lexique et indices embarqués par `include_str!`, analysés une fois (`OnceLock`), avec
  les mêmes contrôles de cohérence qu'au chargement Python.
- Le registre est une table statique compétence → générateur.
- Oracle : pour chaque paire du lexique et chaque sens, l'exercice produit ; pour une
  liste d'exercices fixés, les trois paliers d'indices ; pour une liste de saisies de
  parent, l'exercice ou le refus ; `cle_valide` sur des clés limites.

**Vérification automatique.** Les commandes communes. Aucun palier d'indice ne contient la
réponse, sur tout le lexique et sur 200 divisions tirées.

### Phase 4 : La base SQLite et le rapport

**Contexte.** La base de la maison existe, en schéma 3, écrite par Python. Elle doit se
rouvrir sans rien perdre : c'est le seul état durable du projet.

**Fichiers.**
- `src/storage/sqlite.rs`, `src/rapport.rs`.
- `tests/stockage.rs`, `tests/rapport.rs`.
- `tests/donnees/bases/v1.sqlite`, `v2.sqlite`, `v3.sqlite` : écrites par `scripts/oracle.py`
  avec le dépôt Python, quelques coffres dans chaque statut, des événements, des accents.
- `scripts/oracle.py`, `tests/donnees/oracle/rapport.json`.

**Conception.**
- `Depot` : mêmes méthodes, mêmes requêtes. `SCHEMA` et `MIGRATIONS` recopiés tels quels.
- `statut` et `source` deviennent des `enum` côté Rust, toujours stockés en texte. Une
  valeur inconnue lue dans la base est une `ErreurStockage`, pas un `panic`.
- L'horloge reste injectée, pour les tests.
- Oracle du rapport : sur `v3.sqlite` et une date fixée, le texte exact de `/rapport`.

**Vérification automatique.** Les commandes communes, plus :
- `v1.sqlite` et `v2.sqlite` migrent en version 3 et donnent les mêmes lignes que `v3` ;
- un exercice écrit par Rust est relu par le Python, et l'inverse (test qui lance
  `uv run python` tant que le Python existe, retiré en phase 11) ;
- le rapport Rust est identique à l'oracle.

### Phase 5 : Parler au serveur : RCON et log

**Contexte.** Premier contact avec le jeu. Le client RCON est fait main parce que le
serveur vanilla lit un paquet par lecture et perd le second
(`adapters/game/rcon.py:9-15`) : ce comportement est à porter, pas à remplacer par un crate.

**Fichiers.**
- `src/adapters/game/base.rs`, `rcon.rs`, `java_rcon.rs`, `log.rs`.
- `src/cli.rs` : `dire`, `ecouter`.
- `tests/java_rcon.rs`, `tests/chat_log.rs`.

**Conception.**
- RCON directement asynchrone (`tokio::net::TcpStream`), sans fil auxiliaire : un seul
  paquet en attente à la fois, sentinelle pour les réponses longues, reconnexion quand le
  serveur a fermé, délais par `tokio::time::timeout`.
- Le contrat du jeu est un trait. Les tests Python passent de faux jeux par duck typing ;
  en Rust, un faux jeu qui implémente le trait et enregistre les commandes. Choisir ici
  entre trait générique et objet-trait asynchrone, et s'y tenir pour le canal et le LLM.
- Suivi du log : même logique que `SuiviLog` (départ en fin de fichier, inode, fichier qui
  rétrécit, ligne incomplète gardée). Le flux de messages devient un canal `tokio::mpsc`.
- Les tests RCON tournent contre un faux serveur TCP local, comme en Python.

**Vérification automatique.** Les commandes communes, plus, avec `/essai-en-jeu` :
- `cargo run -- --config config.essai.toml dire <pseudo> "Bonjour, ça marche ?"` ne
  renvoie pas d'erreur ;
- une commande à réponse longue (`data get` sur un coffre plein) revient entière ;
- après `docker compose restart` du serveur d'essai, la commande suivante se reconnecte.

**Vérification en jeu.** Sur le serveur d'essai, connecté avec le jeu. Lancer
`cargo run -- --config config.essai.toml ecouter` dans un terminal. Dans le jeu, touche `T`,
taper `salut à tous`, Entrée : la ligne apparaît dans le terminal avec le pseudo et les
accents. Dans un autre terminal, la commande `dire` ci-dessus : le message s'affiche dans
le chat du jeu, accents compris.

### Phase 6 : Atelier, emplacement et mode paisible

**Contexte.** Les trois modules de jeu qui ne dépendent pas de la base.

**Fichiers.**
- `src/games/atelier.rs`, `emplacement.rs`, `paisible.rs`.
- `src/cli.rs` : `atelier`.
- `tests/atelier.rs`, `tests/emplacement.rs`, `tests/paisible.rs`.
- `scripts/oracle.py`, `tests/donnees/oracle/atelier.json`, `emplacement.json`,
  `paisible.json`.

**Conception.**
- Oracle : le Python est lancé avec un faux jeu qui rejoue des réponses fixées ; la liste
  ordonnée des commandes envoyées est la sortie. Le Rust, avec les mêmes réponses, doit
  envoyer la même liste.
- Les en-têtes de module (contournements, limites) sont recopiés en commentaires de
  documentation : la skill `/nouveau-jeu` les exige.

**Vérification automatique.** Les commandes communes ; les trois oracles passent. Avec
`/essai-en-jeu` : `cargo run -- --config config.essai.toml atelier` construit l'atelier, et
`data get block` sur le coffre et le panneau donne ce que donne la version Python.

**Vérification en jeu.** Sur le serveur d'essai, monde neuf. Lancer la commande `atelier`
ci-dessus, se connecter, aller aux coordonnées affichées (`F3` montre la position) :
l'atelier est là, entier, et la zone passe en mode aventure quand on y entre (impossible de
casser un bloc).

### Phase 7 : Le coffre du soir et la boucle de surveillance

**Contexte.** Le jeu lui-même. À la fin, l'agent Rust fait vivre un coffre de bout en
bout, sans Telegram : notifications dans le terminal.

**Fichiers.**
- `src/games/coffre_du_soir.rs`.
- `src/orchestrateur.rs` : `Orchestrateur`, `ajouter_exercices`, `mettre_en_file`
  (`orchestrateur.py:61-197`). `CommandesParents` attend la phase 10.
- `src/cli.rs` : `coffre`, `lancer` (sans canal, sans Pixel).
- `tests/coffre_du_soir.rs`, `tests/orchestrateur.rs` (la partie boucle).
- `scripts/oracle.py`, `tests/donnees/oracle/coffre_du_soir.json`.

**Conception.**
- Le verrou du coffre est un `tokio::sync::Mutex` partagé par `Rc` (runtime mono-fil).
- `tourner` : une erreur de RCON, de coffre ou de stockage est journalisée et le tour
  suivant repart, comme aujourd'hui. Aucune autre erreur ne doit pouvoir arrêter la boucle.
- Arrêt propre sur `SIGTERM` et `SIGINT` : fin de la boucle, fermeture du dépôt et du RCON.
- Oracle : pose, équipement, annonce, rappel, rangement et retrait d'un coffre, pour une
  division, un mot anglais à apostrophe et un énoncé qui déborde du panneau.

**Vérification automatique.** Les commandes communes ; l'oracle passe ; les scénarios de
`tests/test_orchestrateur.py` sur la boucle (pose, délai de 10 s, coffre disparu, atelier
absent signalé une fois, connexion et rappel) passent avec faux jeu et horloge simulée.

**Vérification en jeu.** Sur le serveur d'essai, atelier construit (phase 6). Terminal 1 :
`cargo run -- --config config.essai.toml lancer`. Terminal 2 :
`cargo run -- --config config.essai.toml coffre "348 ÷ 12"`. Dans le jeu : le chat annonce
le coffre, une « Clé du coffre » arrive dans l'inventaire. Aller à l'atelier, lire le
panneau. Clic droit sur le coffre : il refuse. Clic droit sur l'enclume, poser la clé dans
la case de gauche, effacer le nom, taper `29`, prendre la clé dans la case de droite. Clé en
main, clic droit sur le coffre : il s'ouvre. Prendre le papier « Preuve » : le terminal 1
annonce l'ouverture avec la durée. Mettre un second coffre en file, puis casser le coffre
avant de l'ouvrir (impossible en mode aventure : le retirer par
`docker compose exec minecraft rcon-cli setblock <x> <y> <z> air`, coordonnées affichées en
phase 6) : le terminal annonce qu'il a été reposé.

### Phase 8 : LLM local et lecture de photo

**Contexte.** Ne dépend que des phases 1 à 3. La lecture a été réglée à l'essai sur de
vraies fiches (`photo.py:13-17`) ; les tests ne rejouent que des réponses JSON du modèle,
jamais une image. Changer de bibliothèque d'image change les pixels envoyés.

**Fichiers.**
- `src/adapters/llm/base.rs`, `openai_compatible.rs`, `anthropic.rs`, `mod.rs` (`creer`).
- `src/photo.rs`.
- `src/cli.rs` : `llm`, `lire-photo`.
- `tests/llm.rs`, `tests/photo.rs` (les fichiers `tests/donnees/photo/*.json` resservent).

**Conception.**
- `reqwest` vers l'API compatible OpenAI ; les tests passent par un faux serveur HTTP
  local à la place de `httpx.MockTransport`.
- Préparation de l'image : orientation EXIF appliquée, conversion RGB, coupe en deux
  moitiés recouvrantes si l'image est plus large que haute, réduction dans un carré de
  1 536 px sans agrandir, JPEG qualité 85. Formats d'entrée : ceux que Telegram envoie
  (JPEG), plus PNG pour la ligne de commande.
- Les deux refus de vie privée sont des tests : une photo vers un adaptateur distant lève
  l'erreur sans appel réseau ; la photo ne touche jamais le disque.

**Vérification automatique.** Les commandes communes ; chaque fichier de
`tests/donnees/photo/` donne la même lecture qu'en Python (oracle) ; une image de test
avec orientation EXIF 6 sort redressée, aux bonnes dimensions.

**Vérification à la main.** Avec le LLM local lancé
(`docker compose --profile local-llm up -d`), sur deux vraies fiches déjà lues par la
version Python : `uv run mine-de-rien lire-photo fiche.jpg` puis
`cargo run -- lire-photo fiche.jpg`. Les deux listes de lignes lues doivent se valoir. Un
écart net (lignes inventées, lignes sautées) se note dans « Retours de jeu » et se règle
ici (filtre de réduction, qualité JPEG) avant de continuer.

### Phase 9 : Pixel

**Contexte.** L'assistant du chat. S'appuie sur le log (phase 5), les indices (phase 3), la
base (phase 4) et le LLM (phase 8).

**Fichiers.**
- `src/games/pixel.rs`.
- `src/cli.rs` : `lancer` démarre Pixel quand il est actif.
- `tests/pixel.rs`.
- `scripts/oracle.py`, `tests/donnees/oracle/pixel.json`.

**Conception.**
- Traduction fidèle, en-tête de module compris. Les deux modes restent : mots-clés sans
  LLM, classement par LLM local.
- Le filtre des caractères (`pixel.py:177`) et la reconnaissance du nom de l'assistant
  sont les endroits à risque Unicode.
- Oracle : en mode mots-clés, pour une liste de messages de chat et un coffre actif fixé,
  les commandes envoyées au jeu et les événements écrits.

**Vérification automatique.** Les commandes communes ; l'oracle passe ; aucun message de
Pixel ne contient la réponse du coffre actif ; le texte du chat n'apparaît ni dans les
journaux ni dans la base.

**Vérification en jeu.** Sur le serveur d'essai, `lancer` en cours, un coffre posé. Dans le
chat du jeu (`T`) : `Pixel, aide-moi`, trois fois de suite, puis `Pixel, où est le coffre ?`.
Les trois paliers d'indices arrivent dans l'ordre, puis le retour au coffre, comme avec la
version Python.

### Phase 10 : Telegram et les commandes des parents

**Contexte.** Dernier morceau porté : après cette phase, le binaire Rust fait tout ce que
fait le Python.

**Fichiers.**
- `src/adapters/channel/base.rs`, `telegram.rs`.
- `src/orchestrateur.rs` : `CommandesParents`, `NotificateurCanal`, `servir`
  (`orchestrateur.py:200-453`).
- `src/cli.rs` : `lancer` complet (`cli.py:94-140`).
- `tests/telegram.rs`, `tests/orchestrateur.rs` (la partie commandes).
- `scripts/oracle.py`, `tests/donnees/oracle/commandes_parents.json`.

**Conception.**
- `teloxide` sans son répartiteur : une tâche interroge `getUpdates`, filtre
  (conversation privée, parent autorisé), télécharge la plus grande taille d'une photo en
  mémoire, et pousse des `MessageParent` dans un canal `tokio::mpsc`. `/start` répond son
  identifiant à n'importe qui, et rien d'autre.
- Fonctions de `teloxide` réduites au nécessaire, TLS `rustls`, même version de `reqwest`
  que le LLM : à vérifier par `cargo tree -d`, qui ne doit pas montrer deux `reqwest`.
- `decouper` coupe à 4 000 **caractères**, sur les fins de ligne.
- Un message Telegram non remis est journalisé, la boucle continue.
- Oracle : pour chaque commande (`/coffre`, `/file`, `/annuler`, `/annuler tout`,
  `/rapport`, `/oui`, `/non`, texte libre, photo sans LLM, photo avec LLM distant), la
  réponse exacte, sur une base et une horloge fixées.

**Vérification automatique.** Les commandes communes ; l'oracle passe ; un expéditeur
inconnu ou un message de groupe ne reçoit rien ; une lecture de photo vieille de plus de
30 minutes n'est plus confirmable.

**Vérification en jeu.** Avec le bot d'essai et le serveur d'essai,
`cargo run -- --config config.essai.toml lancer`. Sur Telegram : `/start` (l'identifiant
revient), `/coffre division 2`, `/file`, puis ouvrir le premier coffre en jeu (gestes de la
phase 7) : Telegram annonce la pose, puis l'ouverture. `/annuler tout`, `/rapport`. Envoyer
la photo d'une fiche avec `2` en légende : « Je lis la photo… », la liste, puis `/oui`.
`/atelier` : l'atelier se déplace près du joueur.

### Phase 11 : La bascule

**Contexte.** La seule phase qui retire. Elle ne commence que si les dix précédentes sont
cochées et leurs scénarios joués.

**Fichiers.**
- Supprimés : `src/mine_de_rien/` (les deux `.toml` du lexique déménagent dans
  `src/exercises/anglais/donnees/`), `tests/*.py`, `tests/__init__.py`, `pyproject.toml`,
  `uv.lock`, `scripts/verifier_version.py`, `scripts/oracle.py`, le test croisé
  Rust/Python de la phase 4.
- `docker/agent.Dockerfile`, `docker-compose.yml`, `docker/essai.compose.yml`,
  `.dockerignore`, `.gitignore`.
- `.github/workflows/publier.yml`.
- `tests/version.rs` (nouveau).
- `CLAUDE.md`, `README.md`, `docs/architecture.md`, `docs/installation.md`,
  `docs/publier-une-version.md`, `datapacks/README.md`.
- `.claude/skills/next/SKILL.md`, `nouvel-exercice/SKILL.md`, `nouveau-jeu/SKILL.md`,
  `essai-en-jeu/SKILL.md`, `feature/SKILL.md`.

**Conception.**
- Avant de supprimer : la liste des `def test_` de `tests/*.py` est comparée à celle des
  tests Rust. Tout nom manquant est porté, ou inscrit dans « Écarts assumés ».
- Dockerfile en deux étages. Le premier tourne sur la plateforme de la CI
  (`--platform=$BUILDPLATFORM`) et compile pour `x86_64-unknown-linux-musl` ou
  `aarch64-unknown-linux-musl` selon `TARGETARCH` ; SQLite embarqué demande un compilateur
  C croisé (`cargo-zigbuild` ou la chaîne `musl` : à choisir ici). Le second est
  `FROM scratch` : le binaire, les étiquettes OCI, `CMD` inchangé.
- Sans interpréteur de commandes dans l'image : toutes les commandes du compose sont déjà
  en forme de liste. `docker compose exec agent uv run --no-sync mine-de-rien …`
  (`docs/installation.md:90`, `:205`) devient `docker compose exec agent mine-de-rien …`.
- L'image tourne avec le même utilisateur qu'aujourd'hui, pour que `data/agent` reste
  lisible sans changer de droits.
- CI : `cargo fmt --check`, `clippy`, `cargo test`, puis le tag comparé à
  `mine-de-rien --version`. `tests/version.rs` vérifie que les étiquettes de l'image citées
  dans `docker-compose.yml` égalent la version de `Cargo.toml`.
- `CLAUDE.md` : commandes `cargo`, arborescence sans `mine_de_rien/`, règle « la réponse
  est calculée par le générateur, en Rust ». `docs/architecture.md` reçoit une section
  datée « Passage à Rust », avec le lien vers cette spec.
- La spec de la dictée n'est pas réécrite ici : une ligne en tête de
  `docs/specs/draft/2026-11-dictee-audio.md`, si elle y est toujours, dit que ses
  chemins datent du Python.

**Vérification automatique.**
- Les commandes `cargo` communes ; `git ls-files '*.py' pyproject.toml uv.lock` ne rend rien ;
- `docker build -f docker/agent.Dockerfile .` réussit pour `linux/amd64` et `linux/arm64`
  (`docker buildx build --platform …`), sans QEMU pour la compilation ; l'image pèse moins
  de 40 Mo (à mesurer, et noter la taille de l'image 0.1.2 à côté) ;
- `docker run --rm <image> mine-de-rien --version` affiche la version ;
- `docker compose -f docker/essai.compose.yml up` : le service ponctuel `datapack` écrit le
  datapack, le serveur démarre avec, l'agent se connecte ;
- `docker compose stop agent` rend la main en moins de 3 secondes (arrêt propre) ;
- l'agent Rust démarré sur une **copie** de la base de la maison répond à `/rapport` avec
  les sept derniers jours réels.

**Vérification en jeu.** La recette complète, sur le serveur d'essai, avec l'image
construite et non `cargo run` : rejouer d'affilée les scénarios des phases 7, 9 et 10, puis
vérifier que le mode paisible suit les coffres (un monstre ne peut plus apparaître tant
qu'un coffre attend ; à l'ouverture du dernier, le chat annonce leur retour). Ensuite
seulement, après fusion et publication : copier `data/agent` de côté, mettre à jour le vrai
serveur, et faire ouvrir un coffre à l'enfant sans rien lui dire. S'il ne remarque rien,
c'est fini.

## Retours de jeu

*(à remplir au fil des phases : écarts constatés avec le Python, choix tranchés en cours
de route, taille de l'image.)*

**Phase 1.**
- `[llm] adaptateur` reste un texte, pas un `enum` : une valeur inconnue ne doit pas
  empêcher le lancement, elle désactive le LLM, comme en Python. `[jeu]` et `[canal]` sont
  des `enum` : une valeur inconnue y est maintenant refusée à la lecture.
- La substitution des `${VARIABLES}` se fait après l'analyse du TOML, comme en Python, et
  non avant comme l'écrivait la conception : une valeur contenant un guillemet ne casse
  pas le fichier.
- Les variables d'environnement sont injectées dans `lire_config` : `std::env::set_var`
  est `unsafe` en édition 2024, et le crate interdit `unsafe`.
- Le runtime `tokio` n'est pas encore démarré dans `main` : il le sera à la phase 5, la
  première qui en a besoin.
- `pytest` annonce 860 tests passés : les 261 fonctions de test sont paramétrées.

**Phase 2.**
- Défaut du Python trouvé par l'oracle et **reproduit tel quel** : après un opérateur, la
  parenthèse ouvrante est suivie d'une espace (« (1 + 2) × ( 3 + 4) = ? »). À corriger dans
  un autre chantier, pas ici.
- Écart assumé : seuls `0` à `9` sont des chiffres. Le Python lisait aussi les chiffres
  d'autres écritures (« ٣ + 1 » valait 4) ; Rust répond « Caractère non reconnu ».
- `src/alea.rs` enveloppe `rand` : les générateurs ne voient que `Alea`, ce qui isole le
  projet des changements d'API de `rand` (0.10 a renommé ses traits).
- Les composants texte sont écrits par un sérialiseur maison qui reproduit `json.dumps` de
  Python (séparateurs `", "` et `": "`), pour que les commandes soient identiques à l'octet.

**Phase 3.**
- L'oracle des indices porte 2 678 cas (paliers de plus de 2 000 exercices tirés par le
  Python, filtre, nombres en lettres) : tous identiques, hors chiffres non ASCII.
- `cle_valide` ne passe plus par une fraction : une clé en chiffres est valide si elle a
  exactement l'écriture d'`ecrire_reponse`, ce qu'une expression régulière suffit à dire.
  Les 47 clés limites de l'oracle donnent le même verdict.
- Écart assumé : une réponse de générateur inutilisable dans un coffre était une
  `RuntimeError` qui tuait la tâche Telegram ; c'est maintenant un refus rendu au parent.
- Les tests d'indices sur les doubles et moitiés construisent l'exercice à la main, comme
  le fait la lecture de photo, qui n'arrive qu'en phase 8.
- Les fichiers de l'oracle sont écrits un cas par ligne (912 Ko en tout).
- Les deux `.toml` du lexique sont embarqués depuis leur dossier Python jusqu'à la phase 11.

**Phase 4.**
- `tests/donnees/bases/v1.sqlite`, `v2.sqlite`, `v3.sqlite` sont écrites par
  `scripts/oracle.py` avec les mêmes neuf coffres (v3 en a un dixième, de source photo).
  Rust les rouvre toutes en version 3 et y lit exactement ce que Python y lit.
- Le test croisé `python_relit_une_base_ecrite_par_rust` lance `uv run python` : à retirer
  en phase 11, comme prévu.
- Les dates sont écrites à la seconde (`2026-11-08T18:45:26+00:00`), sans microsecondes :
  c'est ce que faisait `_iso`, contrairement à ce que disait « Pièges de traduction ».
- Toutes les méthodes du dépôt rendent un `Result` : une erreur SQLite pendant une lecture
  était une exception non rattrapée en Python, c'est maintenant une `ErreurStockage` que la
  boucle sait traverser. `statut` et `source` sont des `enum` ; une valeur inconnue dans la
  base est une erreur, pas une panique (testé).
- `src/games/coffre_du_soir.rs` et `src/games/pixel.rs` existent déjà, réduits à
  `formater_duree` et `EVENEMENT_INDICE` dont le rapport a besoin.
- Trois tests de `tests/test_rapport.py` passent par l'orchestrateur
  (`test_aucun_message_aux_parents_ne_contient_le_pseudo`,
  `test_notification_d_ouverture_compte_les_indices`) : ils seront portés en phases 7 et 10.
- Dépendance ajoutée : `chrono` (sans fuseaux, `std` et `now` seulement) et `rusqlite`
  avec SQLite embarqué. Ni l'une ni l'autre n'ouvre de connexion réseau.

**Phase 5.**
- Choix tenu pour tous les adaptateurs : un trait asynchrone objet (`async-trait`, `?Send`),
  partagé par `Rc<dyn AdaptateurJeu>`. Les faux jeux des tests implémentent le trait.
- Une seule erreur pour tout l'adaptateur de jeu, `ErreurRcon` : un pseudo refusé par
  `dire` ou un chemin de log manquant, qui étaient des `ValueError`, en sont maintenant.
- Le flux du chat n'est pas un canal `tokio::mpsc` mais un `FluxChat` qu'on interroge
  (`suivant().await`) : pas de tâche de fond à lancer ni à arrêter.
- Mieux qu'en Python, sans changement visible : une erreur à l'envoi autre qu'une connexion
  coupée (délai dépassé) déclenche aussi la reconnexion, au lieu de remonter en `OSError`
  que la boucle ne rattrapait pas. Un log illisible est journalisé au lieu d'arrêter Pixel.
- Les limites RCON sont bien en **octets** (1 446 pour une commande, 4 096 pour un morceau),
  comme en Python : testé avec des accents.
- `src/journal.rs` remplace `logging` : une ligne datée sur la sortie d'erreur, et une
  capture pour les tests qui vérifient qu'un texte n'est pas journalisé.
- Essai sur Paper 26.3 (`cargo test --test java_rcon -- --ignored`) : 600 commandes en
  rafale sur une seule connexion, accents à l'aller et au retour, reconnexion après
  `docker compose restart`. `dire` sans joueur connecté rend « No player was found », comme
  la version Python.
- **Non vérifié sur le vrai serveur** : la réponse de plus de 4 096 caractères. Paper 26.3
  abrège lui-même ses longues sorties (« [a], [b], ... » pour 300 objectifs,
  « components: {...} » pour `data get`) : je n'ai trouvé aucune commande qui en produise
  une. Le recollage par sentinelle reste couvert par le faux serveur.
- `ecouter` affiche l'heure locale de réception, comme en Python. Reste à jouer en jeu.

**Phase 6.**
- L'oracle des jeux est une **transcription** : `scripts/oracle.py` fait tourner le Python
  contre les faux mondes de ses propres tests et note chaque commande envoyée avec sa
  réponse. Rust rejoue la transcription (`tests/commun/mod.rs`, `Rejeu`) : toute commande
  qui diffère, manque ou s'ajoute fait échouer le test. 128 cas ici (8 mondes, 13 terrains,
  5 serveurs).
- `ErreurAtelier` et `ErreurCoffre` sont des `enum` qui distinguent le refus (message pour
  le parent) de la panne RCON : en Python c'étaient deux exceptions différentes, traitées
  différemment par l'appelant.
- Une position enregistrée illisible (`[` sans `]`) faisait lever une `ValueError` non
  rattrapée en Python ; en Rust elle vaut « pas d'atelier », comme les autres cas illisibles.
- Essai sur Paper 26.3 : atelier démonté par RCON, construit par Python, démonté, construit
  par Rust. Treize relevés du monde (coffre et son orientation, enclume, lanterne, les 25
  blocs du plancher, marqueur au centre du bloc, position enregistrée, chunks gardés
  chargés) sont identiques ; relancé, Rust répond « réparé » sans rien changer.
- Les tests de `tests/test_paisible.py` qui passent par l'orchestrateur sont portés en
  phase 7.
- Reste à jouer en jeu : la zone en mode aventure autour de l'atelier (c'est le datapack,
  inchangé, qui la fait).

**Phase 7.**
- La boucle voit le coffre par un trait `Coffre` : le vrai `CoffreDuSoir`, ou le faux des
  tests, comme le faisait le typage canard en Python.
- `ErreurTour` réunit tout ce qui peut interrompre un tour : la boucle les journalise
  toutes et repart, y compris celles que Python ne rattrapait pas (une `OSError` à l'envoi).
- Oracle : 316 cas transcrits. Les commandes de pose sur sept énoncés (accents, guillemets,
  antislash, saut de ligne, énoncé trop long), quatorze méthodes du coffre sur dix mondes
  avec et sans Pixel, et onze scénarios de surveillance joués tour par tour (enfant absent
  puis connecté, ouverture avec indices, coffre disparu, atelier absent puis construit, pose
  refusée, serveur en panne un tour). Mêmes commandes, mêmes messages à l'enfant, mêmes
  notifications, même base à la fin.
- Les tests de `test_coffre_du_soir.py`, ceux de `test_paisible.py` et de `test_rapport.py`
  qui passaient par l'orchestrateur sont portés. Il n'y a pas de `tests/orchestrateur.rs`
  dans cette phase : `tests/test_orchestrateur.py` ne teste que les commandes des parents,
  il sera porté en phase 10.
- Essai sur Paper 26.3 : `coffre "348 ÷ 12"` puis `lancer`, en Rust puis en Python, sur un
  monde remis à zéro entre les deux. Onze relevés identiques : verrou « 29 », 3 diamants en
  case 13, « Preuve n°1 » marquée `{mdr_coffre: 1}` en case 11, panneau ciré
  `["348 ÷ 12 = ?", "", "", ""]`, enclume, difficulté paisible. `SIGTERM` arrête l'agent
  Rust en 55 ms.
- `lancer` ne démarre encore ni Pixel (phase 9) ni Telegram (phase 10).
- Reste à jouer en jeu : renommer la clé sur l'enclume, ouvrir le coffre, la notification
  d'ouverture, le coffre reposé après disparition.

**Phase 8.**
- `reqwest` est pris en version 0.12 et non 0.13 : c'est celle de `teloxide` 0.17, qui
  arrive en phase 10. Une seule version dans le binaire. TLS `rustls`, racines embarquées.
- Le contrat du LLM est un trait, comme celui du jeu. L'ébauche Anthropic rend une
  `ErreurLlm` quand on l'appelle sans image ; en Python c'était une `NotImplementedError`
  que personne ne rattrapait. Elle ne fait toujours aucun appel réseau.
- Les tests du LLM tournent contre un petit serveur HTTP local écrit dans le test, à la
  place de `httpx.MockTransport`. Ajoutés : le délai dépassé et quatre réponses au format
  inattendu.
- Images : crate `image` limité aux décodeurs JPEG et PNG. Pillow en lisait davantage
  (WebP, HEIC selon l'installation) ; Telegram n'envoie que du JPEG, seul `lire-photo` en
  ligne de commande est concerné. Écart assumé.
- La taille des morceaux reproduit l'arrondi de `Image.thumbnail` : sur 17 tailles
  d'image, Rust et Pillow donnent les mêmes dimensions. Le redressement EXIF est vérifié
  sur une image de synthèse (`tests/donnees/photo/orientation-6.jpg`, deux aplats de
  couleur, rien de personnel).
- Les pixels, eux, diffèrent : autre filtre de réduction, autre encodeur JPEG. Les tests ne
  peuvent pas dire si le modèle lit aussi bien.
- Oracle : 80 cas (consignes au modèle au caractère près, 35 calculs recopiés, les quatre
  fiches enregistrées, lignes mal formées citées comme le faisait `str()` de Python).
- Essai avec le LLM local de la maison (Gemma 4 E4B) : `llm "…"` répond ; `lire-photo`
  sur une **fiche de synthèse** fabriquée pour l'essai (huit lignes imprimées) rend les
  mêmes huit exercices en Python et en Rust, en 19 s et 24 s. Seule différence : Python a
  compté le titre comme « texte hors lexique », Rust ne l'a pas vu ; il est ignoré dans les
  deux cas.
- **Non fait** : l'essai sur deux vraies fiches photographiées demandé par la spec. Les
  photos ne sont pas dans le dépôt (elles montrent le prénom de l'enfant) et je n'en ai
  pas. À faire par le parent : `lire-photo` sur une fiche déjà lue par la version Python.
