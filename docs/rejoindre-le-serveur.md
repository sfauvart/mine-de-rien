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

- **Vanilla côté client.** Le serveur tourne sous Paper, mais Paper est un logiciel
  serveur. Le client n'a besoin d'aucun loader : ni Fabric, ni Forge, ni NeoForge.
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

- Au premier écran « Ce serveur recommande l'utilisation d'un pack de ressources »,
  cliquer **Oui**. Pour ne plus avoir la question : dans la liste des serveurs, sélectionner
  le serveur, **Modifier**, puis régler « Packs de ressources du serveur » sur **Activés**.
- Le volume de la voix se règle dans **Options**, **Musique et sons**, curseur **Voix**. À
  zéro, les dictées sont muettes.
- Le pack se télécharge depuis le PC du serveur, port 8765. S'il ne se charge pas, ouvrir
  ce port comme le 25565 (voir « Pare-feu » plus bas) et vérifier `PACK_URL` dans `.env` :
  depuis le poste de l'enfant, un navigateur ouvert sur cette adresse doit télécharger un
  fichier `mine-de-rien.zip`.
- Une nouvelle dictée arrive dans le pack à la **connexion suivante** : l'enfant doit se
  déconnecter et se reconnecter pour l'entendre.

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
