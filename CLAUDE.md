# Mine de rien

Agent qui transforme les devoirs d'un enfant de CM2 en jeux dans son monde Minecraft,
piloté par les parents via Telegram, avec rapports de progression.

## Règles du projet

- L'agent est écrit en Rust : un seul crate, un seul binaire, `mine-de-rien`.
- Code, commentaires, docs et messages de commit en français. Identifiants Rust en français
  quand ils désignent des notions métier (compétence, énoncé, tentative), en anglais pour la
  plomberie technique standard.
- **Le LLM ne calcule jamais une réponse.** Tout exercice vient d'un générateur déterministe
  dans `src/exercises/`, la réponse est calculée par le générateur.
- Pas de `unsafe` (le crate l'interdit), pas de `unwrap()` hors des tests et des constantes :
  une panique dans une tâche arrête l'agent. Les longueurs de texte se comptent en
  caractères (`chars()`), jamais en octets.
- Rien ne sort de la maison sans configuration explicite. Pas de télémétrie, pas d'appel réseau
  caché. Le prénom et le chat de l'enfant ne sont jamais envoyés à un LLM distant. Une
  dépendance ajoutée à `Cargo.toml` ne doit ouvrir aucune connexion de son propre chef.
- Toute dépendance à un service externe passe par un adaptateur dans `src/adapters/`
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
cargo build                                 # compiler
cargo test                                  # tests
cargo fmt --check                           # mise en forme
cargo clippy --all-targets -- -D warnings   # lint
cargo run -- --config config.essai.toml lancer
```

`tests/donnees/oracle/` garde ce que produisait l'ancienne version Python de l'agent
(commandes Minecraft, messages, rapports) : un test `oracle_python` qui casse signale un
changement de comportement, à assumer dans le commit ou à corriger.

## Arborescence

- `src/adapters/` : jeu (RCON Java), canal (Telegram), LLM (Anthropic, compatible OpenAI),
  voix (Piper, locale)
- `src/exercises/` : générateurs par compétence
- `src/games/` : mises en scène dans Minecraft (coffre du soir, Pixel, ...)
- `src/storage/` : SQLite
- `src/orchestrateur.rs` : boucle de surveillance et commandes des parents
- `tests/` : un fichier par module, `tests/commun/` pour les faux mondes partagés
- `datapacks/` : zone de l'atelier, embarqué dans le binaire à la compilation
- `docs/` : idées et décisions d'architecture
