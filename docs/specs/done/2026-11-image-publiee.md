# Mine de rien s'installe en téléchargeant une image, datapack compris, sans cloner le dépôt

> **Reprise de session** : lire « Pourquoi », « Décisions actées » et « Statut »,
> puis aller à la première phase non terminée.

## Pourquoi

Aujourd'hui, installer Mine de rien demande le dépôt entier sur la machine qui fait tourner
Docker :

- **L'agent est construit sur place.** `docker-compose.yml` déclare `build:` avec
  `docker/agent.Dockerfile` ; il n'existe aucune image publiée. Toute machine qui veut
  l'agent doit avoir le code source et refaire la construction.
- **Le datapack est monté depuis le clone.** Le serveur Minecraft lit
  `./datapacks:/datapacks:ro` (`docker-compose.yml`, service `minecraft`). Sans le dépôt, pas
  de zone en mode aventure autour de l'atelier : l'enfant peut casser le coffre
  (`src/mine_de_rien/games/coffre_du_soir.py:50`).
- **Rien n'est figé.** `docker/agent.Dockerfile` part de `python:3.12-slim` et
  `ghcr.io/astral-sh/uv:latest`, sans version précise : deux constructions du même commit
  peuvent donner deux images différentes. Le compose sert `itzg/minecraft-server` sans tag.
- **Le contexte de construction emporte tout.** Il n'y a pas de `.dockerignore` : un
  `docker compose build` sur le PC de la maison envoie au démon `data/` (le monde de
  l'enfant, la base SQLite, environ 470 Mo aujourd'hui), `.env` et `config.toml`. Le
  Dockerfile ne les copie pas dans l'image, mais rien ne l'empêche.

Pour déployer sur un autre hôte Docker que le PC de développement (un mini-PC, un
Raspberry Pi, un serveur déjà géré par ailleurs), il faut une image téléchargeable qui porte
l'agent **et** le datapack de la même version.

**Cible** : `docker compose up -d` sur une machine qui n'a que `docker-compose.yml`, `.env`
et `config.toml` télécharge `ghcr.io/sfauvart/mine-de-rien:<version>`. Un service ponctuel
recopie le datapack de cette version, le serveur Minecraft le charge, et l'agent démarre.
Chaque tag git `vX.Y.Z` publie l'image pour amd64 et arm64.

## Décisions actées

| Sujet | Décision |
|---|---|
| Registre | `ghcr.io/sfauvart/mine-de-rien`, paquet **public** : téléchargement anonyme, sans `docker login`. |
| Construction | GitHub Actions, dans un checkout propre du dépôt. L'image publiée n'est jamais construite ni poussée depuis un PC de la maison. |
| Déclencheur | Un tag git `vX.Y.Z` seulement. Rien n'est publié sur un simple push. Le workflow refuse de publier si pytest ou ruff échoue, ou si le tag ne correspond pas à la version de `pyproject.toml` et au tag d'image de `docker-compose.yml`. |
| Tags d'image | `X.Y.Z`, `X.Y` et `latest`. Le compose du dépôt cite toujours `X.Y.Z`, jamais `latest`. |
| Architectures | `linux/amd64` et `linux/arm64`, dans une seule image multi-plateforme. |
| Datapack | Copié dans l'image, à `/app/datapacks/mine_de_rien`. La sous-commande `mine-de-rien datapack <dossier>` le recopie dans un dossier partagé avec le serveur Minecraft. Elle ne lit pas `config.toml`. |
| Livraison au serveur | Un service compose ponctuel, `datapack`, lance cette sous-commande depuis l'image de l'agent. `minecraft` attend qu'il ait réussi (`service_completed_successfully`). Une seule version gouverne le code et le datapack. |
| Images de base | Figées sur une version précise : `python:3.12-slim` passe à un tag `3.12.x-slim-<debian>`, `uv` à un tag `0.x.y`, `itzg/minecraft-server` du compose au tag validé sur le serveur d'essai. |
| Compose du dépôt | Utilise l'image publiée par défaut. `build:` reste à côté, pour construire une version locale de l'agent. |
| Serveur d'essai | Inchangé : `docker/essai.compose.yml` continue de monter `./datapacks` depuis la copie de travail, pour essayer un datapack avant de le publier. |
| Mention d'un déploiement particulier | Aucune. Le dépôt décrit une image et un compose génériques ; la façon dont un hôte précis les déploie reste hors du dépôt. |

### Ce que ces décisions renversent

- `docs/installation.md`, étape 1, commence par `git clone`. Le clone devient facultatif : il
  suffit de récupérer trois fichiers. L'étape 6 (`docker compose up -d --build agent`) n'a
  plus de `--build`.
- `datapacks/README.md` dit que le datapack est « monté dans les deux serveurs ». Ce n'est
  plus vrai que pour le serveur d'essai ; le serveur réel le reçoit de l'image.

