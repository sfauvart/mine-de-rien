# Installer Mine de rien à la maison

Ce guide suffit pour installer Mine de rien sans avoir suivi son développement. À la fin,
un parent tape `/coffre 348 ÷ 12` sur Telegram, un coffre verrouillé apparaît près de la
base de l'enfant dans Minecraft, et les deux parents sont prévenus quand il l'ouvre.

Il faut :

- un PC sous Linux allumé quand l'enfant joue, avec Docker et `docker compose` ;
- Minecraft **Java Edition** sur l'ordinateur de l'enfant (pas Bedrock, pas de console) ;
- un compte Telegram par parent.

## 1. Récupérer les fichiers

Trois fichiers suffisent. Docker téléchargera le reste : l'image de l'agent
(`ghcr.io/sfauvart/mine-de-rien`, avec le datapack et le plugin de la voix) et celle du
serveur Minecraft.

Choisir la dernière version dans la liste https://github.com/sfauvart/mine-de-rien/tags
(par exemple `v0.2.0`, à écrire sans le `v` ci-dessous), puis :

```bash
mkdir mine-de-rien && cd mine-de-rien
VERSION=X.Y.Z    # la version choisie, sans le v
for f in docker-compose.yml .env.example config.example.toml; do
  curl -fsSLO "https://raw.githubusercontent.com/sfauvart/mine-de-rien/v$VERSION/$f"
done
cp .env.example .env
cp config.example.toml config.toml
```

Pour lire ou modifier le code, cloner le dépôt fait aussi l'affaire
(`git clone https://github.com/sfauvart/mine-de-rien.git`) : la suite est identique, et
`docker compose build agent` construit l'agent depuis le clone au lieu de le télécharger.

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
- **`[atelier]`** : à laisser commenté, l'étape 7 trouve l'emplacement toute seule.
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
  `docker compose exec agent mine-de-rien --config /app/config.toml llm "Bonjour ?"`
  doit répondre. Avec `adaptateur = "anthropic"`, aucune photo n'est lue : une photo ne
  part jamais vers un LLM distant.
- **`[voix]`** : facultatif, la voix de synthèse qui lira les dictées. Elle tourne à la
  maison, dans son propre conteneur (Piper), sans accès à Internet. Décommenter la section
  `[voix]` de `config.toml`, puis télécharger la voix une fois (60 Mo, depuis Hugging Face)
  et lancer le conteneur :

  ```bash
  docker compose --profile voix-installation run --rm voix-telecharger
  docker compose --profile voix up -d
  ```

  Vérifier en écoutant le fichier produit dans `data/agent/` :
  `docker compose exec agent mine-de-rien --config /app/config.toml voix "Bonjour, ceci est un essai." --sortie /data/essai.ogg`.
  Pour une autre voix française (`fr_FR-upmc-medium`, `fr_FR-tom-medium`), l'écrire dans
  `[voix] voix` et dans `.env` (`VOIX=fr_FR-upmc-medium`), puis refaire les deux commandes.
  Pour que l'enfant l'entende en jeu, garder aussi `dossier_du_jeu = "/sons"` dans cette
  section : c'est le dossier (`data/sons`) où l'agent dépose les sons que le serveur fait
  entendre par le chat vocal. Il faut alors le mod du chat vocal dans le jeu de l'enfant
  (étape 8).

  Avec ce réglage, la voix lit aussi **ce que l'agent et Pixel écrivent à l'enfant** :
  l'annonce d'un coffre, son rappel à la connexion (« 348 ÷ 12 » est lu « 348 divisé par
  12 »), les indices et les réponses de Pixel, « C'est la bonne réponse ! ». Tout reste
  écrit dans le chat : la voix s'ajoute, pour un enfant à qui lire coûte. Elle ne lit ni le
  mode d'emploi répété à chaque coffre, ni la phrase à trou d'une dictée, ni un texte qui
  donnerait la réponse. Pour ne garder à voix haute que les phrases de dictée :
  `dire_le_chat = false` dans `[voix]`.

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

Avant le serveur, un petit service `livraison` recopie ce qui vient avec la version de
l'agent, puis s'arrête : c'est normal qu'il apparaisse « Exited (0) ». Il dépose le datapack
dans `data/datapacks`, et dans `data/plugins` le plugin de la voix (`mine-de-rien-voix.jar`)
avec les réglages du chat vocal. Le serveur télécharge de son côté le plugin
**Simple Voice Chat** sur Modrinth, comme il télécharge Paper.

