# Installer Mine de rien à la maison

Ce guide suffit pour installer Mine de rien sans avoir suivi son développement. À la fin,
un parent tape `/coffre 348 ÷ 12` sur Telegram, un coffre verrouillé apparaît près de la
base de l'enfant dans Minecraft, et les deux parents sont prévenus quand il l'ouvre.

Il faut :

- un PC sous Linux allumé quand l'enfant joue, avec Docker et `docker compose` ;
- Minecraft **Java Edition** sur l'ordinateur de l'enfant (pas Bedrock, pas de console) ;
- un compte Telegram par parent.

## 1. Récupérer le projet et préparer les fichiers

```bash
git clone <adresse du dépôt> mine-de-rien
cd mine-de-rien
cp .env.example .env
cp config.example.toml config.toml
```

## 2. Créer le bot Telegram

1. Dans Telegram, ouvrir une conversation avec **@BotFather**.
2. Envoyer `/newbot`, choisir un nom (par exemple « Coffres de la maison ») puis un
   identifiant qui finit par `bot`.
3. BotFather répond un **jeton** de la forme `123456789:AAH...`. Le recopier dans `.env` :

   ```
   TELEGRAM_BOT_TOKEN=123456789:AAH...
   ```

Le jeton donne le contrôle du bot : ne pas le partager. Telegram est le seul service
extérieur utilisé ; ce qui y part se limite aux énoncés, aux réponses, aux numéros de
coffre et aux durées. Jamais le pseudo ni le prénom de l'enfant.

## 3. Remplir `.env`

