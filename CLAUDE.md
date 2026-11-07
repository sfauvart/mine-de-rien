# Mine de rien

Agent qui transforme les devoirs d'un enfant de CM2 en jeux dans son monde Minecraft,
piloté par les parents via Telegram, avec rapports de progression.

## Règles du projet

- Code, commentaires, docs et messages de commit en français. Identifiants Python en français
  quand ils désignent des notions métier (compétence, énoncé, tentative), en anglais pour la
  plomberie technique standard.
- **Le LLM ne calcule jamais une réponse.** Tout exercice vient d'un générateur déterministe
  dans `src/mine_de_rien/exercises/`, la réponse est calculée en Python.
- Rien ne sort de la maison sans configuration explicite. Pas de télémétrie, pas d'appel réseau
  caché. Le prénom et le chat de l'enfant ne sont jamais envoyés à un LLM distant.
- Toute dépendance à un service externe passe par un adaptateur dans `src/mine_de_rien/adapters/`
  et doit avoir une alternative auto-hébergée.

## Commandes

```bash
uv sync            # installer
uv run pytest      # tests
uv run ruff check  # lint
```

## Arborescence

- `src/mine_de_rien/adapters/` : jeu (RCON Java), canal (Telegram), LLM (Anthropic, compatible OpenAI)
- `src/mine_de_rien/exercises/` : générateurs par compétence
- `src/mine_de_rien/games/` : mises en scène dans Minecraft (coffre du soir, Bartholomé, ...)
- `src/mine_de_rien/storage/` : SQLite
- `datapacks/` : arbre de progrès
- `docs/` : idées et décisions d'architecture
