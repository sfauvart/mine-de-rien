# Rejoindre le serveur

Ce guide explique comment un joueur de la maison se connecte au serveur Minecraft de
Mine de rien. Il couvre le lanceur officiel et les lanceurs qui acceptent les comptes
hors ligne, comme XMCL.

Le serveur tourne en **Minecraft Java Edition**. Bedrock (tablette, console, Minecraft
pour Windows) n'est pas pris en charge.

## Choisir le mode du serveur

La variable `ONLINE_MODE` du fichier `.env` décide quels comptes le serveur accepte.

| Valeur | Comptes acceptés | Lanceurs |
|---|---|---|
| `TRUE`, par défaut | comptes Microsoft vérifiés uniquement | lanceur officiel, et tout lanceur connecté à un compte Microsoft |
| `FALSE` | n'importe quel pseudo, sans vérification | XMCL, Prism Launcher et autres lanceurs en compte hors ligne |

Le mode hors ligne correspond à l'esprit du projet : aucun compte à créer chez un
fournisseur pour jouer à la maison. Il a une contrepartie : le serveur croit sur parole
le pseudo annoncé. Dès que `ONLINE_MODE=FALSE`, deux règles deviennent obligatoires.

1. **Activer la liste blanche**, pour que seuls les pseudos de la famille puissent entrer.
   Voir plus bas.
2. **Ne jamais rendre le serveur accessible depuis internet.** Pas de redirection du port
   25565 sur la box, pas de tunnel. Sinon, n'importe qui pourrait se connecter avec le
   pseudo de l'enfant.

Après avoir modifié `.env`, recréer le serveur. Le monde est conservé dans
`data/minecraft` :

```bash
docker compose up -d minecraft
```

La même variable s'applique au serveur d'essai, `docker/essai.compose.yml`.

## Préparer le lanceur

### Ce qui vaut pour tous les lanceurs

- **Même version que le serveur.** Le client doit être exactement dans la version de
  Minecraft servie, celle de `MC_VERSION` dans `.env`. Pour connaître la version réelle :

  ```bash
  docker compose logs minecraft | grep "Starting minecraft server version"
  ```

- **Vanilla côté client**, sauf pour entendre la voix. Le serveur tourne sous Paper, mais
  Paper est un logiciel serveur : pour jouer, le client n'a besoin d'aucun loader. La voix
  en direct, elle, demande Fabric et un mod : voir « Le chat vocal » plus bas.
- **Java adapté.** Les versions récentes de Minecraft exigent un Java récent. Laisser le
  lanceur proposer le Java qui convient plutôt que d'utiliser celui du système.

### Lanceur officiel

Avec `ONLINE_MODE=TRUE`. Dans « Installations », créer une installation sur la version du
serveur, puis la lancer avec le compte Microsoft du joueur.

### XMCL, compte hors ligne

Avec `ONLINE_MODE=FALSE`.

1. **Activer le mode développeur** dans les paramètres de XMCL. C'est lui qui fait
   apparaître le type de compte hors ligne.
2. **Ajouter un compte hors ligne** avec le pseudo du joueur. Ce pseudo est celui que
   verra le serveur. Pour l'enfant, c'est aussi celui à mettre dans `pseudo_minecraft`
   de `config.toml`, pour que Mine de rien sache à qui poser les coffres.
3. **Créer une instance** sur la version du serveur, sans loader.
4. **Lancer l'instance** avec ce compte.

Les autres lanceurs à compte hors ligne, comme Prism Launcher, suivent le même schéma.

## Se connecter

Dans le jeu : Multijoueur, puis « Ajouter un serveur ». L'adresse dépend de l'endroit où
joue l'enfant.

| Où se trouve le joueur | Adresse |
|---|---|
| Sur le PC qui fait tourner Docker | `localhost` |
| Sur un autre ordinateur de la maison | l'adresse IP locale du PC serveur, par exemple `192.168.1.20` |
| Serveur d'essai, sur le PC qui fait tourner Docker | `localhost:25566` |

Pour trouver l'adresse IP locale du PC serveur, sous Linux :

```bash
ip -4 addr show | grep inet
```

Elle commence en général par `192.168.` ou `10.`. Si elle change d'un jour à l'autre,
réserver une adresse fixe pour ce PC dans l'interface de la box.