Le chat vocal ouvre un second port, **24454 en UDP**, entre le jeu de l'enfant et le
serveur. Comme le 25565, il ne se redirige jamais sur la box. Avec un pare-feu sur le PC :
`sudo ufw allow 24454/udp`.

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
docker compose up -d agent
```

Chaque parent ouvre une conversation avec le bot et envoie `/start`. Le bot répond
« Ton identifiant Telegram : 123456789 ». Recopier les deux nombres dans `config.toml` :

```toml
[canal]
parents = [123456789, 222333444]
```

Tant qu'un identifiant n'est pas dans cette liste, le bot ignore ses messages : `/start`
est la seule commande ouverte à tous, et elle ne fait que donner l'identifiant.

Redémarrer l'agent pour qu'il lise la liste :

```bash
docker compose restart agent
```

## 7. Construire l'atelier

L'atelier est un plancher de 5 × 5 avec un coffre, une enclume, une lanterne et un panneau.
Les coffres apparaîtront toujours là. Mine de rien cherche lui-même où le construire, autour
de l'enfant.

1. L'enfant se connecte au serveur et va **près de sa base**, à la surface, dans le monde
   normal (ni dans une grotte, ni dans le Nether).
2. Sur Telegram, envoyer au bot : `/atelier`.
3. Le bot répond qu'il cherche, puis où il a construit (entre 8 et 32 blocs de l'enfant).
   Dans le jeu, une ligne dans le chat lui indique la distance et la direction.

Il lui faut un carré de 5 × 5 parfaitement plat, sur du sol naturel, sans rien au-dessus ;
rien n'est cassé. S'il n'en trouve pas, le bot dit quoi faire : souvent, emmener l'enfant vers
un terrain plus dégagé et relancer `/atelier`.

Si l'emplacement ne convient pas, emmener l'enfant ailleurs et relancer `/atelier` : l'atelier
se déplace, à condition qu'aucun coffre ne soit en cours et que le coffre de l'atelier soit
vide. L'ancien emplacement redevient un carré de terre.

Tant qu'il n'y a pas d'atelier dans le monde, aucun coffre n'est posé et Pixel ne téléporte
pas : un message sur Telegram le rappelle. Après un changement de monde (nouveau serveur, monde
importé), relancer simplement `/atelier`.

### Imposer un endroit précis (facultatif)

Pour choisir l'emplacement exact au lieu de le laisser chercher :

1. Dans le jeu, aller sur du sol naturel dégagé (herbe, terre, sable, pierre), avec 3 blocs
   d'air au-dessus.
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
   docker compose exec agent mine-de-rien --config /app/config.toml atelier
   ```

   Si un bloc gêne (arbre, construction), Mine de rien refuse sans rien casser et dit où
   est ce bloc : choisir un autre emplacement ou dégager. La commande peut être relancée
   sans risque : elle répare sans dupliquer.

Autour de l'atelier, un joueur en survie passe en mode aventure : il ne peut ni casser le
coffre ni creuser dessous. Il repasse en survie en s'éloignant de quelques blocs.

## 8. Régler le lanceur de l'enfant

Créer une installation (lanceur officiel) ou une instance (XMCL) **exactement dans la
version de `MC_VERSION`**, puis ajouter le serveur avec l'adresse IP locale du PC. Détails
et dépannage : [rejoindre-le-serveur.md](rejoindre-le-serveur.md).

Sans loader, les coffres de calcul marchent. **Pour les dictées, il faut que l'enfant
entende** : son instance a besoin de Fabric et du mod Simple Voice Chat, dans la version
du plugin du serveur. C'est dix minutes, une fois, décrites geste par geste dans
[rejoindre-le-serveur.md](rejoindre-le-serveur.md#le-chat-vocal) : l'installation, l'étape
du micro à passer, le volume « Mine de rien ».

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
| `/atelier` | construit l'atelier près de l'enfant connecté, ou l'y déplace (étape 7) |
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

## 11. Pixel, l'aide dans le chat

Pixel est allumé par défaut dès que l'atelier existe (étape 7) et que `[jeu] log` pointe vers
le journal du serveur, ce que fait `config.example.toml`. L'annonce de chaque coffre le
présente à l'enfant. Dans le jeu, il appuie sur `T` pour ouvrir le chat, puis tape :

| Message | Ce que fait Pixel |
|---|---|
| `Pixel, ramène-moi au coffre` | le téléporte devant le coffre, d'où qu'il soit (Nether compris), une fois toutes les 5 minutes et seulement quand un coffre attend |
| `Pixel, aide-moi` | donne un indice, plus précis à chaque demande ; le dernier se répète. Jamais la réponse |
| autre chose après `Pixel,` | répond en une ou deux phrases, sans aucun chiffre, et rappelle ce qu'il sait faire |

Ce qu'il **ne fait pas** : donner des objets, changer l'heure ou le mode de jeu, faire un
calcul qui n'est pas celui du coffre, parler le premier. Un message qui ne commence pas par
« Pixel » n'est pas lu, ni celui d'un autre joueur.

**Ce que voient les parents** : le nombre d'indices, dans la notification d'ouverture
(« ouvert en 12 min, 2 indices ») et dans `/rapport`. Un coffre ouvert avec 3 indices ou
plus passe « à retravailler ». Le texte de la conversation n'est ni envoyé ni gardé.

**Ce qui part au LLM** : le message adressé à Pixel, l'énoncé du coffre et l'indice, jamais
la réponse ni le pseudo, et seulement vers un LLM **local** (`openai-compatible`). Avec
l'adaptateur `anthropic`, rien ne part : Pixel reconnaît les mots-clés (« aide »,
« ramène »…) et donne les indices tels quels. Comptez une dizaine de secondes de réflexion
avec le LLM local ; Pixel affiche « … » en attendant.

