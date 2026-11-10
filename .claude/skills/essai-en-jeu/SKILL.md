---
name: essai-en-jeu
description: Lance le serveur Minecraft d'essai jetable de Mine de rien (Docker, monde plat, ports séparés, jamais le monde réel de l'enfant), exécute des commandes via RCON et vérifie l'état du monde (blocs, données, coffres) sans ouvrir le jeu. À utiliser pour valider une commande Minecraft ou un jeu avant de le livrer, en particulier pendant /next et /nouveau-jeu.
---

# Essayer en jeu, sans le jeu

Les commandes Minecraft changent de syntaxe d'une version à l'autre. Une
commande copiée d'un wiki ou écrite de mémoire est une hypothèse tant qu'un
serveur de la bonne version ne l'a pas acceptée. Cette skill transforme
l'hypothèse en fait, sur un serveur qu'on peut casser.

## La règle

**Le serveur d'essai, jamais le monde réel.** Toujours
`docker/essai.compose.yml` (projet compose `mine-de-rien-essai`, dossier
`data/essai/`, ports 25566 et 25576). Si une commande vise
`docker-compose.yml`, `data/minecraft/` ou le port 25575, s'arrêter : c'est le
monde de l'enfant.

## 1. Préparer

```bash
grep -E '^(EULA|MC_VERSION)=' .env
```

- **Pas de `.env`, ou `EULA` différent de `TRUE`** → s'arrêter et le dire.
  Accepter l'EULA de Minecraft est une décision de l'utilisateur, pas de
  l'agent. Ne pas la passer à `TRUE` soi-même, ni la forcer en ligne de
  commande.
- **`MC_VERSION=LATEST`** → l'essai valide la dernière version, qui n'est pas
  forcément celle du serveur réel demain. Le signaler dans le rapport, et
  proposer d'épingler une version.

## 2. Lancer et attendre

```bash
docker compose -f docker/essai.compose.yml --env-file .env up -d
```

Le premier lancement télécharge l'image, Paper et génère le monde : plusieurs
minutes. Attendre la ligne `Done` du log plutôt que de deviner un délai :

```bash
docker compose -f docker/essai.compose.yml logs -f minecraft | grep -m1 'Done ('
```

Lancer cette attente en arrière-plan avec un délai maximal, et s'arrêter en
montrant la fin du log si le serveur ne démarre pas.

Puis relever la version réellement servie, qui figurera dans le rapport :

```bash
docker compose -f docker/essai.compose.yml exec -T minecraft rcon-cli version
```

## 3. Exécuter le scénario

Un alias évite de se tromper de serveur :

```bash
essai() { docker compose -f docker/essai.compose.yml exec -T minecraft rcon-cli "$@"; }
```

Chaque scénario suit le même schéma : **préparer la zone, agir, vérifier,
nettoyer.**

```bash
essai "forceload add 0 0"                          # la zone reste chargée sans joueur
essai "fill 0 -60 0 4 -56 4 air"                   # zone vierge
essai "setblock 0 -60 0 chest"                     # agir : la commande à valider
essai "data get block 0 -60 0"                     # vérifier : les données du bloc
essai "execute if block 0 -60 0 chest"             # vérifier : renvoie « Test passed »
```

- Travailler autour de `0 -60 0` : c'est la surface du monde plat, et le
  `forceload` évite les « position non chargée ».
- **Une commande refusée** renvoie un message d'erreur de syntaxe dans la sortie
  RCON. Le recopier tel quel dans le rapport, chercher la syntaxe de la version
  servie, corriger, recommencer.
- **Vérifier par l'état du monde**, pas par l'absence d'erreur : une commande
  acceptée peut ne rien faire (mauvaise coordonnée, mauvais composant).
- Pour vérifier le client RCON lui-même contre le vrai serveur (rafale de commandes,
  accents, reconnexion après redémarrage) : `cargo test --test java_rcon -- --ignored
  --nocapture`. Ce test redémarre le serveur d'essai.
- L'agent se lance contre ce serveur avec `cargo run -- --config config.essai.toml <action>`
  (`atelier`, `coffre "348 ÷ 12"`, `lancer`, `dire <pseudo> "…"`).
- Le code de l'agent peut aussi viser ce serveur via `config.essai.toml`
  (hôte `localhost`, port 25576, mot de passe `essai`). Ce fichier est local et
  ignoré par git : s'il manque, le créer avec
  `cp config.essai.example.toml config.essai.toml`. Ne jamais commiter la copie
  locale, elle porte le pseudo et l'identifiant Telegram du testeur.

## 4. Ce que l'essai ne prouve pas

Aucun joueur n'est connecté au serveur d'essai. Tout ce qui demande un joueur
(inventaire, renommage sur enclume, ouverture d'un coffre verrouillé, chat de
l'enfant, rendu d'un panneau à l'écran) ne se vérifie pas ici. Le lister
explicitement : ce sont les lignes du scénario « Vérification en jeu » que le
parent jouera.

## 5. Ranger

```bash
docker compose -f docker/essai.compose.yml stop
```

Garder le monde par défaut : le relancer est rapide. Pour repartir d'un monde
neuf, `down` puis supprimer `data/essai/`, après avoir vérifié que le chemin est
bien celui-là.

## 6. Rapporter

- la version de Minecraft servie ;
- chaque commande essayée, acceptée ou refusée, avec la sortie RCON utile ;
- l'état vérifié du monde ;
- ce qui reste à vérifier avec un vrai joueur ;
- les commandes de la spec ou de `docs/idees.md` dont la syntaxe a dû changer.