## Activer la liste blanche

La liste blanche est active dès le premier démarrage (`ENABLE_WHITELIST` et
`ENFORCE_WHITELIST` dans `docker-compose.yml`), mais vide : personne n'entre tant qu'on n'a
pas ajouté de pseudo. On ajoute chaque joueur avant sa première connexion :

```bash
docker compose exec minecraft rcon-cli whitelist add <pseudo>
docker compose exec minecraft rcon-cli whitelist list
```

En mode hors ligne, le serveur calcule lui-même l'identifiant du pseudo, sans rien
demander à internet (vérifié en 26.3). La variable `WHITELIST` de l'image Docker n'est
volontairement pas utilisée : elle ferait résoudre les pseudos par un service en ligne.

Le serveur identifie un joueur par un identifiant unique, calculé différemment selon le
mode. Une liste blanche remplie en mode en ligne ne reconnaît plus personne après le
passage en mode hors ligne, et inversement. Après un changement de `ONLINE_MODE`, retirer
puis rajouter chaque pseudo.

**Ne pas rendre l'enfant opérateur.** Un opérateur peut se donner des objets, changer de
mode de jeu et lire les données des blocs. Les jeux de Mine de rien n'auraient plus
aucun intérêt. Pour vérifier :

```bash
cat data/minecraft/ops.json
```

## Le pack de sons des dictées

Si les dictées sont installées (`[voix]` et `[pack]`, [installation.md](installation.md)
étape 4), le serveur propose à la connexion un « pack de ressources » : ce sont les phrases
de dictée lues à voix haute, rien d'autre.

- Avant de se connecter : dans **Multijoueur**, sélectionner le serveur dans la liste,
  cliquer **Modifier**, et régler « Packs de ressources du serveur » sur **Activés** (le
  bouton change de valeur à chaque clic), puis **Terminé**. Sur **Désactivés**, le jeu
  refuse le pack sans rien afficher, et aucun son de dictée ne s'entend : c'était le réglage
  des serveurs d'une instance XMCL à l'essai du 2026-11-10. Sur **Demander**, le jeu pose
  la question à chaque connexion : répondre **Oui**.
- Le volume de la voix se règle dans **Options**, **Musique et sons**, curseur **Voix**. À
  zéro, les dictées sont muettes.
- Le pack se télécharge depuis le PC du serveur, port 8765. S'il ne se charge pas, ouvrir
  ce port comme le 25565 (voir « Pare-feu » plus bas) et vérifier `PACK_URL` dans `.env` :
  depuis le poste de l'enfant, un navigateur ouvert sur cette adresse doit télécharger un
  fichier `mine-de-rien.zip`.
- Une nouvelle dictée arrive dans le pack à la **connexion suivante**. Si l'enfant joue
  quand elle arrive, l'agent renvoie son jeu vers le serveur : un écran de chargement
  passe, sans rien toucher. Il faut pour cela `adresse_du_jeu` dans `config.toml`
  ([installation.md](installation.md)) ; sans ce réglage, l'enfant se déconnecte et se
  reconnecte lui-même.

## Le chat vocal

*Pour l'instant sur le serveur d'essai seulement (`docker/essai.compose.yml`) : le vrai
serveur fait encore entendre les dictées par le pack de sons ci-dessus.*

La voix de Mine de rien arrive dans le jeu par un chat vocal, le mod **Simple Voice Chat** :
le serveur dit une phrase, le joueur l'entend dans la seconde, sans écran de chargement.
Seul le joueur désigné l'entend. Il faut le mod dans le jeu, dans la même version que le
plugin du serveur : **2.6.27**.

### Installer Fabric et le mod dans XMCL

Fabric est le « loader » qui permet à Minecraft de charger des mods. On l'ajoute à une
instance ; le plus simple est d'en créer une à part, pour garder l'instance d'origine.

1. Dans XMCL, **créer une instance** sur la version du serveur (26.3), et cette fois
   choisir le loader **Fabric** dans la liste des loaders, en version **0.19.3 ou plus
   récente**. XMCL le télécharge.
2. Ouvrir cette instance, puis sa page **Mods**. Dans la recherche, taper
   `Simple Voice Chat` (auteur : henkelmax), source **Modrinth**.
