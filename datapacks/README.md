# Datapacks

Les deux serveurs recopient `mine_de_rien` dans `world/datapacks` à chaque démarrage, mais
ne le prennent pas au même endroit :

- **Serveur réel** (`docker-compose.yml`) : le datapack est dans le binaire de l'agent,
  embarqué à la compilation. Le service ponctuel `datapack` l'écrit dans `data/datapacks`
  avant le démarrage du serveur. Agent et datapack ont donc toujours la même version.
- **Serveur d'essai** (`docker/essai.compose.yml`) : monté depuis la copie de travail, pour
  essayer une modification avant de la publier.

```bash
mine-de-rien datapack /un/dossier    # crée ou remplace /un/dossier/mine_de_rien, sans config
mine-de-rien datapack /un/dossier --source datapacks/mine_de_rien   # un autre que l'embarqué
```

- **Zone de l'atelier** (`function/tick.mcfunction`) : met en mode aventure les joueurs en
  survie autour de chaque marqueur `mdr_atelier`, et les remet en survie quand ils en
  sortent. Le datapack ne contient aucune coordonnée : c'est l'agent qui pose le marqueur
  (`mine-de-rien atelier`).
- **Son de dictée** (`function/dictee/jouer.mcfunction`) : fonction à macro qui lit au
  joueur le son dont l'agent a écrit le nom dans le stockage `mine_de_rien:dictee`. Les
  sons eux-mêmes ne sont pas dans le datapack mais dans un pack de ressources, construit et
  servi par l'agent (`src/pack.rs`).
- Format de pack 121, celui de Minecraft Java 26.3. Le pack de ressources des dictées, lui,
  est au format 97 (`FORMAT` dans `src/pack.rs`). Les deux se lisent dans le `version.json`
  du serveur (`pack_version`) et s'ajustent ensemble si `MC_VERSION` change.

Plus tard, l'arbre de progrès : un advancement par attendu de fin de CM2, déclencheur
`impossible`, accordé par l'agent via `/advancement grant`.
