# Datapacks

`mine_de_rien` est monté dans les deux serveurs (`docker-compose.yml` et
`docker/essai.compose.yml`) et recopié dans `world/datapacks` à chaque démarrage.

- **Zone de l'atelier** (`function/tick.mcfunction`) : met en mode aventure les joueurs en
  survie autour de chaque marqueur `mdr_atelier`, et les remet en survie quand ils en
  sortent. Le datapack ne contient aucune coordonnée : c'est l'agent qui pose le marqueur
  (`mine-de-rien atelier`).
- Format de pack 121, celui de Minecraft Java 26.3. À ajuster si `MC_VERSION` change.

Plus tard, l'arbre de progrès : un advancement par attendu de fin de CM2, déclencheur
`impossible`, accordé par l'agent via `/advancement grant`.