Pour l'éteindre :

```toml
[assistant]
actif = false
```

Pour le renommer : `nom = "Bob"` dans la même section (2 à 16 lettres), puis redémarrer
l'agent (`docker compose restart agent`).

## 12. Les monstres pendant les coffres

Tant qu'il reste un coffre à faire, posé à l'atelier ou en file, le serveur passe en mode
paisible : les monstres présents disparaissent, il n'en apparaît plus et la faim ne descend
plus. Quand il n'y a plus de coffre, le serveur revient à sa difficulté habituelle.

L'enfant, s'il est connecté, lit une ligne dans le chat à chaque changement : « Les monstres
s'endorment tant qu'un coffre t'attend. », puis « Plus de coffre : les monstres sont de
retour ! ». Rien n'est envoyé sur Telegram ; `/file` montre ce qui reste à faire.

La difficulté vaut pour **tout le serveur**, donc pour tous les joueurs connectés. Tant qu'un
coffre n'est pas résolu, le serveur reste paisible, sans limite de durée.

Pour choisir la difficulté hors coffre, ou reprendre la main :

```toml
[paisible]
actif = true
difficulte_normale = "easy"     # "easy", "normal" ou "hard"
```

Puis `docker compose restart agent`. **Avec `actif = true`, une difficulté réglée à la main
est corrigée en 3 secondes.** Pour garder par exemple un serveur paisible en permanence, mettre
`actif = false`, puis taper `docker compose exec minecraft rcon-cli difficulty peaceful`.

## 13. Préparer une dictée