- **`EULA`** : lire le contrat de Minecraft (https://aka.ms/MinecraftEULA), puis passer
  `EULA=TRUE` si vous l'acceptez. Sans cela, le serveur ne démarre pas.
- **`MC_VERSION`** : laisser la version proposée. C'est celle sur laquelle les commandes de
  Mine de rien ont été vérifiées. Le lanceur de l'enfant devra utiliser exactement la même.
- **`ONLINE_MODE`** : `TRUE` si l'enfant joue avec un compte Microsoft (lanceur officiel),
  `FALSE` pour un lanceur hors ligne comme XMCL. Lire
  [rejoindre-le-serveur.md](rejoindre-le-serveur.md) avant de choisir `FALSE`.
- **`RCON_PASSWORD`** : remplacer `change-moi` par un mot de passe long, tiré au hasard.
  Il ne sert qu'entre le serveur et l'agent, personne n'a à le retenir :

  ```bash
  openssl rand -hex 16
  ```

- **`ANTHROPIC_API_KEY`** : laisser vide. Le seul usage d'un LLM, la lecture des photos de
  fiches, passe par un LLM local (étape 4).

## 4. Remplir `config.toml`

- **`[enfant] pseudo_minecraft`** : le pseudo exact de l'enfant dans Minecraft, celui du
  compte Microsoft ou du compte hors ligne. Il ne quitte jamais la maison.
- **`[coffre]`** : la récompense de chaque coffre (3 diamants par défaut) et les niveaux
  d'expérience offerts pour les essais (3 par défaut, un renommage coûte un niveau).
- **`[atelier]`** : on y reviendra à l'étape 7, une fois dans le jeu.
- **`[canal] parents`** : on y reviendra à l'étape 6.
- **`[llm]`** : facultatif, pour lire les photos de fiches (étape 10). Un serveur local
  compatible OpenAI avec un modèle qui voit les images : `llama-server` de llama.cpp,
  Ollama ou LM Studio, sur une machine de la maison. Essayé avec Gemma 4 E4B (version QAT,
  4 Go) :

  ```toml
  [llm]
  adaptateur = "openai-compatible"
  url = "http://192.168.1.16:8001/v1"   # l'adresse du serveur, terminée par /v1
  modele = "gemma"                      # identifiant exact donné par {url}/models (Ollama l'exige)
  ```

  Vérifier, une fois le serveur démarré (étape 5) :
  `docker compose exec agent uv run --no-sync mine-de-rien --config /app/config.toml llm "Bonjour ?"`
  doit répondre. Avec `adaptateur = "anthropic"`, aucune photo n'est lue : une photo ne
  part jamais vers un LLM distant.

## 5. Démarrer le serveur, importer le monde

Pour garder le monde dans lequel l'enfant joue déjà (en solo), copier son dossier **avant**
le premier démarrage. Sous Windows, les mondes sont dans
`%APPDATA%\.minecraft\saves\<nom du monde>`. Le dossier entier devient `world` :

```bash
mkdir -p data/minecraft
cp -r "/chemin/vers/saves/Mon monde" data/minecraft/world
```

Le monde doit avoir été ouvert pour la dernière fois dans une version égale ou antérieure à
`MC_VERSION`. Faire une copie de sauvegarde avant : le serveur le convertira.

Puis démarrer le serveur seul :

```bash
docker compose up -d minecraft
docker compose logs -f minecraft    # attendre « Done », puis Ctrl+C
```

Ajouter l'enfant à la liste blanche, déjà active mais vide :

```bash
docker compose exec minecraft rcon-cli whitelist add <pseudo de l'enfant>
```

Vérifier que **personne n'est opérateur**. Le fichier doit contenir `[]` :

```bash
cat data/minecraft/ops.json
```

Un enfant opérateur peut se donner des diamants, lire la réponse dans les données du coffre
et passer en créatif : le jeu n'aurait plus de sens. Si un pseudo apparaît, le retirer avec
`docker compose exec minecraft rcon-cli deop <pseudo>`.

## 6. Relever les identifiants Telegram des parents

Démarrer l'agent :

```bash
docker compose up -d --build agent
```

Chaque parent ouvre une conversation avec le bot et envoie `/start`. Le bot répond
« Ton identifiant Telegram : 123456789 ». Recopier les deux nombres dans `config.toml` :

```toml
[canal]
parents = [123456789, 987654321]
```

Tant qu'un identifiant n'est pas dans cette liste, le bot ignore ses messages : `/start`
est la seule commande ouverte à tous, et elle ne fait que donner l'identifiant.

Redémarrer l'agent pour qu'il lise la liste :

```bash
docker compose restart agent
```

## 7. Choisir l'emplacement de l'atelier

L'atelier est un plancher de 5 × 5 avec un coffre, une enclume, une lanterne et un panneau,
que Mine de rien construit une fois pour toutes. Les coffres apparaîtront toujours là.

1. Dans le jeu, aller près de la base de l'enfant, sur du sol naturel dégagé (herbe, terre,
   sable, pierre), avec 3 blocs d'air au-dessus.
2. Appuyer sur **F3** : la ligne « Block » donne les coordonnées du bloc visé. Se placer
   au centre de l'emplacement et relever x, y, z du bloc **juste au-dessus du sol** : c'est
   là que sera le coffre.
3. Les recopier dans `config.toml` :

   ```toml
   [atelier]
   x = 120
   y = 64
   z = -35
   ```

4. Construire l'atelier :

   ```bash
   docker compose restart agent
   docker compose exec agent uv run --no-sync mine-de-rien --config /app/config.toml atelier
   ```

   Si un bloc gêne (arbre, construction), Mine de rien refuse sans rien casser et dit où
   est ce bloc : choisir un autre emplacement ou dégager. La commande peut être relancée
   sans risque : elle répare sans dupliquer.

Autour de l'atelier, un joueur en survie passe en mode aventure : il ne peut ni casser le
coffre ni creuser dessous. Il repasse en survie en s'éloignant de quelques blocs.

## 8. Régler le lanceur de l'enfant

Créer une installation (lanceur officiel) ou une instance (XMCL) **exactement dans la
version de `MC_VERSION`**, sans loader, puis ajouter le serveur avec l'adresse IP locale
du PC. Détails et dépannage : [rejoindre-le-serveur.md](rejoindre-le-serveur.md).

## 9. Premier coffre

Depuis le téléphone d'un parent :

```
/coffre 348 ÷ 12
```

