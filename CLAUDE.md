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

## Façon de travailler

Pas de forge ni d'issues : les specs de `docs/specs/` sont la seule source de vérité.

- `/feature` : questions, puis spec dans `docs/specs/open/`, validée et commitée sur `main`. Aucun code.
- `/next` : une phase de la spec, sur sa branche `feat/<slug>`, un commit avec le trailer
  `Spec: <slug>, phase N`, puis arrêt avec le scénario à jouer en jeu.
- `/close` : après avoir joué, archive la spec dans `docs/specs/done/`. Ne fusionne pas.
- `/nouvel-exercice`, `/nouveau-jeu` : règles pour ajouter un générateur ou une mise en scène.
- `/essai-en-jeu` : serveur Minecraft d'essai jetable (`docker/essai.compose.yml`),
  jamais le monde réel de l'enfant.

Ne pas pousser ni fusionner sans qu'on le demande.

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