3. Choisir la version **`fabric-2.6.27+26.3`** dans la liste des versions, pas simplement
   la plus récente, et l'installer. Aucun autre mod n'est nécessaire : ce que XMCL
   propose en « optionnel » (Mod Menu, Cloth Config) peut être ignoré.
4. Lancer l'instance avec le compte habituel.

Sans la recherche de XMCL : télécharger `voicechat-fabric-2.6.27+26.3.jar` sur
<https://modrinth.com/plugin/simple-voice-chat/versions> (filtrer sur Fabric et 26.3) et
le déposer dans le dossier `mods` de l'instance (bouton « Ouvrir le dossier » de la page
de l'instance).

Ces téléchargements (Fabric, le mod) sont les seuls appels vers l'extérieur ; ils se font
une fois, à l'installation. Le mod installé ne contient aucune adresse internet (vérifié
dans son fichier, version 2.6.27).

### Au premier lancement : passer l'étape du micro

À la première connexion à un serveur équipé, le mod ouvre un assistant qui propose de
régler un micro. **Mine de rien n'écoute pas le micro et n'enregistre rien** ; l'enfant
n'en a pas besoin pour entendre.

- Passer l'assistant (bouton « Passer » ou « Skip »), ou le suivre sans choisir de micro.
  (Étape décrite d'après la documentation du mod, pas encore jouée : à corriger ici si
  l'assistant se présente autrement.)
- Ensuite, la touche **V** ouvre la fenêtre du chat vocal. La touche **M** coupe le micro :
  le laisser coupé. Si un autre joueur de la maison a aussi le mod et un micro actif, il
  peut entendre ceux qui parlent près de lui dans le jeu.
- Dans cette fenêtre, le bouton des **volumes** montre un curseur **Mine de rien** : c'est
  le volume de la voix des dictées et des messages.
- Une petite icône en bas à gauche de l'écran signale l'état du micro. Elle se masque dans
  les réglages du mod (touche V, puis la roue dentée).

### Réseau

Le chat vocal passe par un port **UDP** entre le jeu et le serveur : **24464** pour le
serveur d'essai. Si le jeu tourne sur un autre ordinateur que le serveur, ouvrir ce port
comme le 25565 (voir « Pare-feu » plus bas), en UDP :

```bash
sudo ufw allow 24464/udp
```

Ce port ne se redirige jamais sur la box, comme les autres.

### Vérifier

Le serveur dit si le jeu d'un joueur est relié :

```bash
docker compose -f docker/essai.compose.yml exec minecraft rcon-cli "mdrvoix etat <pseudo>"
```

| Réponse | Sens |
|---|---|
| `relié` | le joueur entendra la voix |
| `sans chat vocal` | le joueur est en jeu, mais sans le mod, dans une autre version du mod, ou le port UDP est fermé |
| `absent` | le joueur n'est pas connecté |

Puis lui faire dire une phrase (il faut la voix du compose d'essai, `--profile voix`) :

```bash
cargo run -- --config config.essai.toml parler "Bonjour, je suis Pixel."
```

## Si la connexion échoue

| Message dans le jeu | Cause probable | Que faire |
|---|---|---|
| Échec de la vérification du nom d'utilisateur | compte hors ligne sur un serveur en mode en ligne | passer `ONLINE_MODE=FALSE`, puis recréer le serveur |
| Client obsolète, ou serveur obsolète | versions différentes | régler l'instance sur la version du serveur |
| Vous n'êtes pas sur la liste blanche | pseudo différent, ou liste remplie dans l'autre mode | vérifier le pseudo, retirer puis rajouter |
| Connexion refusée, ou délai dépassé, depuis un autre ordinateur | pare-feu du PC serveur, ou mauvaise adresse | vérifier l'adresse IP, puis le pare-feu, voir ci-dessous |
| Connexion refusée, même en local | serveur arrêté ou en cours de démarrage | `docker compose ps` puis `docker compose logs minecraft` |

### Pare-feu

Avec Docker en mode rootless, le port 25565 est ouvert par un processus ordinaire : le
pare-feu du système s'applique. Avec ufw :

```bash
sudo ufw status
sudo ufw allow 25565/tcp
```

Avec Docker classique, Docker contourne en général ufw et le port est déjà ouvert sur le
réseau local. Raison de plus pour ne pas le rediriger sur la box.
