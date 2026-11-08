# Datapacks

Les deux serveurs recopient `mine_de_rien` dans `world/datapacks` à chaque démarrage, mais
ne le prennent pas au même endroit :

- **Serveur réel** (`docker-compose.yml`) : le datapack vient de l'image de l'agent, à
  `/app/datapacks/mine_de_rien`. Le service ponctuel `datapack` le recopie dans
  `data/datapacks` avant le démarrage du serveur. Agent et datapack ont donc toujours la
  même version.
- **Serveur d'essai** (`docker/essai.compose.yml`) : monté depuis la copie de travail, pour
  essayer une modification avant de la publier.

```bash
mine-de-rien datapack /un/dossier    # crée ou remplace /un/dossier/mine_de_rien, sans config
```

- **Zone de l'atelier** (`function/tick.mcfunction`) : met en mode aventure les joueurs en
  survie autour de chaque marqueur `mdr_atelier`, et les remet en survie quand ils en
  sortent. Le datapack ne contient aucune coordonnée : c'est l'agent qui pose le marqueur
  (`mine-de-rien atelier`).
- Format de pack 121, celui de Minecraft Java 26.3. À ajuster si `MC_VERSION` change.

Plus tard, l'arbre de progrès : un advancement par attendu de fin de CM2, déclencheur
`impossible`, accordé par l'agent via `/advancement grant`.