## Hors périmètre

- **Publication à chaque push sur `main`** (tag `main` ou `sha-…`) : écartée, seuls les
  tags de version publient.
- **Image Minecraft sur mesure** (Paper avec le datapack embarqué) : écartée, on garde
  `itzg/minecraft-server` tel quel.
- **Zip du datapack en asset de release**, téléchargé par la variable `DATAPACKS` d'itzg :
  écarté au profit du datapack dans l'image, pour ne pas ajouter de téléchargement au
  démarrage du serveur.
- **Seconde image pour le datapack** : écartée, deux images à garder synchrones.
- **Utilisateur non root dans l'image** : écarté pour ce chantier. Les installations
  existantes ont `data/agent` créé par root ; changer d'utilisateur demanderait une
  migration des droits. À rouvrir à part.
- **Mises à jour automatiques des images de base** (Dependabot, Renovate) : hors de ce
  chantier.
- **Signature d'image, SBOM, attestation de provenance** : hors de ce chantier, sauf si
  l'action de build les ajoute d'elle-même.
- **Service `llm`** (profil `local-llm`) : inchangé.

## Vie privée

- **Ce qui part vers ghcr.io** : l'image, construite par GitHub Actions à partir du code du
  dépôt et du datapack. Elle ne contient ni `.env`, ni `config.toml`, ni `data/` : la
  construction se fait dans un checkout propre, et le `.dockerignore` de la phase 1 exclut
  ces chemins même pour une construction locale. Le datapack ne contient aucune coordonnée
  ni aucune réponse (`datapacks/README.md`).
- **Ce que la maison contacte en plus** : ghcr.io, au téléchargement de l'image (installation
  et mise à jour), comme Docker Hub aujourd'hui pour `itzg/minecraft-server` et `python`.
  C'est explicite : l'image est nommée dans `docker-compose.yml`. Rien n'est contacté à
  l'exécution.
- **Alternative auto-hébergée** : `docker compose build agent` reconstruit l'image depuis le
  clone, sans ghcr.io.

## Statut

**Branche** : `feat/image-publiee`, créée depuis `main` à la phase 1, fusionnée à la clôture.

- [x] **Phase 1 : une image qui porte l'agent et son datapack**
- [x] **Phase 2 : un tag git publie l'image sur ghcr.io**
- [x] **Phase 3 : le compose et le guide utilisent l'image publiée**

Close le 2026-11-09.

La phase 2 construit l'image de la phase 1. La phase 3 a besoin d'une image réellement
publiée, donc d'un premier tag poussé à la main après la phase 2.

## Conception

### Phase 1 : une image qui porte l'agent et son datapack

**Contexte.** L'image actuelle (`docker/agent.Dockerfile`) ne copie que `pyproject.toml`,
`uv.lock`, `LICENSE`, `README.md` et `src/`. Le datapack reste hors de l'image.
`charger_config` est appelé avant toute sous-commande (`src/mine_de_rien/cli.py:169`) : une
sous-commande sans `config.toml` doit être traitée avant cette ligne.

**Fichiers.**
- `docker/agent.Dockerfile`
- `.dockerignore` (nouveau)
- `src/mine_de_rien/cli.py`
- `src/mine_de_rien/datapack.py` (nouveau)
- `tests/test_datapack.py` (nouveau)
- `datapacks/README.md`

**Conception.**
- **Sous-commande `mine-de-rien datapack <dossier>`.** Elle recopie `mine_de_rien` dans
  `<dossier>/mine_de_rien` et affiche ce qu'elle a fait. Elle est traitée avant
  `charger_config`, et ne demande ni `--config` ni réseau.
  - *Source.* Le dossier `datapacks/mine_de_rien` à la racine du projet, retrouvé depuis
    `datapack.py`. Dans l'image, le projet est installé en mode éditable par `uv sync` : la
    racine est `/app` et le Dockerfile y copie `datapacks/`. En développement, la racine est
    le clone. Une option `--source` permet de forcer un autre dossier.
  - *Remplacement complet.* Copie dans un dossier temporaire voisin, suppression de
    l'ancien `mine_de_rien`, puis renommage. Un fichier retiré du datapack disparaît donc
    aussi de la destination. La commande se relance sans risque.
  - *Garde-fous.* Elle refuse une source sans `pack.mcmeta` et une destination qui est la
    source elle-même. Elle crée `<dossier>` s'il manque.