Les deux parents reçoivent « Coffre n°1 posé ». Dans le jeu, l'enfant entend une cloche et
lit le message ; à l'atelier, le panneau porte l'énoncé. Il a reçu une « Clé du coffre »
dont la description, comme le message du chat, explique les gestes : poser la clé à gauche
sur l'enclume, remplacer son nom par « 29 », cliquer sur la clé renommée à droite pour la
prendre, puis ouvrir le coffre en la tenant en main. Les parents reçoivent « Coffre n°1 ouvert ».

Commandes des parents :

| Commande | Effet |
|---|---|
| `/coffre 348 ÷ 12` | un calcul recopié du cahier, recalculé par Mine de rien |
| `/coffre division 3` | trois exercices d'une compétence (`division`, `anglais`) |
| photo d'une fiche | voir l'étape 10 |
| `/oui`, `/non` | met en file ou abandonne les exercices tirés d'une photo |
| `/file` | le coffre actif et ceux qui attendent |
| `/annuler 12` | retire le coffre n°12, du jeu ou de la file |
| `/annuler tout` | retire le coffre actif du jeu et vide la file |
| `/rapport` | les 7 derniers jours, et ce qui est à retravailler |

Les réponses s'écrivent avec une virgule pour les décimaux (`4,75`). Un calcul dont le
résultat ne tombe pas juste (`10 ÷ 3`) est refusé avec une explication. Les mots d'anglais
s'écrivent en minuscules, avec les accents du français (`août`, `dix-sept`).

## 10. Envoyer la photo d'une fiche

Il faut la section `[llm]` de l'étape 4. Dans la conversation avec le bot, envoyer la photo
d'une fiche **imprimée** de calculs (opérations, tables, « Double de… », « Moitié de… ») ou
de vocabulaire anglais (jours, mois, nombres jusqu'à 20, couleurs, salutations), avec en
légende le nombre de coffres voulus, et au besoin la partie de la page :

```
5 exercice 7
```

Le bot répond « Je lis la photo… », puis, après 20 secondes à 2 minutes, ce qu'il a lu
(« Doubles de (37) : 1, 2, 3… »), les calculs écartés, et les 5 exercices tirés au hasard
avec leur réponse, calculée par Mine de rien. **Comparer la liste à la fiche** : `/oui` met
les 5 coffres en file, `/non` abandonne. Sans réponse, la lecture est oubliée au bout de
30 minutes.

- **La photo doit être à l'endroit.** De travers, le modèle peut inventer des lignes
  plausibles (« Double de 21, 22… » sur une fiche qui s'arrête à 20). Leurs réponses restent
  justes, mais ce ne sont plus les exercices de la fiche : répondre `/non`, redresser la
  photo dans Telegram (bouton de retouche avant l'envoi) et la renvoyer.
- **Une double page** se photographie en entier : Mine de rien la coupe en deux.
- **Cadrer sans le prénom de l'enfant.** La photo passe par les serveurs de Telegram avant
  d'arriver à la maison. Mine de rien ne l'écrit pas sur le disque et ne l'envoie qu'au LLM
  local.
- Les problèmes rédigés ne sont pas lus : seules les opérations écrites sont retenues.

## En cas de souci

```bash
docker compose ps                  # les deux services tournent-ils ?
docker compose logs --tail 50 agent
docker compose logs --tail 50 minecraft
```

- **Le serveur ne télécharge pas Paper** (« Failed to download paper ») : souvent un souci
  de DNS avec Docker rootless. Ajouter le serveur DNS de la maison dans
  `~/.config/docker/daemon.json` (`"dns": ["192.168.1.1"]`), redémarrer Docker puis
  recréer les conteneurs avec `docker compose up -d --force-recreate`.
- **Aucun message Telegram** : vérifier `TELEGRAM_BOT_TOKEN` et la liste `parents`, puis
  les journaux de l'agent.
- **Le coffre n'apparaît pas** : l'atelier a-t-il été construit (étape 7) ? Le coffre
  précédent a-t-il été vidé ? Le suivant attend qu'il soit vide.
