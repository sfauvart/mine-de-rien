# Mine de rien

*Les devoirs se font sans que personne ne s'en aperçoive.*
*Homework that happens without anyone noticing.*

Mine de rien transforme les devoirs d'un enfant en jeux dans **son** monde Minecraft.
Les parents envoient les devoirs du moment depuis Telegram, l'agent les met en scène
dans le jeu avec de vraies récompenses, puis renvoie un rapport : ce qui est acquis,
ce qui est à retravailler.

## Principes non négociables

- **Rien ne sort de la maison.** Serveur Minecraft auto-hébergé, scores et rapports
  dans un fichier SQLite local. Aucun compte à créer chez un tiers, aucun abonnement.
- **Le LLM n'est jamais la source de vérité.** Les exercices sont engendrés par des
  générateurs déterministes et la réponse est calculée par le code. Le modèle de langage
  habille l'énoncé en histoire Minecraft et lit la photo du cahier, rien de plus.
- **LLM au choix.** Anthropic, ou n'importe quel serveur compatible OpenAI,
  dont `llama-server` de llama.cpp, Ollama ou LM Studio, pour tourner 100 % en local.
- **Pseudonymisation.** Quand un fournisseur distant est configuré, ni le prénom ni le
  chat brut de l'enfant ne partent, seulement le texte de l'exercice.

## Les jeux

| Jeu | Mécanisme Minecraft | État |
|---|---|---|
| Coffre du soir | coffre verrouillé par la réponse, à renommer sur une enclume | livré ([spec](docs/specs/done/2026-11-coffre-du-soir.md)) |
| Dictée de la semaine | coffre du soir dont la phrase est lue par une voix locale, la clé est le mot manquant | livré ([spec](docs/specs/done/2026-11-dictee-audio.md)) |
| Arbre de progrès | datapack d'advancements, un nœud par compétence de CM2 | à faire |
| Bartholomé l'apprenti | villageois qui se trompe, l'enfant le corrige dans le chat | à faire |
| Bordure du monde | chaque exercice validé agrandit la bordure | à faire |
| Chantiers | enclos à construire avec une aire et un périmètre imposés | à faire |

Détail des idées et des choix : [docs/idees.md](docs/idees.md), [docs/architecture.md](docs/architecture.md).

## Pile technique

- **Minecraft Java Edition** sur un serveur **Paper** auto-hébergé, piloté par RCON et
  la lecture du log serveur. Les coffres de calcul ne demandent aucun plugin. Pour les
  dictées, l'enfant entend la voix par le chat vocal du jeu : le serveur porte le plugin
  **Simple Voice Chat** et un petit plugin maison (`plugin/`), le jeu de l'enfant le mod
  du même nom ([docs/rejoindre-le-serveur.md](docs/rejoindre-le-serveur.md)).
- **Java Edition uniquement** pour l'instant. Bedrock (tablette, console, Minecraft pour
  Windows) n'est pas pris en charge : voir [docs/architecture.md](docs/architecture.md).
- **Rust**, un seul binaire statique, SQLite embarqué.
- `docker compose` qui lance Paper et l'agent d'un coup. L'agent, son datapack et son
  plugin sont publiés en une image, `ghcr.io/sfauvart/mine-de-rien`, pour amd64 et arm64 :
  elle ne contient que le binaire et le `.jar` du plugin.

## Installation

Le guide pas à pas, du bot Telegram au premier coffre : [docs/installation.md](docs/installation.md).

En résumé, sans cloner le dépôt : récupérer `docker-compose.yml`, `.env.example` et
`config.example.toml` d'une version taguée, puis

```bash
cp .env.example .env            # EULA, version, mot de passe RCON, jeton Telegram
cp config.example.toml config.toml
docker compose up -d            # télécharge les images, livre datapack et plugin, lance serveur et agent
```

Publier une nouvelle version de l'image : [docs/publier-une-version.md](docs/publier-une-version.md).

Pour se connecter au serveur depuis le lanceur officiel ou un lanceur hors ligne comme XMCL,
voir [docs/rejoindre-le-serveur.md](docs/rejoindre-le-serveur.md).

## Développement

Il faut Rust (la version est figée dans `rust-toolchain.toml`, `rustup` l'installe seul).

```bash
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Tout se joue aussi sans Telegram, sur le serveur d'essai jetable
(`docker/essai.compose.yml`, jamais le monde de l'enfant) :

```bash
docker compose -f docker/essai.compose.yml --env-file .env up -d
cp config.essai.example.toml config.essai.toml                  # puis y mettre ton pseudo
cargo run -- --config config.essai.toml atelier          # construit l'atelier
cargo run -- --config config.essai.toml lancer           # surveille, pose les coffres
cargo run -- --config config.essai.toml coffre "division 2"   # dans un autre terminal
```

## Licence

MIT. Voir [LICENSE](LICENSE).