- **Dockerfile.**
  - Images de base figées : `python:3.12.16-slim-trixie` et `ghcr.io/astral-sh/uv:0.13.7`,
    relevées le 2026-11-08, publiées toutes deux pour amd64 et arm64.
  - `COPY datapacks ./datapacks`.
  - `ENV PATH=/app/.venv/bin:$PATH`, pour que `mine-de-rien` se lance directement. Les
    commandes `uv run --no-sync mine-de-rien …` du guide continuent de marcher.
  - Labels OCI : `org.opencontainers.image.source` (l'URL du dépôt GitHub, qui relie le
    paquet ghcr.io au dépôt), `description` et `licenses=MIT`. `version` et `revision`
    sont passés en `ARG` par le workflow de la phase 2.
  - Le `CMD` de l'agent reste `lancer`.
- **`.dockerignore`.** Exclut `data/`, `.env`, `config.toml`, `config.essai.toml`, `.git`,
  `.venv`, les caches et `tests/`.

**Vérification automatique.**
- `uv run pytest` : `tests/test_datapack.py` vérifie
  - la copie complète de l'arborescence ;
  - qu'un second appel remplace sans dupliquer ;
  - qu'un fichier retiré de la source disparaît de la destination ;
  - le refus d'une source sans `pack.mcmeta` ;
  - qu'aucun `config.toml` n'est lu (appel de `main(["datapack", tmp])` dans un dossier sans
    config).
- `uv run ruff check`.
- `docker build -f docker/agent.Dockerfile -t mdr:essai .`, puis :
  - `docker run --rm -v "$PWD/data/essai-datapack:/out" mdr:essai mine-de-rien datapack /out`
    produit `data/essai-datapack/mine_de_rien/pack.mcmeta` ;
  - `docker run --rm mdr:essai mine-de-rien --version` affiche la version ;
  - `docker run --rm mdr:essai ls -a /app` ne montre ni `data`, ni `.env`, ni `config.toml`.
- Construction arm64 locale si QEMU est installé sur le poste
  (`docker buildx build --platform linux/arm64 …`), sinon reportée à la phase 2.

**Vérification en jeu.** Aucune pour cette phase : le serveur d'essai continue de monter
`./datapacks`.

### Phase 2 : un tag git publie l'image sur ghcr.io

**Contexte.** Le dépôt est sur GitHub (`git@github.com:sfauvart/mine-de-rien.git`) et n'a
encore aucun tag ; `pyproject.toml` est en `0.1.0`. On croyait qu'un paquet ghcr.io serait
privé à sa première publication ; à la publication de v0.1.0, il a pris la visibilité
publique du dépôt, sans rien régler.

**Fichiers.**
- `.github/workflows/publier.yml` (nouveau)
- `docs/publier-une-version.md` (nouveau, la procédure de version)

**Conception.**
- **Déclencheur.** `on: push: tags: ["v*.*.*"]`.
- **Job `verifier`.**
  - `uv sync`, `uv run pytest`, `uv run ruff check`.
  - Contrôle de cohérence (`scripts/verifier_version.py`) : la version du tag (sans le `v`)
    doit égaler `version` dans `pyproject.toml` et `__version__` dans
    `src/mine_de_rien/__init__.py`, que la phase a trouvé en double. La phase 3 ajoutera le
    tag d'image de `docker-compose.yml` à ce contrôle.
- **Job `publier`**, qui dépend de `verifier`.
  - `permissions: packages: write, contents: read`, connexion à ghcr.io avec `GITHUB_TOKEN`.
  - `docker/setup-qemu-action`, `docker/setup-buildx-action`, `docker/metadata-action`
    pour les tags `X.Y.Z`, `X.Y`, `latest`.
  - `docker/build-push-action` avec `platforms: linux/amd64,linux/arm64`, `file:
    docker/agent.Dockerfile` et les `build-args` de version et de révision. Cache GitHub
    Actions (`type=gha`).
  - Actions épinglées sur une version précise.
- **Procédure de version** (`docs/publier-une-version.md`) :
  1. changer `version` dans `pyproject.toml` et le tag d'image dans `docker-compose.yml`
     (à partir de la phase 3) ;
  2. commit sur `main` ;
  3. `git tag vX.Y.Z` puis `git push origin vX.Y.Z` ;
  4. suivre le workflow.

  Visibilité : héritée du dépôt public, rien à régler (constaté sur v0.1.0).

**Vérification automatique.**
- `actionlint` sur le workflow, lancé par conteneur : `docker run --rm -v "$PWD:/repo"
  -w /repo rhysd/actionlint:<version>`.
- Le contrôle de cohérence, extrait dans un petit script shell ou Python, est testé en local
  contre un tag fictif : un tag qui ne correspond pas fait échouer le script.

