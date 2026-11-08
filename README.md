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
  générateurs déterministes en Python et la réponse est calculée. Le modèle de langage
  habille l'énoncé en histoire Minecraft et lit la photo du cahier, rien de plus.
- **LLM au choix.** Anthropic, ou n'importe quel serveur compatible OpenAI,
  dont `llama-server` de llama.cpp, Ollama ou LM Studio, pour tourner 100 % en local.
- **Pseudonymisation.** Quand un fournisseur distant est configuré, ni le prénom ni le
  chat brut de l'enfant ne partent, seulement le texte de l'exercice.

## Les jeux

| Jeu | Mécanisme Minecraft | État |
|---|---|---|
| Coffre du soir | coffre verrouillé par la réponse, à renommer sur une enclume | à tester en jeu ([spec](docs/specs/open/2026-11-coffre-du-soir.md)) |
| Arbre de progrès | datapack d'advancements, un nœud par compétence de CM2 | à faire |
| Bartholomé l'apprenti | villageois qui se trompe, l'enfant le corrige dans le chat | à faire |
| Bordure du monde | chaque exercice validé agrandit la bordure | à faire |
| Chantiers | enclos à construire avec une aire et un périmètre imposés | à faire |

Détail des idées et des choix : [docs/idees.md](docs/idees.md), [docs/architecture.md](docs/architecture.md).

## Pile technique

- **Minecraft Java Edition** sur un serveur **Paper** auto-hébergé, piloté par RCON et
  la lecture du log serveur. Aucun plugin requis pour commencer.
- **Java Edition uniquement** pour l'instant. Bedrock (tablette, console, Minecraft pour
  Windows) n'est pas pris en charge : voir [docs/architecture.md](docs/architecture.md).
- **Python 3.12+**, `uv`, SQLite.
- `docker compose` qui lance Paper et l'agent d'un coup.

## Installation

Le guide pas à pas, du bot Telegram au premier coffre : [docs/installation.md](docs/installation.md).

En résumé :

```bash
cp .env.example .env            # EULA, version, mot de passe RCON, jeton Telegram
cp config.example.toml config.toml
docker compose up -d            # serveur Minecraft + agent
```

Pour se connecter au serveur depuis le lanceur officiel ou un lanceur hors ligne comme XMCL,
voir [docs/rejoindre-le-serveur.md](docs/rejoindre-le-serveur.md).

## Développement

```bash
uv sync
uv run pytest
uv run ruff check
```

Tout se joue aussi sans Telegram, sur le serveur d'essai jetable
(`docker/essai.compose.yml`, jamais le monde de l'enfant) :

```bash
docker compose -f docker/essai.compose.yml --env-file .env up -d
cp config.essai.example.toml config.essai.toml                  # puis y mettre ton pseudo
uv run mine-de-rien --config config.essai.toml atelier          # construit l'atelier
uv run mine-de-rien --config config.essai.toml lancer           # surveille, pose les coffres
uv run mine-de-rien --config config.essai.toml coffre "division 2"   # dans un autre terminal
```

## Licence

MIT. Voir [LICENSE](LICENSE).