Il faut la section `[voix]` de l'étape 4, avec `dossier_du_jeu = "/sons"`, et le chat vocal
dans le jeu de l'enfant
([rejoindre-le-serveur.md](rejoindre-le-serveur.md#le-chat-vocal)).

Quand la maîtresse donne le texte à préparer, l'envoyer au bot avec le jour de la dictée :

```
/dictee vendredi Dans la ville de Dubaï, aux Émirats arabes unis, s'élève la plus haute tour…
```

Le jour est un nom de jour (le prochain) ou une date (`13/11`). Pour une liste de mots sans
texte : `/dictee vendredi mots : plus, trois, personnes`.

Le bot répond « Je prépare la dictée… », puis renvoie le texte découpé en phrases numérotées
et un fichier audio : toute la dictée lue par la voix. **Comparer le texte à la feuille, mot
par mot, et écouter l'aperçu** : chaque mot peut devenir la réponse d'un coffre, et une
faute ici serait enseignée à l'enfant. `/oui` enregistre la dictée, `/non` abandonne. Sans
réponse, elle est oubliée au bout de 30 minutes.

Ensuite, plus rien à faire : **du lendemain jusqu'à la veille de la dictée, quatre coffres
l'attendent à sa première connexion de chaque jour**, et le bot l'annonce (« Séance de
dictée du jour : 4 coffre(s)… »). Un jour sans connexion n'est pas rattrapé. Un mot raté
repose un jour puis revient en priorité ; un mot réussi du premier coup deux fois de suite
ne revient plus. La veille, l'annonce donne les mots encore fragiles, avec ce que l'enfant
avait écrit. `/rapport` fait le même point à tout moment. Le nombre de coffres (3 à 5) et
le fuseau horaire se règlent dans la section `[dictee]` de `config.toml`.

| Message | Effet |
|---|---|
| `/dictee` | les dictées en cours, avec leur numéro |
| `/dictee seance 3` | une séance de plus pour la dictée n°3, tout de suite |
| `/dictee mots 3 plus trois` | ces mots sortiront en priorité, à leur place dans le texte |
| `/dictee annuler 3` | retire la dictée n°3, ses coffres en attente et ses sons |

Ce que vit l'enfant : le coffre s'annonce, la phrase est lue, le chat l'affiche avec
`_____` à la place du mot. Derrière le coffre, un grand tableau porte la dictée entière : le
mot à trouver est un trou jaune, les mots des coffres suivants des trous gris, et chaque mot
trouvé s'y écrit en vert. Il écrit le mot sur l'enclume : ni les majuscules ni les espaces ne
comptent, les accents si. Quand le mot est juste, le chat le dit (« C'est la bonne
réponse ! ») et le coffre s'ouvre. Un clic droit avec l'objet « Écouteur » relit la phrase. « Pixel,
aide-moi » la relit lentement, puis donne le nombre de lettres, puis la première lettre.

- **L'enfant reste connecté.** La phrase lui arrive par le chat vocal, dans la seconde,
  sans écran de chargement. Si son jeu n'a pas le chat vocal, aucun coffre de dictée n'est
  posé : il lit « Demande à un parent d'installer le chat vocal », les coffres de calcul
  continuent, et le bot prévient les parents une fois par jour.
- **Photo de la feuille** : possible avec `[llm]` (étape 4), légende `dictée vendredi`. À
  l'essai, le modèle a recopié « noirades » pour « noiraudes » sur une feuille nette : le
  texte tapé est plus sûr, et une photo demande une relecture lettre par lettre.
- Les mots à majuscule accentuée (« Émirats ») ne sont jamais demandés : trop difficiles à
  taper. Les mots d'une seule lettre non plus.

## Mettre à jour

Le monde, la base des coffres, `.env` et `config.toml` sont conservés.

1. Faire une copie de `data/` : une nouvelle version peut changer celle du serveur
   Minecraft, qui convertit alors le monde.
2. Remplacer `docker-compose.yml` par celui de la nouvelle version, qui ne contient aucun
   réglage personnel :

   ```bash
   VERSION=X.Y.Z    # la nouvelle version, sans le v
   curl -fsSLO "https://raw.githubusercontent.com/sfauvart/mine-de-rien/v$VERSION/docker-compose.yml"
   ```

3. Comparer `config.toml` à `config.example.toml` de la même version, pour une section
   nouvelle à recopier. Depuis la version qui fait entendre la voix par le chat vocal : la
   section `[pack]` et la ligne `PACK_URL` de `.env` ne servent plus et peuvent être
   retirées (l'agent démarre aussi avec, en le signalant), `[voix]` gagne
   `dossier_du_jeu = "/sons"`, et le jeu de l'enfant a besoin du mod (étape 8).
4. Télécharger les images et redémarrer. Le datapack et le plugin de la voix suivent
   l'agent :

   ```bash
   docker compose pull
   docker compose up -d
   ```

## En cas de souci

```bash
docker compose ps -a               # minecraft et agent tournent, livraison est « Exited (0) »
docker compose logs --tail 50 agent
docker compose logs --tail 50 minecraft
```

- **La zone de l'atelier ne passe plus en mode aventure** : vérifier que
  `data/datapacks/mine_de_rien` existe et que `docker compose logs livraison` affiche
  « Datapack recopié ». Dans le log du serveur, chercher `Copying datapack`.
- **L'enfant n'entend pas la dictée**, ou le bot dit que son jeu n'est pas relié au chat
  vocal : `docker compose exec minecraft rcon-cli "mdrvoix etat <pseudo>"` doit répondre
  `relié` quand il est en jeu. `sans chat vocal` : le mod manque, n'est pas dans la version
  du plugin, ou le port 24454/udp est fermé. Une réponse « Unknown command » : le plugin
  n'est pas chargé, voir `docker compose logs livraison` (« Plugin recopié ») et chercher
  `mine-de-rien-voix` et `voicechat` dans le log du serveur.
- **Une image construite en local reste en place** : une image construite par
  `docker compose build agent` porte le même nom que l'image publiée, et `up` la garde.
  `docker compose pull` remet l'image publiée.

- **Le serveur ne télécharge pas Paper** (« Failed to download paper ») : souvent un souci
  de DNS avec Docker rootless. Ajouter le serveur DNS de la maison dans
  `~/.config/docker/daemon.json` (`"dns": ["192.168.1.1"]`), redémarrer Docker puis
  recréer les conteneurs avec `docker compose up -d --force-recreate`.
- **Aucun message Telegram** : vérifier `TELEGRAM_BOT_TOKEN` et la liste `parents`, puis
  les journaux de l'agent.
- **Le coffre n'apparaît pas** : l'atelier a-t-il été construit (étape 7) ? Le coffre
  précédent a-t-il été vidé ? Le suivant attend qu'il soit vide.