**Vérification manuelle** (après la phase : le tag se pousse seulement quand le parent le
demande).
- Pousser `v0.1.0`. Le workflow est vert dans l'onglet Actions.
- ~~Passer le paquet en public.~~ Inutile : il l'était déjà, hérité du dépôt.
- Depuis une machine non connectée à ghcr.io :
  `docker buildx imagetools inspect ghcr.io/sfauvart/mine-de-rien:0.1.0` liste `linux/amd64`
  et `linux/arm64`, et `docker run --rm ghcr.io/sfauvart/mine-de-rien:0.1.0 mine-de-rien
  --version` affiche `0.1.0`.

### Phase 3 : le compose et le guide utilisent l'image publiée

**Contexte.** Il faut que `ghcr.io/sfauvart/mine-de-rien:0.1.0` existe et soit public
(phase 2). Le compose actuel construit l'agent et monte `./datapacks`.

**Fichiers.**
- `docker-compose.yml`
- `.github/workflows/publier.yml` (contrôle de cohérence étendu)
- `docs/installation.md`
- `docs/publier-une-version.md`
- `README.md`
- `datapacks/README.md`

**Conception.**
- **`docker-compose.yml`.**
  - Nouveau service `datapack` : `image: ghcr.io/sfauvart/mine-de-rien:0.1.0`,
    `command: ["mine-de-rien", "datapack", "/datapacks"]`,
    volume `./data/datapacks:/datapacks`, pas de `restart`.
  - `minecraft` : volume `./data/datapacks:/datapacks:ro` à la place de `./datapacks`,
    `depends_on: datapack: condition: service_completed_successfully`, image
    `itzg/minecraft-server:2026.10.1-java25` figée. Le serveur d'essai tourne sur `latest`,
    un build du 2026-11-06 sans tag versionné : on a pris la dernière version publiée,
    même Java 25, et vérifié qu'elle sert Paper 26.3 (essai sur dossier vide ci-dessous).
  - `agent` : `image: ghcr.io/sfauvart/mine-de-rien:0.1.0`, en gardant le `build:` actuel.
- **Comportement vérifié** (2026-11-08) : avec `image:` et `build:` ensemble,
  `docker compose up -d` télécharge l'image publiée sans construire (essai sur un dossier
  sans Dockerfile), et `docker compose build agent` construit en local sous le même nom ;
  `docker compose pull agent` remet ensuite l'image publiée. Ni `pull_policy` ni fichier
  séparé.
- **Contrôle de cohérence** du workflow : le tag d'image `mine-de-rien` des services `agent`
  et `datapack` doit égaler la version du tag git.
- **`docs/installation.md`.**
  - Étape 1 : récupérer `docker-compose.yml`, `.env.example` et `config.example.toml`
    (liens bruts vers le tag de la version), le clone n'étant plus qu'une option.
  - Étape 6 : `docker compose up -d agent`, sans `--build`.
  - Nouvelle section « Mettre à jour » : changer le tag dans `docker-compose.yml`, puis
    `docker compose pull && docker compose up -d`. Le datapack suit.
  - « En cas de souci » : vérifier que `data/datapacks/mine_de_rien` existe, avec
    `docker compose logs datapack`.
- **`README.md`** : installation en trois fichiers, et une mention de l'image publiée.
- **`datapacks/README.md`** : le serveur réel reçoit le datapack de l'image, le serveur
  d'essai le monte depuis la copie de travail.

**Vérification automatique.**
- `docker compose config -q`.
- `docker compose pull` télécharge l'image publiée.
- Sur un dossier vide qui ne contient que `docker-compose.yml`, `.env` et `config.toml` (copie
  du modèle, `EULA=TRUE`, pas de jeton Telegram), `docker compose up -d` doit :
  - faire sortir `datapack` avec le code 0 ;
  - démarrer `minecraft` jusqu'à « Done » ;
  - montrer dans le log de `minecraft` que `world/datapacks/mine_de_rien` est copié ;
  - laisser `rcon-cli datapack list` citer `file/mine_de_rien` parmi les packs actifs.
- Version modifiée par erreur dans le compose : le script de cohérence de la phase 2
  échoue.

**Vérification en jeu.**
Sur l'installation de la maison, après `docker compose pull && docker compose up -d`, avec
le monde de l'enfant conservé dans `data/minecraft` :
1. Se connecter avec le lanceur de l'enfant et marcher jusqu'à l'atelier en survie : en
   arrivant près du coffre, le jeu passe en mode aventure (le coffre ne se casse plus au
   clic gauche).
2. S'éloigner de quelques blocs : le jeu repasse en survie.
3. Depuis Telegram, `/coffre 348 ÷ 12` : le coffre apparaît, s'ouvre avec la clé renommée
   « 29 », et les parents reçoivent « Coffre ouvert ». L'agent qui tourne vient de l'image
   publiée (`docker compose images agent` affiche `ghcr.io/sfauvart/mine-de-rien` `0.1.0`).

## Retours de jeu

- 2026-11-09 : scénario de la phase 3 joué sur l'installation locale, conforme.
